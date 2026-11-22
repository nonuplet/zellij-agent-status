//! 幅の割り当て：各エージェントの名前・cwd・msg を、どの幅まで表示するかを決める。
//!
//! cwd はエージェントを見分けるためのものなので、縮めるときは msg より後まで残す。
//!
//! [`ViewOptions::max_length`](super::ViewOptions::max_length) を指定しているときは、まず 1 エージェント分
//! （アイコン・進捗を含む）をその幅に収める（[`Item::widths`]）。msg を切り詰め、[`MIN_MSG_WIDTH`] を
//! 下回るなら msg を省いて cwd を切り詰め、[`MIN_CWD_WIDTH`] を下回るなら cwd も省いて名前を切り詰める。
//!
//! そのうえで行の幅が足りないときは、次の順に縮める（[`plan`]）。
//! 1. msg を各エージェントで均等に切り詰める
//! 2. msg を省き、cwd を均等に切り詰める
//! 3. cwd を省き、名前を均等に切り詰める
//! 4. 状態アイコン（と進捗）だけにする
//! 5. それでも収まらなければ、行の末尾を切る（[`clamp`](super::text::clamp)。末尾の
//!    [`ELLIPSIS`](super::text::ELLIPSIS) は切れた部分と同じ背景で塗る）

use unicode_width::UnicodeWidthStr;

use super::item::{CWD_OVERHEAD, Item};
use super::layout::PAD;

/// 切り詰めた msg / cwd / 名前がこの幅を下回るなら、切り詰めずに次の段階へ進む
pub(super) const MIN_MSG_WIDTH: usize = 6;
pub(super) const MIN_CWD_WIDTH: usize = 4;
pub(super) const MIN_NAME_WIDTH: usize = 4;
/// [`ViewOptions::max_length`](super::ViewOptions::max_length) の下限。
/// 状態アイコン（絵文字で 2、それ以外は 1）とバッジの両端（絵文字以外で 2。Nerd Font モードでは両端の内側の空白を足して 4）、
/// 進捗（最大 ` 255%` の 5）に、名前の前の空白、[`MIN_NAME_WIDTH`]、名前の前の `app` のアイコンと前の空白
/// （Nerd Font モードで 2）、アイコンの後ろの空白
/// （[`ViewOptions::nerd_font_icon_padding`](super::ViewOptions::nerd_font_icon_padding) で 1）を足した幅。
/// 最も広いのは Nerd Font モードで絵文字以外のアイコン（1 + 4）を選び、アイコンの後ろに空白を置くとき。
/// 設定ごとに下限を変えると config の読み込みが設定の順番に依存するので、最も広い場合に合わせている。
/// Nerd Font の状態アイコンの後ろの空白は、進捗がないときだけ付けるので、進捗の最大幅を超えない。
///
/// これより小さい上限を許すと、次の問題が起きるため受け付けない（config で検証する）。
/// - アイコンと進捗だけで上限を超えることがある（例：Nerd Font モードのバッジ + ` 100%` で 10）
/// - 上限に収めるために、名前が [`MIN_NAME_WIDTH`] より短く切り詰められて見分けられなくなる
pub const MIN_MAX_LENGTH: usize = 18;

/// 各部分を表示する幅の上限。None なら表示しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Widths {
    pub(super) name: Option<usize>,
    pub(super) cwd: Option<usize>,
    pub(super) msg: Option<usize>,
}

