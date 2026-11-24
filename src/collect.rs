//! 状態の収集。
//!
//! Collector は、イベントごとに次の 3 つの表を持ち、[`Collector::statuses`] で結合して `AgentStatus` を作る。
//! どの表もペイン id かタブの位置をキーにした HashMap で、順序は持たない。表示の並び順は view で決める。
//!
//! | 表 | 元のイベント | 中身 |
//! |---|---|---|
//! | エージェントの表 | `PaneUpdate` | ペイン id → タブの位置と、ルートレコードから取り出した状態 |
//! | タブ名の表 | `TabUpdate` | タブの位置 → タブ名 |
//! | cwd の表 | `get_pane_cwd` / `CwdChanged` | ペイン id → カレントディレクトリ |
//!
//! 再描画するかどうかは、イベントが更新した表が変わったかで決める。`PaneUpdate` / `TabUpdate` は
//! フォーカスの移動などでも届くが、表が変わらなければ再描画しない。HashMap 同士の比較は順序に依存しないので、
//! manifest の走査順の違いは変化として扱われない。
//!
//! ## エージェントの表
//!
//! `PaneUpdate` で届く `PaneManifest` から、各ターミナルペインのルートレコード
//! （`id` が None の `ProgramStatusRecord`）を取り出す。
//! `id` 付きレコードは更新イベントが届かないことがあるため扱わない（doc/osc7501-findings.md）。
//! レコードはそのまま持たず、表示に使うフィールドだけを取り出す（[`Agent`]）。
//!
//! レコードは送信元のプログラムが終わると Zellij が消すので、古いレコードの掃除はしない。
//!
//! ## タブ名の表
//!
//! `PaneManifest.panes` のキーは `TabInfo.position` と同じ（zellij-server が `tab.position` を
//! キーにして作っている）ので、位置でタブ名を引く。`TabUpdate` と `PaneUpdate` は届く順序が
//! 決まっていないが、結合は `statuses` で行うので、どちらが先に届いても正しく付く。
//! タブを移動した直後などで一時的にずれても、もう一方が届けば正しくなる。
//!
//! ## cwd の表
//!
//! カレントディレクトリは `PaneInfo` にないので、別に取得する。表示するかどうか（`show_cwd`）は view で決め、
//! ここでは常に取得する（`get_pane_cwd` も `CwdChanged` も、すでに要求している `ReadApplicationState` で足りる）。
//! - エージェントのペインが現れたら、`get_pane_cwd` で 1 回取得する（`CwdChanged` は変化したときにしか届かないため）
//! - その後の移動は `CwdChanged` で追従する
//! - エージェントでなくなったペインの cwd は捨てる。次にエージェントが現れたら取得し直すので、
//!   その間に届いた `CwdChanged` を取りこぼしても古い値は残らない
//!
//! どちらも Zellij が返すのはペインのルートプロセス（通常はシェル）の cwd で、
//! エージェントが中で別のディレクトリに移っても追従しない。

use std::collections::HashMap;
use std::path::PathBuf;

use crate::MESSAGE_PREFIX;
use crate::model::{AgentState, AgentStatus};
use zellij_tile::prelude::*;
use zellij_utils::attention::ProgramStatusRecord;

/// イベントを受け取り、現在のエージェントの一覧を保持する。
#[derive(Debug, Default)]
pub struct Collector {
    /// エージェントの表。キーはペイン id（ターミナルペインの id はタブをまたいでも一意）
    agents: HashMap<u32, Agent>,
    /// タブ名の表。最後に届いた `TabUpdate` の、タブの位置 → タブ名
    tab_names: HashMap<usize, String>,
    /// cwd の表。エージェントのペインの id → カレントディレクトリ。
    /// 取得に失敗したペインは None で持ち、取得し直さない
    cwds: HashMap<u32, Option<PathBuf>>,
}

/// エージェントの表の 1 件。ルートレコードのうち、表示に使うフィールドだけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Agent {
    /// ペインが属するタブの位置（`PaneManifest.panes` のキー）
    tab: usize,
    state: AgentState,
    progress: Option<u8>,
    app: Option<String>,
    title: Option<String>,
    msg: Option<String>,
}

impl Collector {
    /// 収集に使うイベント。`ZellijPlugin::load` で購読する。
    /// 届くのは `ReadApplicationState` が許可された後（permission の要求は [`crate::permission`]）。
    pub const EVENTS: &[EventType] = &[
        EventType::PaneUpdate,
        EventType::TabUpdate,
        EventType::CwdChanged,
    ];

