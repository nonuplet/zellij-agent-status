//! 色付け：Span 列を ANSI エスケープ付きの文字列にする。

use std::fmt::Write;

use zellij_tile::graphemes::display_width;

use super::span::{Fill, Role, Span};
use super::theme::{Color, Theme};

/// Span 列に色を付け、幅 `cols` まで背景色の空白で埋める。
///
/// 状態の色で塗る Span（[`Fill::State`]）は、文字を行の背景色にして色を反転させる。
/// 状態の色は前景色向けの色なので、背景にしたときの見え方はテーマ次第になる。
///
/// `selected` が Some なら選択モード中で、中身は選んでいるエージェントのペインの id。選択モード中は次のように描く。
/// - 行全体（バッジ以外）の文字色と背景色を [`Theme::select_fg`] / [`Theme::select_bg`] にする。
///   行全体の色が変わるので、選択モードだと分かる。バッジの端の文字の外側も同じ背景色になる
/// - 選んでいないエージェントはバッジを外す（状態の色で塗らず、状態の色の文字にする）。
///   バッジの端の文字は背景と同じ色にして見えなくする
/// - 選んでいるエージェントがバッジにならない（絵文字アイコン）なら、バッジの代わりに、文字色と背景色を反転させて塗る
///   （[`Fill::Selected`]）。状態の色で塗ると、色の近い絵文字が見えにくくなるため。
///   `highlight_attention` で強調するエージェントは、絵文字アイコンでも普段から状態の色で塗るので、反転させない
///
/// 色の塊で描くのは選んでいるエージェントだけになるので、msg の有無やバッジの範囲（`badge_items` など）に
/// 関係なく、どれを選んでいるかが分かる。選んでいるエージェントのバッジは普段と同じ色で描く
/// （反転すると、バッジが背景と同化して見づらくなるため）。
pub(super) fn paint(spans: &[Span], cols: usize, theme: &Theme, selected: Option<u32>) -> String {
    let (text_fg, line_bg): (Color, Color) = if selected.is_some() {
        (theme.select_fg, theme.select_bg)
    } else {
        (theme.fg, theme.bg)
    };

    let mut out = String::new();
    let mut used = 0;
    for span in spans {
        // 選択モード中で、選んでいるエージェントの部分でなければ true（バッジを外す）
        let dimmed = selected.is_some() && span.pane_id != selected;
        let (fg, bold) = match span.role {
            Role::Plain | Role::App | Role::Cwd | Role::Msg => (text_fg, false),
            Role::Name => (text_fg, true),
            Role::State(state) => (theme.state(state), true),
            // バッジを外したエージェントでは、端の文字は背景に溶け込ませて空白に見せる（幅は変えない）
            Role::BadgeEnd(_) if dimmed => (line_bg, false),
            Role::BadgeEnd(state) => (theme.state(state), false),
        };
        let (fg, bg) = match span.fill {
            Fill::State(state) if !dimmed => (theme.bg.fg(), theme.state(state).bg()),
            Fill::Selected if selected.is_some() && !dimmed => (line_bg.fg(), text_fg.bg()),
            Fill::None | Fill::State(_) | Fill::Selected => (fg.fg(), line_bg.bg()),
        };
        let bold = if bold { "1;" } else { "" };
        // String への write! は失敗しないので、結果は捨てる
        let _ = write!(out, "\x1b[0;{bold}{fg};{bg}m{}", span.text);
        used += display_width(&span.text);
    }
    let _ = write!(
        out,
        "\x1b[0;{}m{}\x1b[0m",
        line_bg.bg(),
        " ".repeat(cols.saturating_sub(used))
    );
    out
}
