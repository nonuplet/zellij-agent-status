//! 幅の割り当て：各エージェントの名前・cwd・msg を、どの幅まで表示するかを決める。
//!
//! cwd はエージェントを見分けるためのものなので、縮めるときは msg より後まで残す。
//! 幅が足りないときは、次の順に縮める（[`shrink`]）。
//! 1. msg を各エージェントで均等に切り詰める（[`MIN_MSG_WIDTH`] を下回るなら次へ）
//! 2. msg を省き、cwd を均等に切り詰める（[`MIN_CWD_WIDTH`] を下回るなら次へ）
//! 3. cwd を省き、名前を均等に切り詰める（[`MIN_NAME_WIDTH`] を下回るなら次へ）
//! 4. 状態アイコン（と進捗）だけにする
//! 5. それでも収まらなければ、行の末尾を切る（[`clamp`](super::text::clamp)。末尾の
//!    [`ELLIPSIS`](super::text::ELLIPSIS) は切れた部分と同じ背景で塗る）
//!
//! [`ViewOptions::max_agent_width`](super::ViewOptions::max_agent_width) を指定しているときは、まず 1 エージェント分
//! （アイコン・進捗を含む）ずつ同じ手順でその幅に収め、そのうえで行の幅に収める（[`plan`]）。

use super::item::{Item, Widths};

/// 切り詰めた msg / cwd / 名前がこの幅を下回るなら、切り詰めずに次の段階へ進む
pub(super) const MIN_MSG_WIDTH: usize = 6;
pub(super) const MIN_CWD_WIDTH: usize = 4;
pub(super) const MIN_NAME_WIDTH: usize = 4;
/// [`ViewOptions::max_agent_width`](super::ViewOptions::max_agent_width) の下限。
/// 状態アイコン（絵文字で 2、それ以外は 1）とバッジの両端（絵文字以外で 2。端が Nerd Font の文字なら内側の空白を足して 4）、
/// 進捗（最大 ` 255%` の 5）に、名前の前の空白、[`MIN_NAME_WIDTH`]、名前の前の `app` のアイコンと前の空白
/// （[`ViewOptions::show_app_icon`](super::ViewOptions::show_app_icon) で 2）、`app` のアイコンの後ろの空白
/// （[`ViewOptions::nerd_font_icon_padding`](super::ViewOptions::nerd_font_icon_padding) で 1）を足した幅。
/// 最も広いのは、絵文字以外のアイコンで端を Nerd Font の文字にし（1 + 4）、`app` のアイコンとその後ろの空白を付けるとき。
/// 設定ごとに下限を変えると config の読み込みが設定の順番に依存するので、最も広い場合に合わせている。
/// Nerd Font の状態アイコンの後ろの空白は、進捗がないときだけ付けるので、進捗の最大幅を超えない。
///
/// これより小さい上限を許すと、次の問題が起きるため受け付けない（config で検証する）。
/// - アイコンと進捗だけで上限を超えることがある（例：端が Nerd Font の文字のバッジ + ` 100%` で 10）
/// - 上限に収めるために名前を省くことになり、エージェントを見分けられなくなる
pub const MIN_MAX_AGENT_WIDTH: usize = 18;

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

/// 各エージェントの各部分の幅の上限（`caps`）を、合計が `room` に収まるように縮める。
fn shrink(items: &[Item], caps: &[Widths], room: usize) -> Vec<Widths> {
    let names: Vec<Option<usize>> = caps.iter().map(|w| w.name).collect();
    let cwds: Vec<Option<usize>> = caps.iter().map(|w| w.cwd).collect();
    let msgs: Vec<Option<usize>> = caps.iter().map(|w| w.msg).collect();
    let hidden = vec![None; items.len()];

    let heads = items.iter().map(Item::head_width).sum::<usize>();
    let names_overhead = overhead(items, &names, Item::name_overhead);
    let names_total = total(&names) + names_overhead;
    let cwds_overhead = overhead(items, &cwds, Item::cwd_overhead);
    let cwds_total = total(&cwds) + cwds_overhead;
    let msgs_overhead = overhead(items, &msgs, Item::msg_overhead);

    if let Some(room) = room.checked_sub(heads + names_total + cwds_total + msgs_overhead)
        && let Some(msgs) = fit(&msgs, room, MIN_MSG_WIDTH)
    {
        return zip(names, cwds, msgs);
    }
    if let Some(room) = room.checked_sub(heads + names_total + cwds_overhead)
        && let Some(cwds) = fit(&cwds, room, MIN_CWD_WIDTH)
    {
        return zip(names, cwds, hidden);
    }
    if let Some(room) = room.checked_sub(heads + names_overhead)
        && let Some(names) = fit(&names, room, MIN_NAME_WIDTH)
    {
        return zip(names, hidden.clone(), hidden);
    }
    vec![Widths::default(); items.len()]
}

/// 表示する（Some の）部分の幅の合計
fn total(widths: &[Option<usize>]) -> usize {
    widths.iter().flatten().sum()
}

/// 表示する（Some の）部分に付く部分（`overhead`）の幅の合計
fn overhead(items: &[Item], widths: &[Option<usize>], overhead: fn(&Item) -> usize) -> usize {
    items
        .iter()
        .zip(widths)
        .filter(|(_, width)| width.is_some())
        .map(|(item, _)| overhead(item))
        .sum()
}

fn zip(
    names: Vec<Option<usize>>,
    cwds: Vec<Option<usize>>,
    msgs: Vec<Option<usize>>,
) -> Vec<Widths> {
    names
        .into_iter()
        .zip(cwds)
        .zip(msgs)
        .map(|((name, cwd), msg)| Widths { name, cwd, msg })
        .collect()
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