    /// `ZellijPlugin::update` から呼ぶ。再描画が必要なときに true を返す。
    pub fn update(&mut self, event: &Event) -> bool {
        let mut changed = self.apply(event);
        for pane_id in self.missing_cwds() {
            let cwd = get_pane_cwd(PaneId::Terminal(pane_id))
                .inspect_err(|e| {
                    eprintln!("{MESSAGE_PREFIX}ペイン {pane_id} の cwd を取得できません: {e}")
                })
                .ok();
            changed |= self.set_cwd(pane_id, cwd);
        }
        changed
    }

    /// 現在のエージェントの一覧。3 つの表を結合して作る。順序は決まっていない。
    pub fn statuses(&self) -> impl Iterator<Item = AgentStatus> + '_ {
        self.agents.iter().map(|(&pane_id, agent)| AgentStatus {
            pane_id,
            tab: agent.tab,
            tab_name: self.tab_names.get(&agent.tab).cloned(),
            cwd: self.cwds.get(&pane_id).cloned().flatten(),
            state: agent.state,
            progress: agent.progress,
            app: agent.app.clone(),
            title: agent.title.clone(),
            msg: agent.msg.clone(),
        })
    }

    /// `update` のうち、Zellij の API を呼ばない部分（テスト用に分けている）。
    fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                let agents = collect(manifest);
                self.cwds.retain(|pane_id, _| agents.contains_key(pane_id));
                replace(&mut self.agents, agents)
            }
            Event::TabUpdate(tabs) => replace(&mut self.tab_names, tab_names(tabs)),
            Event::CwdChanged(PaneId::Terminal(pane_id), cwd, _) => {
                self.set_cwd(*pane_id, Some(cwd.clone()))
            }
            _ => false,
        }
    }

    /// cwd をまだ取得していないエージェントのペイン id（昇順）。
    fn missing_cwds(&self) -> Vec<u32> {
        let mut missing: Vec<u32> = self
            .agents
            .keys()
            .filter(|pane_id| !self.cwds.contains_key(pane_id))
            .copied()
            .collect();
        missing.sort_unstable();
        missing
    }

    /// エージェントのペインの cwd を記録し、表示が変わるなら true を返す。
    /// エージェントでないペインの cwd は持たない（エージェントが現れたときに取得する）。
    /// まだ取得していないペインで取得に失敗した（`cwd` が None の）ときは、取得前と表示が同じなので変化として扱わない。
    fn set_cwd(&mut self, pane_id: u32, cwd: Option<PathBuf>) -> bool {
        if !self.agents.contains_key(&pane_id) {
            return false;
        }
        let old = self.cwds.insert(pane_id, cwd.clone()).flatten();
        old != cwd
    }
}

/// 表を置き換え、変化があれば true を返す。
fn replace<T: PartialEq>(table: &mut T, new: T) -> bool {
    let changed = *table != new;
    *table = new;
    changed
}

/// タブの位置（`TabInfo.position`）→ タブ名。
fn tab_names(tabs: &[TabInfo]) -> HashMap<usize, String> {
    tabs.iter()
        .map(|tab| (tab.position, tab.name.clone()))
        .collect()
}

/// `PaneManifest` から、ルートレコードを持つターミナルペインを取り出す。キーはペイン id。
fn collect(manifest: &PaneManifest) -> HashMap<u32, Agent> {
    manifest
        .panes
        .iter()
        .flat_map(|(&tab, panes)| {
            panes
                .iter()
                .filter(|pane| !pane.is_plugin)
                .filter_map(move |pane| {
                    let record = pane.program_status.iter().find(|r| r.is_root())?;
                    Some((pane.id, Agent::new(tab, record)))
                })
        })
        .collect()
}

