//! 状態の収集。
//!
//! `PaneUpdate` で届く `PaneManifest` から、各ターミナルペインのルートレコード
//! （`id` が None の `ProgramStatusRecord`）を取り出して `AgentStatus` に変換する。
//! `id` 付きレコードは更新イベントが届かないことがあるため扱わない（doc/osc7501-findings.md）。
//!
//! レコードは送信元のプログラムが終わると Zellij が消すので、古いレコードの掃除はしない。
//!
//! 一覧はペイン id をキーにした HashMap で持ち、順序は持たない。表示の並び順は view で決める。
//!
//! タブ名は `TabUpdate` で別に届く。`PaneManifest.panes` のキーは `TabInfo.position` と同じ
//! （zellij-server が `tab.position` をキーにして作っている）ので、位置でタブ名を引いて付ける。
//! `TabUpdate` と `PaneUpdate` は届く順序が決まっていないため、タブ名の表を持っておき、
//! どちらが届いても付け直す。タブを移動した直後などで一時的にずれても、もう一方が届けば正しくなる。
//!
//! カレントディレクトリは `PaneInfo` にないので、別に持つ。表示するかどうか（`show_cwd`）は view で決め、
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

use crate::model::{AgentState, AgentStatus};
use zellij_tile::prelude::*;
use zellij_utils::attention::ProgramStatusRecord;

/// `ReadApplicationState` の許可状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Permission {
    /// 結果がまだ届いていない
    #[default]
    Pending,
    Granted,
    Denied,
}

/// イベントを受け取り、現在のエージェントの一覧を保持する。
#[derive(Debug, Default)]
pub struct Collector {
    permission: Permission,
    /// 最後に届いた `TabUpdate` の、タブの位置 → タブ名
    tab_names: HashMap<usize, String>,
    /// エージェントのペインの id → カレントディレクトリ。取得に失敗したペインは None で持ち、取得し直さない
    cwds: HashMap<u32, Option<PathBuf>>,
    /// キーはペイン id（ターミナルペインの id はタブをまたいでも一意）
    statuses: HashMap<u32, AgentStatus>,
}

impl Collector {
    /// `ZellijPlugin::load` から呼ぶ。permission の要求と、必要なイベントの購読を行う。
    ///
    /// permission ダイアログはプラグインのペインに出るため、許可が得られるまでは
    /// selectable のままにしておく（unselectable にするのは `update` で許可を受け取ったとき）。
    pub fn load(&self) {
        request_permission(&[PermissionType::ReadApplicationState]);
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::CwdChanged,
        ]);
    }

    /// `ZellijPlugin::update` から呼ぶ。再描画が必要なときに true を返す。
    pub fn update(&mut self, event: &Event) -> bool {
        let was_granted = self.permission == Permission::Granted;
        let mut changed = self.apply(event);
        if !was_granted && self.permission == Permission::Granted {
            set_selectable(false);
        }
        for pane_id in self.missing_cwds() {
            let cwd = get_pane_cwd(PaneId::Terminal(pane_id))
                .inspect_err(|e| {
                    eprintln!("zellij-agent-status: ペイン {pane_id} の cwd を取得できません: {e}")
                })
                .ok();
            changed |= self.set_cwd(pane_id, cwd);
        }
        changed
    }

    pub fn permission(&self) -> Permission {
        self.permission
    }

    /// 現在のエージェントの一覧。順序は決まっていない。
    pub fn statuses(&self) -> impl Iterator<Item = &AgentStatus> {
        self.statuses.values()
    }

    /// `update` のうち、Zellij の API を呼ばない部分（テスト用に分けている）。
    fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                let permission = match status {
                    PermissionStatus::Granted => Permission::Granted,
                    PermissionStatus::Denied => Permission::Denied,
                };
                let changed = self.permission != permission;
                self.permission = permission;
                changed
            }
            Event::PaneUpdate(manifest) => {
                let mut statuses = collect(manifest, &self.tab_names);
                self.cwds
                    .retain(|pane_id, _| statuses.contains_key(pane_id));
                for status in statuses.values_mut() {
                    status.cwd = self.cwds.get(&status.pane_id).cloned().flatten();
                }
                self.replace(statuses)
            }
            // エージェントでないペインの cwd は持たない（エージェントが現れたときに取得する）
            Event::CwdChanged(PaneId::Terminal(pane_id), cwd, _)
                if self.statuses.contains_key(pane_id) =>
            {
                self.set_cwd(*pane_id, Some(cwd.clone()))
            }
            Event::TabUpdate(tabs) => {
                self.tab_names = tab_names(tabs);
                let mut statuses = self.statuses.clone();
                for status in statuses.values_mut() {
                    status.tab_name = self.tab_names.get(&status.tab).cloned();
                }
                self.replace(statuses)
            }
            _ => false,
        }
    }

    /// 一覧を置き換え、変化があれば true を返す。
    /// `PaneUpdate` / `TabUpdate` はフォーカス移動などでも届くので、変化がなければ再描画しない。
    /// HashMap 同士の比較は順序に依存しないので、manifest の走査順の違いは変化として扱われない。
    fn replace(&mut self, statuses: HashMap<u32, AgentStatus>) -> bool {
        let changed = self.statuses != statuses;
        self.statuses = statuses;
        changed
    }

    /// cwd をまだ取得していないエージェントのペイン id（昇順）。
    fn missing_cwds(&self) -> Vec<u32> {
        let mut missing: Vec<u32> = self
            .statuses
            .keys()
            .filter(|pane_id| !self.cwds.contains_key(pane_id))
            .copied()
            .collect();
        missing.sort_unstable();
        missing
    }

    /// エージェントのペインの cwd を記録し、表示が変わるなら true を返す。
    fn set_cwd(&mut self, pane_id: u32, cwd: Option<PathBuf>) -> bool {
        let Some(status) = self.statuses.get_mut(&pane_id) else {
            return false;
        };
        self.cwds.insert(pane_id, cwd.clone());
        let changed = status.cwd != cwd;
        status.cwd = cwd;
        changed
    }
}

