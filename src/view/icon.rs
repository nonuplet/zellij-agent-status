//! 状態ごとのアイコン。

use crate::model::AgentState;

/// アイコンの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconStyle {
    /// 記号。East Asian Width が Ambiguous の文字（`●` など）は、端末によって幅がずれるので使わない。
    #[default]
    Symbol,
    /// アルファベット 1 文字。記号のフォントがない環境向け。
    Alphabet,
}

impl IconStyle {
    pub(super) fn icon(self, state: AgentState) -> &'static str {
        match (self, state) {
            (IconStyle::Symbol, AgentState::Idle) => "◌",
            (IconStyle::Symbol, AgentState::Working) => "⟳",
            (IconStyle::Symbol, AgentState::Blocked) => "!",
            (IconStyle::Symbol, AgentState::Done) => "✓",
            (IconStyle::Symbol, AgentState::Error) => "✗",
            (IconStyle::Alphabet, AgentState::Idle) => "I",
            (IconStyle::Alphabet, AgentState::Working) => "W",
            (IconStyle::Alphabet, AgentState::Blocked) => "B",
            (IconStyle::Alphabet, AgentState::Done) => "D",
            (IconStyle::Alphabet, AgentState::Error) => "E",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn symbols_are_one_column_wide() {
        for state in [
            AgentState::Idle,
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Done,
            AgentState::Error,
        ] {
            assert_eq!(IconStyle::Symbol.icon(state).width(), 1, "{state:?}");
            // 端末によって幅が変わらないこと（Ambiguous でないこと）
            assert_eq!(IconStyle::Symbol.icon(state).width_cjk(), 1, "{state:?}");
        }
    }
}
