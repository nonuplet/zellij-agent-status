//! 表示：エージェントの一覧と幅から、1 行分の文字列（ANSI エスケープ付き）を作る。
//!
//! 副作用のない関数だけで構成し、`render` からは [`render_line`] の結果を出力するだけにする。
//! 表示するエージェントの絞り込みと並び順も、表示の都合としてここで決める（Collector は順序を持たない）。
//!
//! - [`layout`](mod@layout)：どのエージェントを、どの順で並べるか。1 行分の Span 列を組み立てる
//!   - [`item`]：1 エージェント分の内容と Span の組み立て
//!   - [`badge`]：状態バッジにする範囲と、バッジの端の文字
//!   - [`fit`]：幅に合わせた切り詰め（どの部分を、どの順に縮めるか）
//! - [`paint`](mod@paint)：色付け
//! - [`span`]：layout と paint の間の中間表現
//! - [`text`]：文字列の整形と、表示幅での切り詰め
//! - [`options`]：表示の設定
//! - [`theme`] / [`icon`]：色・アイコンの定義

mod badge;
mod fit;
mod icon;
mod item;
mod layout;
mod options;
mod paint;
mod span;
mod text;
mod theme;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use badge::BadgeShape;
pub use fit::MIN_MAX_LENGTH;
pub use icon::IconStyle;
pub use options::{DEFAULT_MAX_LENGTH, MsgMode, ViewOptions};
pub use theme::Theme;

use crate::model::AgentStatus;
use layout::{PAD, layout};
use paint::paint;
use span::{Role, Span};
use text::{clamp, clean};

/// 幅 `cols` の 1 行を作る。末尾は背景色の空白で埋める。
/// `agents` の順序は問わない（並び順は [`layout()`] で決める）。
pub fn render_line<'a>(
    agents: impl IntoIterator<Item = &'a AgentStatus>,
    cols: usize,
    theme: &Theme,
    options: &ViewOptions,
) -> String {
    paint(&layout(agents, cols, options), cols, theme)
}

/// エージェント一覧の代わりに出す 1 行のメッセージ（permission の案内や設定の警告）を作る。
/// 幅を超える分は切り詰め、末尾は背景色の空白で埋める。
pub fn render_message(text: &str, cols: usize, theme: &Theme) -> String {
    let spans = match clean(Some(text)) {
        Some(text) => clamp(
            vec![Span::new(PAD, Role::Plain), Span::new(text, Role::Msg)],
            cols,
        ),
        None => vec![],
    };
    paint(&spans, cols, theme)
}
