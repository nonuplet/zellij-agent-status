//! 状態の収集。
//!
//! `PaneUpdate` で届く `PaneManifest` から、各ターミナルペインのルートレコード
//! （`id` が None の `ProgramStatusRecord`）を取り出して `AgentStatus` に変換する。
//! `id` 付きレコードは更新イベントが届かないことがあるため扱わない（doc/osc7501-findings.md）。
//!
//! レコードは送信元のプログラムが終わると Zellij が消すので、古いレコードの掃除はしない。

// main.rs への組み込み（doc/tasks.md の E）までは未使用のため
#![allow(dead_code)]

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
    statuses: Vec<AgentStatus>,
}

impl Collector {
    /// `ZellijPlugin::load` から呼ぶ。permission の要求と、必要なイベントの購読を行う。
    ///
    /// permission ダイアログはプラグインのペインに出るため、許可が得られるまでは
    /// selectable のままにしておく（unselectable にするのは `update` で許可を受け取ったとき）。
    pub fn load(&self) {
        request_permission(&[PermissionType::ReadApplicationState]);
        subscribe(&[EventType::PermissionRequestResult, EventType::PaneUpdate]);
    }

    /// `ZellijPlugin::update` から呼ぶ。再描画が必要なときに true を返す。
    pub fn update(&mut self, event: &Event) -> bool {
        let was_granted = self.permission == Permission::Granted;
        let changed = self.apply(event);
        if !was_granted && self.permission == Permission::Granted {
            set_selectable(false);
        }
        changed
    }

    pub fn permission(&self) -> Permission {
        self.permission
    }

    /// タブ順、同じタブ内はペイン id 順に並んだエージェントの一覧。
    pub fn statuses(&self) -> &[AgentStatus] {
        &self.statuses
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
                // PaneUpdate はフォーカス移動などでも届くので、変化がなければ再描画しない
                let statuses = collect(manifest);
                let changed = self.statuses != statuses;
                self.statuses = statuses;
                changed
            }
            _ => false,
        }
    }
}

/// `PaneManifest` から、ルートレコードを持つターミナルペインを `AgentStatus` にする。
///
/// 並び順はタブ順、同じタブ内ではペイン id 順。`PaneManifest.panes` は HashMap なので、
/// 表示が揺れないようにここで順序を固定する。
pub fn collect(manifest: &PaneManifest) -> Vec<AgentStatus> {
    let mut statuses: Vec<AgentStatus> = manifest
        .panes
        .iter()
        .flat_map(|(&tab, panes)| {
            panes
                .iter()
                .filter(|pane| !pane.is_plugin)
                .filter_map(move |pane| {
                    let record = pane.program_status.iter().find(|r| r.is_root())?;
                    Some(to_status(tab, pane.id, record))
                })
        })
        .collect();
    statuses.sort_by_key(|s| (s.tab, s.pane_id));
    statuses
}

fn to_status(tab: usize, pane_id: u32, record: &ProgramStatusRecord) -> AgentStatus {
    AgentStatus {
        pane_id,
        tab,
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
    use std::collections::HashMap;
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

        assert_eq!(
            collect(&m),
            vec![AgentStatus {
                pane_id: 7,
                tab: 1,
                state: AgentState::Blocked,
                kind: Some(BlockedKind::Permission),
                progress: Some(42),
                app: Some("claude".to_owned()),
                title: Some("タイトル".to_owned()),
                msg: Some("Bash を実行しますか".to_owned()),
                updated_ms: 1234,
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

        let statuses = collect(&m);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state, AgentState::Working);
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

        assert_eq!(collect(&m)[0].state, AgentState::Idle);
    }

    #[test]
    fn sorted_by_tab_then_pane_id() {
        let working = || vec![record(None, ProgramState::Working)];
        let m = manifest(vec![
            (2, vec![terminal(5, working())]),
            (0, vec![terminal(9, working()), terminal(3, working())]),
            (1, vec![terminal(1, working())]),
        ]);

        let order: Vec<(usize, u32)> = collect(&m).iter().map(|s| (s.tab, s.pane_id)).collect();
        assert_eq!(order, vec![(0, 3), (0, 9), (1, 1), (2, 5)]);
    }

    #[test]
    fn pane_update_reports_change_only_when_statuses_differ() {
        let mut collector = Collector::default();
        let m = manifest(vec![(
            0,
            vec![terminal(1, vec![record(None, ProgramState::Working)])],
        )]);

        assert!(collector.apply(&Event::PaneUpdate(m.clone())));
        assert_eq!(collector.statuses().len(), 1);

        // フォーカス移動など、エージェントの状態に関係ない変化
        let mut focused = m.clone();
        focused.panes.get_mut(&0).unwrap()[0].is_focused = true;
        assert!(!collector.apply(&Event::PaneUpdate(focused)));

        // レコードが消えた
        assert!(collector.apply(&Event::PaneUpdate(manifest(vec![(
            0,
            vec![terminal(1, vec![])]
        )]))));
        assert!(collector.statuses().is_empty());
    }

    #[test]
    fn permission_result_is_recorded() {
        let mut collector = Collector::default();
        assert_eq!(collector.permission(), Permission::Pending);

        assert!(collector.apply(&Event::PermissionRequestResult(PermissionStatus::Granted)));
        assert_eq!(collector.permission(), Permission::Granted);

        assert!(!collector.apply(&Event::PermissionRequestResult(PermissionStatus::Granted)));
    }

    #[test]
    fn unrelated_events_do_not_request_render() {
        let mut collector = Collector::default();
        assert!(!collector.apply(&Event::Timer(0.5)));
    }
}
