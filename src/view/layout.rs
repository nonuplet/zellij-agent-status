//! レイアウト：どのエージェントを、どの順で並べるかを決め、1 行分の Span 列を組み立てる。
//!
//! 1 エージェント分の内容は [`item`](super::item)、バッジにする範囲は [`badge`](super::badge)、
//! 幅に合わせた縮め方は [`fit`](super::fit) で決める。

use unicode_width::UnicodeWidthStr;

use super::ViewOptions;
use super::badge::BadgeStyle;
use super::fit::plan;
use super::item::Item;
use super::span::{Role, Span};
use super::text::clamp;
use crate::model::{AgentState, AgentStatus};

/// 行頭の余白（[`render_message`](super::render_message) でも使う）
pub(super) const PAD: &str = " ";
/// エージェント同士の区切り。バッジの端の形によらず同じ（エージェントの境目はバッジで見分けられるため）
const SEPARATOR: &str = "  ";

/// [`ViewOptions::shows`] が true のエージェントだけを、(タブ, ペイン) の順に並べる。
///
/// 受け取る一覧の順序には依存しない（Collector は順序を持たない）。
/// 状態の優先度で並べ替えると、状態が変わるたびに位置が入れ替わって追いにくいため、既定では状態を見ない。
/// [`ViewOptions::blocked_first`] が true のときだけ、見落としやすく省略されやすい後ろのほうに
/// 操作待ちが埋もれないよう、`Blocked` を先頭に割り込ませる。`Blocked` 同士、それ以外同士は (タブ, ペイン) の順のまま。
/// `Error` などは操作待ちではないので割り込ませない。
pub(super) fn layout<'a>(
    agents: impl IntoIterator<Item = &'a AgentStatus>,
    cols: usize,
    options: &ViewOptions,
) -> Vec<Span> {
    let mut agents: Vec<&AgentStatus> = agents.into_iter().filter(|a| options.shows(a)).collect();
    agents.sort_by_key(|a| {
        let blocked = options.blocked_first && a.state == AgentState::Blocked;
        (!blocked, a.tab, a.pane_id)
    });
    let style = BadgeStyle::new(options);
    let items: Vec<Item> = agents
        .into_iter()
        .map(|a| Item::new(a, options, &style))
        .collect();
    if items.is_empty() {
        return vec![];
    }

    let mut spans = vec![Span::new(PAD, Role::Plain)];
    let line_overhead = PAD.width() + SEPARATOR.width() * (items.len() - 1);
    let plans = plan(&items, cols, line_overhead, options.max_length);
    for (i, (item, widths)) in items.iter().zip(plans).enumerate() {
        if i > 0 {
            spans.push(Span::new(SEPARATOR, Role::Plain));
        }
        spans.extend(item.spans(widths));
    }
    clamp(spans, cols)
}

#[cfg(test)]
mod tests;
