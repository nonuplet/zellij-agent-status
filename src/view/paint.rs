//! 色付け：Span 列を ANSI エスケープ付きの文字列にする。

use unicode_width::UnicodeWidthStr;

use super::layout::{Role, Span};
use super::theme::Theme;

/// Span 列に色を付け、幅 `cols` まで背景色の空白で埋める。
pub(super) fn paint(spans: &[Span], cols: usize, theme: &Theme) -> String {
    let bg = theme.bg.sgr(48);
    let mut out = String::new();
    let mut used = 0;
    for span in spans {
        let (fg, bold) = match span.role {
            Role::Plain | Role::Cwd | Role::Msg => (theme.fg, false),
            Role::Name => (theme.fg, true),
            Role::State(state) => (theme.state(state), true),
        };
        let bold = if bold { "1;" } else { "" };
        out.push_str(&format!("\x1b[0;{bold}{};{bg}m{}", fg.sgr(38), span.text));
        used += span.text.width();
    }
    out.push_str(&format!(
        "\x1b[0;{bg}m{}\x1b[0m",
        " ".repeat(cols.saturating_sub(used))
    ));
    out
}
