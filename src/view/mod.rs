//! 表示：エージェントの一覧と幅から、1 行分の [`Line`] を作る。
//!
//! 副作用のない関数だけで構成し、`render` からは [`render_line`] の結果を [`Line::paint`] して出力するだけにする。
//! クリックされた列のエージェントも、描画した [`Line`] から引く（[`Line::pane_at`]）。
//! キー操作で選べるエージェントも同じく [`Line`] から引き（[`Line::pane_ids`]）、選択モード中は [`Line::paint`] で行の色を変え、選んでいるエージェントだけを色の塊（バッジなど）で描く。
//! 表示するエージェントの絞り込みと並び順も、表示の都合としてここで決める（Collector は順序を持たない）。
//!
//! - [`layout`](mod@layout)：どのエージェントを、どの順で並べるか。1 行分の Span 列を組み立てる
//!   - [`item`]：1 エージェント分の内容と Span の組み立て
//!   - [`badge`]：状態バッジにする範囲と、バッジの端の文字
//!   - [`fit`]：幅に合わせた切り詰め（どの部分を、どの順に縮めるか）
//! - [`line`](mod@line)：描画する 1 行。色付けとクリックの判定
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
mod line;
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
pub use fit::MIN_MAX_AGENT_WIDTH;
pub use icon::StateIconStyle;
pub use line::Line;
pub use options::{DEFAULT_MAX_AGENT_WIDTH, MsgMode, ViewOptions};
pub use theme::Theme;

use crate::model::AgentStatus;
use layout::{PAD, layout};
use span::{Role, Span};
use text::{clamp, clean};

/// 幅 `cols` の 1 行を作る。
/// `agents` の順序は問わない（並び順は [`layout()`] で決める）。
pub fn render_line<'a>(
    agents: impl IntoIterator<Item = &'a AgentStatus>,
    cols: usize,
    options: &ViewOptions,
) -> Line {
    Line::new(layout(agents, cols, options), cols)
}

/// エージェント一覧の代わりに出す 1 行のメッセージ（permission の案内や設定の警告）を作る。
/// 幅を超える分は切り詰める。どのエージェントにも属さないので、クリックしても何も起きない。
pub fn render_message(text: &str, cols: usize) -> Line {
    let spans = match clean(text) {
        Some(text) => clamp(
            vec![Span::new(PAD, Role::Plain), Span::new(text, Role::Msg)],
            cols,
        ),
        None => vec![],
    };
    Line::new(spans, cols)
}
