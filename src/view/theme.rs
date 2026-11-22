//! 色とテーマ。

use std::fmt;

use zellij_tile::prelude::{PaletteColor, Styling};

use crate::model::AgentState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
    /// 256 色のインデックス
    Indexed(u8),
}

impl From<PaletteColor> for Color {
    fn from(color: PaletteColor) -> Self {
        match color {
            PaletteColor::Rgb((r, g, b)) => Color::Rgb(r, g, b),
            PaletteColor::EightBit(n) => Color::Indexed(n),
        }
    }
}

impl Color {
    /// 前景色にする SGR のパラメータ
    pub(super) fn fg(self) -> Sgr {
        Sgr {
            color: self,
            base: 38,
        }
    }

    /// 背景色にする SGR のパラメータ
    pub(super) fn bg(self) -> Sgr {
        Sgr {
            color: self,
            base: 48,
        }
    }
}

/// 色を指定する SGR のパラメータ（`38;5;n` など）。文字列は作らず、[`Display`](fmt::Display) で書き出す。
#[derive(Debug, Clone, Copy)]
pub(super) struct Sgr {
    color: Color,
    /// 前景色なら 38、背景色なら 48
    base: u8,
}

impl fmt::Display for Sgr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let base = self.base;
        match self.color {
            Color::Rgb(r, g, b) => write!(f, "{base};2;{r};{g};{b}"),
            Color::Indexed(n) => write!(f, "{base};5;{n}"),
        }
    }
}

/// 表示に使う色。`ModeUpdate` で届く `ModeInfo.style.colors` から作る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub idle: Color,
    pub working: Color,
    pub blocked: Color,
    pub done: Color,
    pub error: Color,
}

impl Theme {
    /// 色の対応は zellij-utils の `From<Palette> for Styling` を参照
    /// （`text_unselected` の emphasis_0〜2 がそれぞれ orange / cyan / green、`exit_code_error.base` が red）。
    pub fn from_styling(styling: &Styling) -> Self {
        let text = &styling.text_unselected;
        Theme {
            fg: text.base.into(),
            bg: text.background.into(),
            idle: text.base.into(),
            working: text.emphasis_1.into(),
            blocked: text.emphasis_0.into(),
            done: text.emphasis_2.into(),
            error: styling.exit_code_error.base.into(),
        }
    }

    pub(super) fn state(&self, state: AgentState) -> Color {
        match state {
            AgentState::Idle => self.idle,
            AgentState::Working => self.working,
            AgentState::Blocked => self.blocked,
            AgentState::Done => self.done,
            AgentState::Error => self.error,
        }
    }
}

impl Default for Theme {
    /// `ModeUpdate` が届くまでに使う色
    fn default() -> Self {
        Theme::from_styling(&Styling::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_follows_styling() {
        let mut styling = Styling::default();
        styling.text_unselected.base = PaletteColor::EightBit(1);
        styling.text_unselected.background = PaletteColor::EightBit(2);
        styling.text_unselected.emphasis_0 = PaletteColor::EightBit(3);
        styling.text_unselected.emphasis_1 = PaletteColor::EightBit(4);
        styling.text_unselected.emphasis_2 = PaletteColor::EightBit(5);
        styling.exit_code_error.base = PaletteColor::Rgb((6, 7, 8));
        let theme = Theme::from_styling(&styling);
        assert_eq!(theme.fg, Color::Indexed(1));
        assert_eq!(theme.bg, Color::Indexed(2));
        assert_eq!(theme.idle, Color::Indexed(1));
        assert_eq!(theme.blocked, Color::Indexed(3));
        assert_eq!(theme.working, Color::Indexed(4));
        assert_eq!(theme.done, Color::Indexed(5));
        assert_eq!(theme.error, Color::Rgb(6, 7, 8));
    }
}