/// タブの位置（`TabInfo.position`）→ タブ名。
fn tab_names(tabs: &[TabInfo]) -> HashMap<usize, String> {
    tabs.iter()
        .map(|tab| (tab.position, tab.name.clone()))
        .collect()
}

/// `PaneManifest` から、ルートレコードを持つターミナルペインを `AgentStatus` にする。
/// キーはペイン id。タブ名は `tab_names`（タブの位置 → タブ名）から引く。
pub fn collect(
    manifest: &PaneManifest,
    tab_names: &HashMap<usize, String>,
) -> HashMap<u32, AgentStatus> {
    manifest
        .panes
        .iter()
        .flat_map(|(&tab, panes)| {
            let tab_name = tab_names.get(&tab);
            panes
                .iter()
                .filter(|pane| !pane.is_plugin)
                .filter_map(move |pane| {
                    let record = pane.program_status.iter().find(|r| r.is_root())?;
                    Some((pane.id, to_status(tab, tab_name, pane.id, record)))
                })
        })
        .collect()
}

fn to_status(
    tab: usize,
    tab_name: Option<&String>,
    pane_id: u32,
    record: &ProgramStatusRecord,
) -> AgentStatus {
    AgentStatus {
        pane_id,
        tab,
        tab_name: tab_name.cloned(),
        // manifest には含まれないので、Collector が別に付ける
        cwd: None,
        // OSC 7501 では state が必須なので通常は Some。None のときは Zellij 側の解釈（Idle）に合わせる
        state: AgentState::from(record.state()),
        kind: record.kind,
        progress: record.progress,
        app: record.app.clone(),
        title: record.title.clone(),
        msg: record.msg.clone(),
        updated_ms: record.updated_ms,
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

    fn tab_name_of(collector: &Collector, pane_id: u32) -> Option<&str> {
        collector
            .statuses()
            .find(|s| s.pane_id == pane_id)
            .and_then(|s| s.tab_name.as_deref())
    }

    #[test]
    fn tab_without_name_entry_has_no_tab_name() {
        let working = || vec![record(None, ProgramState::Working)];
        let m = manifest(vec![
            (0, vec![terminal(1, working())]),
            (1, vec![terminal(2, working())]),
        ]);
        let names = HashMap::from([(0, "main".to_owned())]);

        let statuses = collect(&m, &names);
        assert_eq!(statuses[&1].tab_name.as_deref(), Some("main"));
        assert_eq!(statuses[&2].tab_name, None);
    }

    #[test]
    fn tab_name_is_attached_when_tab_update_comes_first() {
        let mut collector = Collector::default();
        assert!(!collector.apply(&tab_update(&[(0, "main"), (1, "work")])));

        let m = manifest(vec![(
            1,
            vec![terminal(4, vec![record(None, ProgramState::Working)])],
        )]);
        assert!(collector.apply(&Event::PaneUpdate(m)));
        assert_eq!(tab_name_of(&collector, 4), Some("work"));
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
        assert_eq!(tab_name_of(&collector, 4), Some("work"));
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
        assert_eq!(tab_name_of(&collector, 2), Some("review"));

        // 名前に関係ない TabUpdate（フォーカス移動など）では再描画しない
        assert!(!collector.apply(&tab_update(&[(0, "main"), (1, "review")])));

        // タブの入れ替え：TabUpdate が先に届くと一時的にずれるが、PaneUpdate で正しくなる
        collector.apply(&tab_update(&[(0, "review"), (1, "main")]));
        collector.apply(&Event::PaneUpdate(manifest(vec![
            (0, vec![terminal(2, working())]),
            (1, vec![terminal(1, working())]),
        ])));
        assert_eq!(tab_name_of(&collector, 1), Some("main"));
        assert_eq!(tab_name_of(&collector, 2), Some("review"));
    }

    #[test]
    fn converts_all_fields_of_root_record() {
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
        let m = manifest(vec![(1, vec![terminal(7, vec![root])])]);
        let names = HashMap::from([(0, "main".to_owned()), (1, "work".to_owned())]);

        assert_eq!(
            collect(&m, &names),
            HashMap::from([(
                7,
                AgentStatus {
                    pane_id: 7,
                    tab: 1,
                    tab_name: Some("work".to_owned()),
                    cwd: None,
                    state: AgentState::Blocked,
                    kind: Some(BlockedKind::Permission),
                    progress: Some(42),
                    app: Some("claude".to_owned()),
                    title: Some("タイトル".to_owned()),
                    msg: Some("Bash を実行しますか".to_owned()),
                    updated_ms: 1234,
                }
            )])
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

        let statuses = collect(&m, &HashMap::new());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[&1].state, AgentState::Working);
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

        assert!(collect(&m, &HashMap::new()).is_empty());
    }

    #[test]
    fn skips_plugin_panes() {
        let mut plugin = terminal(1, vec![record(None, ProgramState::Working)]);
        plugin.is_plugin = true;
        let m = manifest(vec![(0, vec![plugin])]);

        assert!(collect(&m, &HashMap::new()).is_empty());
    }

    #[test]
    fn missing_state_is_treated_as_idle() {
        let root = ProgramStatusRecord::default();
        let m = manifest(vec![(0, vec![terminal(1, vec![root])])]);

        assert_eq!(collect(&m, &HashMap::new())[&1].state, AgentState::Idle);
    }

    #[test]
    fn keeps_tab_of_each_pane() {
        let working = || vec![record(None, ProgramState::Working)];
        let m = manifest(vec![
            (2, vec![terminal(5, working())]),
            (0, vec![terminal(9, working()), terminal(3, working())]),
            (1, vec![terminal(1, working())]),
        ]);

        let statuses = collect(&m, &HashMap::new());
        assert_eq!(statuses.len(), 4);
        assert_eq!(statuses[&1].tab, 1);
        assert_eq!(statuses[&5].tab, 2);
        assert_eq!(statuses[&9].tab, 0);
        assert_eq!(statuses[&3].tab, 0);
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
    fn permission_result_is_recorded() {
        let mut collector = Collector::default();
        assert_eq!(collector.permission(), Permission::Pending);

        assert!(collector.apply(&Event::PermissionRequestResult(PermissionStatus::Granted)));
        assert_eq!(collector.permission(), Permission::Granted);

        assert!(!collector.apply(&Event::PermissionRequestResult(PermissionStatus::Granted)));
    }

    fn cwd_of(collector: &Collector, pane_id: u32) -> Option<&str> {
        collector
            .statuses()
            .find(|s| s.pane_id == pane_id)
            .and_then(|s| s.cwd.as_deref())
            .and_then(|p| p.to_str())
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
        assert_eq!(cwd_of(&collector, 1), Some("/repo/main"));
        assert_eq!(cwd_of(&collector, 3), None);

        // PaneUpdate で作り直しても cwd は残り、変化として扱わない
        assert!(!collector.apply(&agents(&[3, 1])));
        assert_eq!(cwd_of(&collector, 1), Some("/repo/main"));
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
        assert_eq!(cwd_of(&collector, 1), Some("/repo/issue-21"));
        assert!(!collector.apply(&cwd_changed(1, "/repo/issue-21")));

        // 取得に失敗していたペインも、CwdChanged が届けば表示する
        collector.set_cwd(1, None);
        assert!(collector.apply(&cwd_changed(1, "/repo/main")));
        assert_eq!(cwd_of(&collector, 1), Some("/repo/main"));

        // TabUpdate で付け直しても cwd は残る
        collector.apply(&tab_update(&[(0, "main")]));
        assert_eq!(cwd_of(&collector, 1), Some("/repo/main"));
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
    }
}
