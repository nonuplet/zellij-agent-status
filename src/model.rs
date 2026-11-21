//! プラグイン内部のデータモデル。
//!
//! 状態の収集（`PaneUpdate` / `TabUpdate` からの変換）と表示は、この型を境界にして分けて実装する。
//! 元データは `PaneInfo.program_status` のルートレコード（`id` が None のもの）。
//! 決めた経緯は doc/osc7501-findings.md を参照。

use std::path::PathBuf;

use zellij_tile::prelude::BlockedKind;
use zellij_utils::attention::ProgramState;

/// 1 ペインで動いているエージェント 1 件分の状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    /// ターミナルペインの id（`PaneId::Terminal(pane_id)`）
    pub pane_id: u32,
    /// ペインが属するタブの位置（`PaneManifest.panes` のキー、0 始まり）
    pub tab: usize,
    /// ペインが属するタブの名前（`TabUpdate` の `TabInfo.name`）。`TabUpdate` がまだ届いていなければ None
    pub tab_name: Option<String>,
    /// ペインのカレントディレクトリ（ペインのルートプロセス、通常はシェルのもの）。
    /// まだ取得していないとき、取得に失敗したときは None。表示するかどうかは view の `show_cwd` で決める
    pub cwd: Option<PathBuf>,
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

impl AgentState {
    /// 注意が必要な状態か（`Blocked` / `Error`）。
    /// view の強調（`highlight_attention`）と msg の表示（`msg_mode`）が同じ判定を使い、
    /// 強調するエージェントと msg を表示するエージェントがずれないようにする。
    pub fn needs_attention(self) -> bool {
        matches!(self, AgentState::Blocked | AgentState::Error)
    }
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

    #[test]
    fn only_blocked_and_error_need_attention() {
        use AgentState::*;
        let cases = [
            (Idle, false),
            (Working, false),
            (Blocked, true),
            (Done, false),
            (Error, true),
        ];
        for (state, expected) in cases {
            assert_eq!(state.needs_attention(), expected, "{state:?}");
        }
    }
}
