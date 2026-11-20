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
    /// 絵文字。幅は 2 桁。
    /// 端末による幅のずれを避けるため、East Asian Width が W で、異体字セレクタ（U+FE0F）なしで
    /// 絵文字表示になる文字（Emoji_Presentation）だけを使う。
    /// 絵文字は前景色で塗っても色が変わらないので、状態の色では区別できない。
    Emoji,
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
            (IconStyle::Emoji, AgentState::Idle) => "💤",
            (IconStyle::Emoji, AgentState::Working) => "⏳",
            (IconStyle::Emoji, AgentState::Blocked) => "❗",
            (IconStyle::Emoji, AgentState::Done) => "✅",
            (IconStyle::Emoji, AgentState::Error) => "❌",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    const STATES: [AgentState; 5] = [
        AgentState::Idle,
        AgentState::Working,
        AgentState::Blocked,
        AgentState::Done,
        AgentState::Error,
    ];

    #[test]
    fn symbols_are_one_column_wide() {
        for state in STATES {
            assert_eq!(IconStyle::Symbol.icon(state).width(), 1, "{state:?}");
            // 端末によって幅が変わらないこと（Ambiguous でないこと）
            assert_eq!(IconStyle::Symbol.icon(state).width_cjk(), 1, "{state:?}");
        }
    }

    #[test]
    fn emojis_are_two_columns_wide_without_variation_selector() {
        for state in STATES {
            let icon = IconStyle::Emoji.icon(state);
            // 1 文字だけで、異体字セレクタ（U+FE0F）に頼らないこと
            assert_eq!(icon.chars().count(), 1, "{state:?}");
            assert_eq!(icon.width(), 2, "{state:?}");
            assert_eq!(icon.width_cjk(), 2, "{state:?}");
        }
    }
}
