//! エージェント同士の区切り。
//!
//! Nerd Font モードでは Powerline の区切り文字を使う。Powerline の塗りつぶしの区切り（`U+E0B0` など）は
//! 背景色を塗り分ける前提の形で、前景色だけで塗る今の配色では形にならないため、細い版だけを使う。

/// Nerd Font モードでないときの区切り
pub(super) const PLAIN_SEPARATOR: &str = "  ";

/// Nerd Font モードの区切り文字の種類。
/// Nerd Font の文字は 1 桁として扱う（`unicode-width` の `width()` と同じ）。前後に空白を入れて幅 3 にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Separator {
    /// `nf-pl-left_soft_divider`（U+E0B1）
    #[default]
    Arrow,
    /// `nf-ple-right_half_circle_thin`（U+E0B5）
    Round,
    /// `nf-ple-forwardslash_separator`（U+E0BB）
    Slash,
}

impl Separator {
    pub(super) fn text(self) -> &'static str {
        match self {
            Separator::Arrow => " \u{e0b1} ",
            Separator::Round => " \u{e0b5} ",
            Separator::Slash => " \u{e0bb} ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::icon::tests::assert_nerd_font_glyph;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn separators_are_nerd_font_glyphs_surrounded_by_spaces() {
        for separator in [Separator::Arrow, Separator::Round, Separator::Slash] {
            let text = separator.text();
            assert_eq!(text.width(), 3, "{separator:?}");
            let glyph = text
                .strip_prefix(' ')
                .and_then(|t| t.strip_suffix(' '))
                .unwrap();
            assert_nerd_font_glyph(glyph);
        }
    }
}
