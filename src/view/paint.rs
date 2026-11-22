//! 色付け：Span 列を ANSI エスケープ付きの文字列にする。

use unicode_width::UnicodeWidthStr;

use super::span::{Fill, Role, Span};
use super::theme::Theme;

/// Span 列に色を付け、幅 `cols` まで背景色の空白で埋める。
///
/// 状態の色で塗る Span（[`Fill::State`]）は、文字を行の背景色にして色を反転させる。
/// 状態の色は前景色向けの色なので、背景にしたときの見え方はテーマ次第になる。
pub(super) fn paint(spans: &[Span], cols: usize, theme: &Theme) -> String {
    let line_bg = theme.bg.sgr(48);
    let mut out = String::new();
    let mut used = 0;
    for span in spans {
        let (fg, bold) = match span.role {
            Role::Plain | Role::App | Role::Cwd | Role::Msg => (theme.fg, false),
            Role::Name => (theme.fg, true),
            Role::State(state) => (theme.state(state), true),
            Role::BadgeEnd(state) => (theme.state(state), false),
        };
        let (fg, bg) = match span.fill {
            Fill::None => (fg.sgr(38), line_bg.clone()),
            Fill::State(state) => (theme.bg.sgr(38), theme.state(state).sgr(48)),
        };
        let bold = if bold { "1;" } else { "" };
        out.push_str(&format!("\x1b[0;{bold}{fg};{bg}m{}", span.text));
        used += span.text.width();
    }
    out.push_str(&format!(
        "\x1b[0;{line_bg}m{}\x1b[0m",
        " ".repeat(cols.saturating_sub(used))
    ));
    out
}
