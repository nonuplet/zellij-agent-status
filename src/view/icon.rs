//! 状態ごとのアイコンと、エージェント（`app`）ごとのアイコン。

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
    /// Nerd Font のアイコン（Font Awesome 系）。幅は 1 桁として扱う（[`app_icon`] を参照）。
    Nerd,
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
            // nf-fa-moon_o
            (IconStyle::Nerd, AgentState::Idle) => "\u{f186}",
            // nf-fa-gear
            (IconStyle::Nerd, AgentState::Working) => "\u{f013}",
            // nf-fa-exclamation_triangle
            (IconStyle::Nerd, AgentState::Blocked) => "\u{f071}",
            // nf-fa-check
            (IconStyle::Nerd, AgentState::Done) => "\u{f00c}",
            // nf-fa-times
            (IconStyle::Nerd, AgentState::Error) => "\u{f00d}",
        }
    }
}

/// `app` ごとの Nerd Font のアイコン。対応がなければ None（アイコンを出さない）。
///
/// Nerd Font の文字は Private Use Area にあり、East Asian Width は Ambiguous
/// （`unicode-width` の `width()` は 1、`width_cjk()` は 2）。幅は 1 桁として扱う。
/// 端末で Ambiguous を 2 桁にしていると表示がずれる（README に注意として書いている）。
/// Material Design 系（U+F0000 以降）は避け、BMP の Private Use Area の文字を使う。
pub(super) fn app_icon(app: &str) -> Option<&'static str> {
    match app {
        "claude-code" => Some("\u{ec82}"),
        _ => None,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    const STATES: [AgentState; 5] = [
        AgentState::Idle,
        AgentState::Working,
        AgentState::Blocked,
        AgentState::Done,
        AgentState::Error,
    ];

    /// 1 文字だけで、BMP の Private Use Area にあり、幅 1 として数えられること
    pub(in crate::view) fn assert_nerd_font_glyph(glyph: &str) {
        let mut chars = glyph.chars();
        let c = chars.next().unwrap();
        assert_eq!(chars.next(), None, "{glyph:?}");
        assert!(('\u{e000}'..='\u{f8ff}').contains(&c), "{glyph:?}");
        assert_eq!(glyph.width(), 1, "{glyph:?}");
    }

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

    #[test]
    fn nerd_icons_are_nerd_font_glyphs() {
        for state in STATES {
            assert_nerd_font_glyph(IconStyle::Nerd.icon(state));
        }
    }

    #[test]
    fn app_icons() {
        assert_eq!(app_icon("claude-code"), Some("\u{ec82}"));
        assert_nerd_font_glyph(app_icon("claude-code").unwrap());
        assert_eq!(app_icon("claude"), None);
        assert_eq!(app_icon("codex"), None);
    }
}
