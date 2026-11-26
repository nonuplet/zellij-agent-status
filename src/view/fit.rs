//! 幅の割り当て：各エージェントの要素と msg を、どの幅まで表示するかを決める。
//!
//! 幅が足りないときは、一覧の順番やバッジの中か外かに関係なく、[`Slot::ALL`] の順
//! （msg → cwd → `app_name` → `tab_name` → `title` → 最後に残る要素）に縮める（[`shrink`]）。
//! 1. その Slot を各エージェントで均等に切り詰める（[`min_width`] を下回るなら次へ）
//! 2. その Slot を省き、次の Slot で 1. を繰り返す
//! 3. 最後に残る要素（[`Slot::Last`]。`Item::new` を参照）も収まらなければ、状態アイコン（と進捗）だけにする
//! 4. それでも収まらなければ、行の末尾を切る（[`clamp`](super::text::clamp)。末尾の
//!    [`ELLIPSIS`](super::text::ELLIPSIS) は切れた部分と同じ背景で塗る）
//!
//! 最後に残る要素を分けているのは、`app_name` しかないエージェントの名前が、`title` などより先に消えないようにするため。
//!
//! [`ViewOptions::max_agent_width`](super::ViewOptions::max_agent_width) を指定しているときは、まず 1 エージェント分
//! （アイコン・進捗を含む）ずつ同じ手順でその幅に収め、そのうえで行の幅に収める（[`plan`]）。

use super::item::{Item, Slot, Widths};

/// 切り詰めた msg / cwd / ほかの要素がこの幅を下回るなら、切り詰めずに次の段階へ進む
pub(super) const MIN_MSG_WIDTH: usize = 6;
pub(super) const MIN_CWD_WIDTH: usize = 4;
pub(super) const MIN_NAME_WIDTH: usize = 4;
/// [`ViewOptions::max_agent_width`](super::ViewOptions::max_agent_width) の下限。
/// 状態アイコン（絵文字で 2、それ以外は 1）とバッジの両端（絵文字以外で 2。端が Nerd Font の文字なら内側の空白を足して 4）、
/// 進捗（最大 ` 255%` の 5）に、最後に残る要素の前後（cwd なら ` (` と `)` の 3、ほかは前の空白の 1）、[`MIN_NAME_WIDTH`]、
/// `app` のアイコンと前の空白（2）、`app` のアイコンの後ろの空白
/// （[`ViewOptions::nerd_font_icon_padding`](super::ViewOptions::nerd_font_icon_padding) で 1）を足した幅。
/// 最も広いのは、絵文字以外のアイコンで端を Nerd Font の文字にし（1 + 4）、`app` のアイコンとその後ろの空白を付け、
/// cwd が最後に残るとき。
/// 設定ごとに下限を変えると config の読み込みが設定の順番に依存するので、最も広い場合に合わせている。
/// Nerd Font の状態アイコンの後ろの空白は、進捗がないときだけ付けるので、進捗の最大幅を超えない。
///
/// これより小さい上限を許すと、次の問題が起きるため受け付けない（config で検証する）。
/// - アイコンと進捗だけで上限を超えることがある（例：端が Nerd Font の文字のバッジ + ` 100%` で 10）
/// - 上限に収めるために最後に残る要素を省くことになり、エージェントを見分けられなくなる
pub const MIN_MAX_AGENT_WIDTH: usize = 20;

/// 切り詰めた `slot` の幅の下限。最後に残る要素は、元の要素によらず [`MIN_NAME_WIDTH`]
fn min_width(slot: Slot) -> usize {
    match slot {
        Slot::Msg => MIN_MSG_WIDTH,
        Slot::Cwd => MIN_CWD_WIDTH,
        Slot::AppName | Slot::TabName | Slot::Title | Slot::Last => MIN_NAME_WIDTH,
    }
}