impl Agent {
    fn new(tab: usize, record: &ProgramStatusRecord) -> Self {
        Agent {
            tab,
            // OSC 7501 では state が必須なので通常は Some。None のときは Zellij 側の解釈（Idle）に合わせる
            state: AgentState::from(record.state()),
            progress: record.progress,
            app: record.app.clone(),
            title: record.title.clone(),
            msg: record.msg.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zellij_utils::attention::ProgramState;

    fn record(id: Option<&str>, state: ProgramState) -> ProgramStatusRecord {
        ProgramStatusRecord {
            id: id.map(str::to_owned),
            state: Some(state),
            ..Default::default()
        }
    }

    fn terminal(id: u32, records: Vec<ProgramStatusRecord>) -> PaneInfo {
        PaneInfo {
            id,
            program_status: records,
            ..Default::default()
        }
    }

    fn manifest(tabs: Vec<(usize, Vec<PaneInfo>)>) -> PaneManifest {
        PaneManifest {
            panes: tabs.into_iter().collect::<HashMap<_, _>>(),
        }
    }

    fn tab_update(tabs: &[(usize, &str)]) -> Event {
        Event::TabUpdate(
            tabs.iter()
                .map(|&(position, name)| TabInfo {
                    position,
                    name: name.to_owned(),
                    ..Default::default()
                })
                .collect(),
        )
    }

    fn status_of(collector: &Collector, pane_id: u32) -> Option<AgentStatus> {
        collector.statuses().find(|s| s.pane_id == pane_id)
    }

    fn tab_name_of(collector: &Collector, pane_id: u32) -> Option<String> {
        status_of(collector, pane_id)?.tab_name
    }

    #[test]
    fn tab_without_name_entry_has_no_tab_name() {
        let mut collector = Collector::default();
        let working = || vec![record(None, ProgramState::Working)];
        collector.apply(&tab_update(&[(0, "main")]));
        collector.apply(&Event::PaneUpdate(manifest(vec![
            (0, vec![terminal(1, working())]),
            (1, vec![terminal(2, working())]),
        ])));

        assert_eq!(tab_name_of(&collector, 1).as_deref(), Some("main"));
        assert_eq!(tab_name_of(&collector, 2), None);
    }

    #[test]
    fn tab_name_is_attached_when_tab_update_comes_first() {
        let mut collector = Collector::default();
        // タブ名の表が変わるので、エージェントがいなくても再描画する
        assert!(collector.apply(&tab_update(&[(0, "main"), (1, "work")])));

        let m = manifest(vec![(
            1,
            vec![terminal(4, vec![record(None, ProgramState::Working)])],
        )]);
        assert!(collector.apply(&Event::PaneUpdate(m)));
        assert_eq!(tab_name_of(&collector, 4).as_deref(), Some("work"));
    }

    #[test]
    fn tab_name_is_attached_when_pane_update_comes_first() {
        let mut collector = Collector::default();
        let m = manifest(vec![(
            1,
            vec![terminal(4, vec![record(None, ProgramState::Working)])],
        )]);
        assert!(collector.apply(&Event::PaneUpdate(m)));
        assert_eq!(tab_name_of(&collector, 4), None);

        assert!(collector.apply(&tab_update(&[(0, "main"), (1, "work")])));
        assert_eq!(tab_name_of(&collector, 4).as_deref(), Some("work"));
    }

    #[test]
    fn tab_rename_and_move_are_reflected() {
        let mut collector = Collector::default();
        let working = || vec![record(None, ProgramState::Working)];
        collector.apply(&tab_update(&[(0, "main"), (1, "work")]));
        collector.apply(&Event::PaneUpdate(manifest(vec![
            (0, vec![terminal(1, working())]),
            (1, vec![terminal(2, working())]),
        ])));

        // タブ名の変更
        assert!(collector.apply(&tab_update(&[(0, "main"), (1, "review")])));
        assert_eq!(tab_name_of(&collector, 2).as_deref(), Some("review"));

        // 名前に関係ない TabUpdate（フォーカス移動など）では再描画しない
        assert!(!collector.apply(&tab_update(&[(0, "main"), (1, "review")])));

        // タブの入れ替え：TabUpdate が先に届くと一時的にずれるが、PaneUpdate で正しくなる
        collector.apply(&tab_update(&[(0, "review"), (1, "main")]));
        collector.apply(&Event::PaneUpdate(manifest(vec![
            (0, vec![terminal(2, working())]),
            (1, vec![terminal(1, working())]),
        ])));
        assert_eq!(tab_name_of(&collector, 1).as_deref(), Some("main"));
        assert_eq!(tab_name_of(&collector, 2).as_deref(), Some("review"));
    }

    #[test]
    fn converts_all_fields_of_root_record() {
        let mut collector = Collector::default();
        let root = ProgramStatusRecord {
            id: None,
            state: Some(ProgramState::Blocked),
            kind: Some(BlockedKind::Permission),
            progress: Some(42),
            app: Some("claude".to_owned()),
            title: Some("タイトル".to_owned()),
            msg: Some("Bash を実行しますか".to_owned()),
            updated_ms: 1234,
        };
        collector.apply(&tab_update(&[(0, "main"), (1, "work")]));
        collector.apply(&Event::PaneUpdate(manifest(vec![(
            1,
            vec![terminal(7, vec![root])],
        )])));
        collector.set_cwd(7, Some(PathBuf::from("/repo/main")));

        assert_eq!(
            collector.statuses().collect::<Vec<_>>(),
            vec![AgentStatus {
                pane_id: 7,
                tab: 1,
                tab_name: Some("work".to_owned()),
                cwd: Some(PathBuf::from("/repo/main")),
                state: AgentState::Blocked,
                progress: Some(42),
                app: Some("claude".to_owned()),
                title: Some("タイトル".to_owned()),
                msg: Some("Bash を実行しますか".to_owned()),
            }]
        );
    }

    #[test]
    fn uses_only_root_record() {
        let m = manifest(vec![(
            0,
            vec![terminal(
                1,
                vec![
                    record(Some("sub"), ProgramState::Error),
                    record(None, ProgramState::Working),
                ],
            )],
        )]);

        let agents = collect(&m);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[&1].state, AgentState::Working);
    }

    #[test]
    fn skips_panes_without_root_record() {
        let m = manifest(vec![(
            0,
            vec![
                terminal(1, vec![]),
                terminal(2, vec![record(Some("sub"), ProgramState::Working)]),
            ],
        )]);

        assert!(collect(&m).is_empty());
    }

    #[test]
    fn skips_plugin_panes() {
        let mut plugin = terminal(1, vec![record(None, ProgramState::Working)]);
        plugin.is_plugin = true;
        let m = manifest(vec![(0, vec![plugin])]);

        assert!(collect(&m).is_empty());
    }

    #[test]
    fn missing_state_is_treated_as_idle() {
        let root = ProgramStatusRecord::default();
        let m = manifest(vec![(0, vec![terminal(1, vec![root])])]);

        assert_eq!(collect(&m)[&1].state, AgentState::Idle);
    }

    #[test]
    fn keeps_tab_of_each_pane() {
        let working = || vec![record(None, ProgramState::Working)];
        let m = manifest(vec![
            (2, vec![terminal(5, working())]),
            (0, vec![terminal(9, working()), terminal(3, working())]),
            (1, vec![terminal(1, working())]),
        ]);

        let agents = collect(&m);
        assert_eq!(agents.len(), 4);
        assert_eq!(agents[&1].tab, 1);
        assert_eq!(agents[&5].tab, 2);
        assert_eq!(agents[&9].tab, 0);
        assert_eq!(agents[&3].tab, 0);
    }

    #[test]
    fn pane_update_reports_change_only_when_statuses_differ() {
        let mut collector = Collector::default();
        let m = manifest(vec![(
            0,
            vec![terminal(1, vec![record(None, ProgramState::Working)])],
        )]);

        assert!(collector.apply(&Event::PaneUpdate(m.clone())));
        assert_eq!(collector.statuses().count(), 1);

        // フォーカス移動など、エージェントの状態に関係ない変化
        let mut focused = m.clone();
        focused.panes.get_mut(&0).unwrap()[0].is_focused = true;
        assert!(!collector.apply(&Event::PaneUpdate(focused)));

        // ペインの並び順だけが違う
        let working = || vec![record(None, ProgramState::Working)];
        let two = |ids: [u32; 2]| {
            manifest(vec![(
                0,
                ids.into_iter().map(|id| terminal(id, working())).collect(),
            )])
        };
        assert!(collector.apply(&Event::PaneUpdate(two([1, 2]))));
        assert!(!collector.apply(&Event::PaneUpdate(two([2, 1]))));

        // レコードが消えた
        assert!(collector.apply(&Event::PaneUpdate(manifest(vec![(
            0,
            vec![terminal(1, vec![])]
        )]))));
        assert_eq!(collector.statuses().count(), 0);
    }

    #[test]
    fn fields_not_shown_do_not_request_render() {
        let mut collector = Collector::default();
        let pane_update = |kind, updated_ms| {
            let root = ProgramStatusRecord {
                kind,
                updated_ms,
                ..record(None, ProgramState::Blocked)
            };
            Event::PaneUpdate(manifest(vec![(0, vec![terminal(1, vec![root])])]))
        };
        assert!(collector.apply(&pane_update(None, 1)));

        // 同じ状態の再送（updated_ms だけが変わる）
        assert!(!collector.apply(&pane_update(None, 2)));
        // 表示に使っていない kind だけが変わる
        assert!(!collector.apply(&pane_update(Some(BlockedKind::Permission), 3)));
    }

    fn cwd_of(collector: &Collector, pane_id: u32) -> Option<String> {
        let cwd = status_of(collector, pane_id)?.cwd?;
        cwd.to_str().map(str::to_owned)
    }

    fn cwd_changed(pane_id: u32, cwd: &str) -> Event {
        Event::CwdChanged(PaneId::Terminal(pane_id), PathBuf::from(cwd), vec![])
    }

    fn agents(pane_ids: &[u32]) -> Event {
        let panes = pane_ids
            .iter()
            .map(|&id| terminal(id, vec![record(None, ProgramState::Working)]))
            .collect();
        Event::PaneUpdate(manifest(vec![(0, panes)]))
    }

    #[test]
    fn cwd_is_fetched_once_for_new_agents() {
        let mut collector = Collector::default();
        collector.apply(&agents(&[3, 1]));
        assert_eq!(collector.missing_cwds(), vec![1, 3]);

        assert!(collector.set_cwd(1, Some(PathBuf::from("/repo/main"))));
        // 取得に失敗したペインも、取得し直さない
        assert!(!collector.set_cwd(3, None));
        assert!(collector.missing_cwds().is_empty());
        assert_eq!(cwd_of(&collector, 1).as_deref(), Some("/repo/main"));
        assert_eq!(cwd_of(&collector, 3), None);

        // PaneUpdate で作り直しても cwd は残り、変化として扱わない
        assert!(!collector.apply(&agents(&[3, 1])));
        assert_eq!(cwd_of(&collector, 1).as_deref(), Some("/repo/main"));
        assert!(collector.missing_cwds().is_empty());

        collector.apply(&agents(&[3, 1, 5]));
        assert_eq!(collector.missing_cwds(), vec![5]);
    }

    #[test]
    fn cwd_follows_cwd_changed() {
        let mut collector = Collector::default();
        collector.apply(&agents(&[1]));
        collector.set_cwd(1, Some(PathBuf::from("/repo/main")));

        assert!(collector.apply(&cwd_changed(1, "/repo/issue-21")));
        assert_eq!(cwd_of(&collector, 1).as_deref(), Some("/repo/issue-21"));
        assert!(!collector.apply(&cwd_changed(1, "/repo/issue-21")));

        // 取得に失敗していたペインも、CwdChanged が届けば表示する
        collector.set_cwd(1, None);
        assert!(collector.apply(&cwd_changed(1, "/repo/main")));
        assert_eq!(cwd_of(&collector, 1).as_deref(), Some("/repo/main"));

        // TabUpdate が届いても cwd は残る
        collector.apply(&tab_update(&[(0, "main")]));
        assert_eq!(cwd_of(&collector, 1).as_deref(), Some("/repo/main"));
    }

    #[test]
    fn cwd_of_non_agent_pane_is_not_kept() {
        let mut collector = Collector::default();
        collector.apply(&agents(&[1]));
        collector.set_cwd(1, Some(PathBuf::from("/repo/main")));

        // エージェントでないペインの CwdChanged は無視する
        assert!(!collector.apply(&cwd_changed(2, "/tmp")));
        assert!(!collector.apply(&Event::CwdChanged(
            PaneId::Plugin(1),
            PathBuf::from("/tmp"),
            vec![]
        )));

        // エージェントが終わると cwd を捨て、再び現れたら取得し直す
        collector.apply(&agents(&[]));
        assert!(!collector.apply(&cwd_changed(1, "/repo/issue-21")));
        collector.apply(&agents(&[1]));
        assert_eq!(cwd_of(&collector, 1), None);
        assert_eq!(collector.missing_cwds(), vec![1]);
    }

    #[test]
    fn unrelated_events_do_not_request_render() {
        let mut collector = Collector::default();
        assert!(!collector.apply(&Event::Timer(0.5)));
        // permission の結果は Collector では扱わない（crate::permission）
        assert!(!collector.apply(&Event::PermissionRequestResult(PermissionStatus::Granted)));
    }
}