/// 各エージェントの各部分の幅を決める（縮める順はモジュールの説明を参照）。
/// [`Item::widths`] で `max_length` に収めた幅を、行の幅に合わせてさらに縮める。
/// `sep_width` はエージェント同士の区切りの幅。
pub(super) fn plan(
    items: &[Item],
    cols: usize,
    sep_width: usize,
    max_length: Option<usize>,
) -> Vec<Widths> {
    let fixed = PAD.width()
        + sep_width * (items.len() - 1)
        + items.iter().map(Item::head_width).sum::<usize>();
    let caps: Vec<Widths> = items.iter().map(|i| i.widths(max_length)).collect();
    // 名前は常に表示する前提（Item::widths で常に Some）
    let names = shown(&caps, |w| w.name);
    // 名前の前の空白と後ろの app のアイコン
    let names_overhead = items.iter().map(Item::name_overhead).sum::<usize>();
    let names_total = names.iter().sum::<usize>() + names_overhead;
    let cwds = shown(&caps, |w| w.cwd);
    let cwds_overhead = CWD_OVERHEAD * cwds.len();
    let cwds_total = cwds.iter().sum::<usize>() + cwds_overhead;
    let msgs = shown(&caps, |w| w.msg);
    // 区切り（Item::msg_separator）の幅はエージェントごとに違うので、msg を表示するものだけ足す
    let msgs_overhead = items
        .iter()
        .zip(&caps)
        .filter(|(_, cap)| cap.msg.is_some())
        .map(|(item, _)| item.msg_separator().width())
        .sum::<usize>();

    if let Some(room) = cols.checked_sub(fixed + names_total + cwds_total + msgs_overhead)
        && let Some(msgs) = fit(&msgs, room, MIN_MSG_WIDTH)
    {
        return assemble(&caps, Some(names), Some(cwds), Some(msgs));
    }

    if let Some(room) = cols.checked_sub(fixed + names_total + cwds_overhead)
        && let Some(cwds) = fit(&cwds, room, MIN_CWD_WIDTH)
    {
        return assemble(&caps, Some(names), Some(cwds), None);
    }

    if let Some(room) = cols.checked_sub(fixed + names_overhead)
        && let Some(names) = fit(&names, room, MIN_NAME_WIDTH)
    {
        return assemble(&caps, Some(names), None, None);
    }

    assemble(&caps, None, None, None)
}

/// `part` を表示するエージェントについての、その幅の一覧。
fn shown(caps: &[Widths], part: impl Fn(&Widths) -> Option<usize>) -> Vec<usize> {
    caps.iter().filter_map(part).collect()
}

/// [`plan`] の各段階の結果を、エージェントごとの [`Widths`] にする。
/// `names` / `cwds` / `msgs` は、`caps`（[`Item::widths`] の結果）でその部分を表示するエージェントの分だけを並べたもの。
/// None はその部分を表示しない。
fn assemble(
    caps: &[Widths],
    names: Option<Vec<usize>>,
    cwds: Option<Vec<usize>>,
    msgs: Option<Vec<usize>>,
) -> Vec<Widths> {
    let mut names = names.map(Vec::into_iter);
    let mut cwds = cwds.map(Vec::into_iter);
    let mut msgs = msgs.map(Vec::into_iter);
    let next = |cap: Option<usize>, shares: &mut Option<std::vec::IntoIter<usize>>| {
        cap.and_then(|_| shares.as_mut().and_then(Iterator::next))
    };
    caps.iter()
        .map(|cap| Widths {
            name: next(cap.name, &mut names),
            cwd: next(cap.cwd, &mut cwds),
            msg: next(cap.msg, &mut msgs),
        })
        .collect()
}

/// `room` を [`fair_share`] で分け、切り詰めた結果が `min` を下回るものがあれば None。
fn fit(widths: &[usize], room: usize, min: usize) -> Option<Vec<usize>> {
    let shares = fair_share(widths, room);
    let ok = widths
        .iter()
        .zip(&shares)
        .all(|(&width, &share)| share >= width || share >= min);
    ok.then_some(shares)
}

/// 合計 `total` の幅を、各要素の幅（`widths`）を上限として均等に分ける。
/// 短いものに余った分は、長いものに回す。
fn fair_share(widths: &[usize], total: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..widths.len()).collect();
    order.sort_by_key(|&i| widths[i]);
    let mut shares = vec![0; widths.len()];
    let mut left = total;
    for (k, &i) in order.iter().enumerate() {
        let share = (left / (order.len() - k)).min(widths[i]);
        shares[i] = share;
        left -= share;
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_share_gives_leftover_to_longer_items() {
        assert_eq!(fair_share(&[3, 20, 20], 20), vec![3, 8, 9]);
        assert_eq!(fair_share(&[3, 4], 100), vec![3, 4]);
        assert_eq!(fair_share(&[], 10), Vec::<usize>::new());
    }
}