/// 各エージェントの各部分の幅を決める（縮める順はモジュールの説明を参照）。
/// `line_overhead` は行頭の余白とエージェント同士の区切りの幅の合計。
pub(super) fn plan(
    items: &[Item],
    cols: usize,
    line_overhead: usize,
    max_agent_width: Option<usize>,
) -> Vec<Widths> {
    let caps: Vec<Widths> = items
        .iter()
        .map(|item| match max_agent_width {
            Some(max_agent_width) => shrink(
                std::slice::from_ref(item),
                &[item.widths()],
                max_agent_width,
            )[0],
            None => item.widths(),
        })
        .collect();
    match cols.checked_sub(line_overhead) {
        Some(room) => shrink(items, &caps, room),
        None => vec![Widths::default(); items.len()],
    }
}

/// 各エージェントの各 Slot の幅の上限（`caps`）を、合計が `room` に収まるように縮める。
/// 前の Slot から順に省いていくので、ある Slot を切り詰めるときは、それより前の Slot はすべて省いてあり、
/// 後ろの Slot はすべて上限の幅のまま。
fn shrink(items: &[Item], caps: &[Widths], room: usize) -> Vec<Widths> {
    let heads = items.iter().map(Item::head_width).sum::<usize>();
    let mut widths = caps.to_vec();
    for slot in Slot::ALL {
        // slot は切り詰めるので、付く部分の幅だけを数える
        let fixed: usize = items
            .iter()
            .zip(&widths)
            .map(|(item, w)| {
                Slot::ALL
                    .into_iter()
                    .filter_map(|s| {
                        let width = w.get(s)?;
                        Some(item.overhead(s) + if s == slot { 0 } else { width })
                    })
                    .sum::<usize>()
            })
            .sum();
        let current: Vec<Option<usize>> = widths.iter().map(|w| w.get(slot)).collect();
        if let Some(room) = room.checked_sub(heads + fixed)
            && let Some(shares) = fit(&current, room, min_width(slot))
        {
            for (w, share) in widths.iter_mut().zip(shares) {
                w.set(slot, share);
            }
            return widths;
        }
        for w in &mut widths {
            w.set(slot, None);
        }
    }
    widths
}

/// `room` を [`fair_share`] で分け、切り詰めた結果が `min` を下回るものがあれば None。
fn fit(widths: &[Option<usize>], room: usize, min: usize) -> Option<Vec<Option<usize>>> {
    let shares = fair_share(widths, room);
    // Some の位置は widths と shares で同じ
    let ok = widths
        .iter()
        .flatten()
        .zip(shares.iter().flatten())
        .all(|(&width, &share)| share >= width || share >= min);
    ok.then_some(shares)
}

/// 合計 `total` の幅を、表示する（Some の）要素に、その幅を上限として均等に分ける。
/// 短いものに余った分は、長いものに回す。None の要素は None のまま。
fn fair_share(widths: &[Option<usize>], total: usize) -> Vec<Option<usize>> {
    let mut shown: Vec<(usize, usize)> = widths
        .iter()
        .enumerate()
        .filter_map(|(i, width)| width.map(|w| (i, w)))
        .collect();
    shown.sort_by_key(|&(_, width)| width);
    let mut shares = vec![None; widths.len()];
    let mut left = total;
    for (k, &(i, width)) in shown.iter().enumerate() {
        let share = (left / (shown.len() - k)).min(width);
        shares[i] = Some(share);
        left -= share;
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_share_gives_leftover_to_longer_items() {
        assert_eq!(
            fair_share(&[Some(3), Some(20), Some(20)], 20),
            vec![Some(3), Some(8), Some(9)]
        );
        assert_eq!(fair_share(&[Some(3), Some(4)], 100), vec![Some(3), Some(4)]);
        assert_eq!(fair_share(&[], 10), Vec::<Option<usize>>::new());
    }

    #[test]
    fn fair_share_skips_hidden_items() {
        assert_eq!(
            fair_share(&[Some(20), None, Some(3), None, Some(20)], 20),
            vec![Some(8), None, Some(3), None, Some(9)]
        );
        assert_eq!(fair_share(&[None, None], 10), vec![None, None]);
    }
}
