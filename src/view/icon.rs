//! 状態ごとのアイコンと、エージェント（`app`）ごとのアイコン。

use crate::model::AgentState;

/// 状態アイコンの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateIconStyle {
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
    /// 設定によっては右隣に空白を置く（`Item` で付ける。[`app_icon`] を参照）。
    NerdFont,
}

impl StateIconStyle {
    pub(super) fn icon(self, state: AgentState) -> &'static str {
        match (self, state) {
            // バッジ（反転した背景）の上でも形が分かるよう、線の細い記号（`◌` / `⟳`）は避けている
            (StateIconStyle::Symbol, AgentState::Idle) => "⋯",
            (StateIconStyle::Symbol, AgentState::Working) => "↻",
            (StateIconStyle::Symbol, AgentState::Blocked) => "!",
            (StateIconStyle::Symbol, AgentState::Done) => "✓",
            (StateIconStyle::Symbol, AgentState::Error) => "✗",
            (StateIconStyle::Alphabet, AgentState::Idle) => "I",
            (StateIconStyle::Alphabet, AgentState::Working) => "W",
            (StateIconStyle::Alphabet, AgentState::Blocked) => "B",
            (StateIconStyle::Alphabet, AgentState::Done) => "D",
            (StateIconStyle::Alphabet, AgentState::Error) => "E",
            (StateIconStyle::Emoji, AgentState::Idle) => "💤",
            (StateIconStyle::Emoji, AgentState::Working) => "⏳",
            (StateIconStyle::Emoji, AgentState::Blocked) => "❗",
            (StateIconStyle::Emoji, AgentState::Done) => "✅",
            (StateIconStyle::Emoji, AgentState::Error) => "❌",
            // nf-fa-moon_o
            (StateIconStyle::NerdFont, AgentState::Idle) => "\u{f186}",
            // nf-fa-gear
            (StateIconStyle::NerdFont, AgentState::Working) => "\u{f013}",
            // nf-fa-exclamation_triangle
            (StateIconStyle::NerdFont, AgentState::Blocked) => "\u{f071}",
            // nf-fa-check
            (StateIconStyle::NerdFont, AgentState::Done) => "\u{f00c}",
            // nf-fa-times
            (StateIconStyle::NerdFont, AgentState::Error) => "\u{f00d}",
        }
    }
}

/// `app` ごとのアイコン（Nerd Font の文字）。対応がなければ None（アイコンを出さない）。
/// 付けるかどうかは `show_app_icon` で決まり、状態アイコンの種類（[`StateIconStyle`]）には関係しない。
///
/// Nerd Font の文字は Private Use Area にあり、East Asian Width は Ambiguous
/// （`unicode-width` の `width()` は 1、`width_cjk()` は 2）。幅は 1 桁として扱う。
/// 端末で Ambiguous を 2 桁にしていると表示がずれる（README に注意として書いている）。
/// Material Design 系（U+F0000 以降）は避け、BMP の Private Use Area の文字を使う。
///
/// アイコンのグリフは 1 桁から右にはみ出して描かれることがある。はみ出すかどうか・どれだけはみ出すかは
/// フォントと端末の組み合わせで変わる（Mono 版のフォントでもはみ出すものがある。1 桁に収まるフォントでは、
/// 端末によってアイコンが小さく潰れて見分けられないことがある）。
/// 右隣が空白でないと、はみ出した部分が切れたり隣の文字と重なったりする。プログラムからは判別できないので、
/// `nerd_font_icon_padding` が true のときだけ、状態アイコン・`app` のアイコンの右隣に `Item` で空白を置く。
pub(super) fn app_icon(app: &str) -> Option<&'static str> {
    match app {
        "claude-code" => Some("\u{ec82}"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_support::assert_nerd_font_glyph;
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
            assert_eq!(StateIconStyle::Symbol.icon(state).width(), 1, "{state:?}");
            // 端末によって幅が変わらないこと（Ambiguous でないこと）
            assert_eq!(
                StateIconStyle::Symbol.icon(state).width_cjk(),
                1,
                "{state:?}"
            );
        }
    }

    #[test]
    fn emojis_are_two_columns_wide_without_variation_selector() {
        for state in STATES {
            let icon = StateIconStyle::Emoji.icon(state);
            // 1 文字だけで、異体字セレクタ（U+FE0F）に頼らないこと
            assert_eq!(icon.chars().count(), 1, "{state:?}");
            assert_eq!(icon.width(), 2, "{state:?}");
            assert_eq!(icon.width_cjk(), 2, "{state:?}");
        }
    }

    #[test]
    fn nerd_icons_are_nerd_font_glyphs() {
        for state in STATES {
            assert_nerd_font_glyph(StateIconStyle::NerdFont.icon(state));
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
