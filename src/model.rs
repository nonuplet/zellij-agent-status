//! プラグイン内部のデータモデル。
//!
//! 状態の収集（`PaneUpdate` からの変換）と表示は、この型を境界にして分けて実装する。
//! 元データは `PaneInfo.program_status` のルートレコード（`id` が None のもの）。
//! 決めた経緯は doc/osc7501-findings.md を参照。

// 収集と表示の実装が揃うまでは未使用のため
#![allow(dead_code)]

use zellij_tile::prelude::BlockedKind;
use zellij_utils::attention::ProgramState;

/// 1 ペインで動いているエージェント 1 件分の状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    /// ターミナルペインの id（`PaneId::Terminal(pane_id)`）
    pub pane_id: u32,
    /// ペインが属するタブの位置（`PaneManifest.panes` のキー、0 始まり）
    pub tab: usize,
    pub state: AgentState,
    /// `state` が `Blocked` のときの理由
    pub kind: Option<BlockedKind>,
    /// 0〜100
    pub progress: Option<u8>,
    /// 送信元のアプリ名（例：`claude`）
    pub app: Option<String>,
    /// base64 デコード済み
    pub title: Option<String>,
    /// base64 デコード済み
    pub msg: Option<String>,
    /// Zellij がレコードを受け取った時刻（UNIX エポックからのミリ秒）
    pub updated_ms: u64,
}

/// エージェントの状態。
///
/// `ProgramState` をそのまま使わず独自に持つのは、表示や設定の側を zellij-utils から切り離すため。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    Idle,
    Working,
    /// ユーザの操作待ち（許可・質問・ログインなど）
    Blocked,
    Done,
    Error,
}

impl From<ProgramState> for AgentState {
    fn from(state: ProgramState) -> Self {
        match state {
            ProgramState::Idle => AgentState::Idle,
            ProgramState::Working => AgentState::Working,
            ProgramState::Blocked => AgentState::Blocked,
            ProgramState::Done => AgentState::Done,
            ProgramState::Error => AgentState::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_state_maps_one_to_one() {
        let cases = [
            (ProgramState::Idle, AgentState::Idle),
            (ProgramState::Working, AgentState::Working),
            (ProgramState::Blocked, AgentState::Blocked),
            (ProgramState::Done, AgentState::Done),
            (ProgramState::Error, AgentState::Error),
        ];
        for (input, expected) in cases {
            assert_eq!(AgentState::from(input), expected);
        }
    }
}
