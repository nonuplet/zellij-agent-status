//! 文字列のユーティリティ：外部から届いた文字列の整形と、表示幅での切り詰め。
//!
//! 表示幅は、view 全体で [`zellij_tile::graphemes`] で数える（書記素クラスタごとの幅の合計）。
//! Zellij がプラグインのペインの桁を書記素クラスタ単位で数えるのに合わせている。
//! `unicode-width` で文字列や `char` の幅を直接数えると、`❤️` などの絵文字や合字でこれとずれるので、混ぜて使わない。

use std::path::Path;

use zellij_tile::graphemes::{display_width, prefix_within};

use super::span::{Role, Span, width};

/// 切り詰めたことを示す記号。
/// `…`（U+2026）は East Asian Width が Ambiguous で、端末によっては 2 桁になるため使わない。
pub(super) const ELLIPSIS: char = '~';

/// 外部から届いた文字列を 1 行の表示用にする。
/// 制御文字（改行や ESC など）はそのまま出すと画面が崩れるので空白に置き換える。
/// 空になったら None。
pub(super) fn clean(text: &str) -> Option<String> {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// パスの末尾のディレクトリ名。ルート（`/`）のように名前がなければ、パス全体を使う。
/// フルパスは長く、ホームを `~` に縮めるには環境変数を読む permission が要るため、末尾だけにしている。
pub(super) fn dir_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// 幅 `width` に収まらなければ、末尾を [`ELLIPSIS`] にして切り詰める。
pub(super) fn truncate(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let (head, _) = prefix_within(text, width - 1);
    format!("{head}{ELLIPSIS}")
}

/// 行全体が `cols` に収まらなければ、末尾を切って [`ELLIPSIS`] を付ける。
/// [`ELLIPSIS`] は切れた Span と同じ背景で塗る。バッジの途中で切れたときは、バッジの色の塊が
/// [`ELLIPSIS`] で終わる（右端の文字は付けない）ので、続きがあると分かる。
/// [`ELLIPSIS`] は切れた Span のペインも引き継ぎ、クリックすると切れたエージェントのペインにフォーカスする。
///
/// 切り詰めた Span は、文字列以外のフィールドを元の Span から引き継ぐ。
pub(super) fn clamp(spans: Vec<Span>, cols: usize) -> Vec<Span> {
    if width(&spans) <= cols {
        return spans;
    }
    if cols == 0 {
        return vec![];
    }
    let mut left = cols - 1;
    let mut out = vec![];
    let mut ellipsis = Span::new(ELLIPSIS, Role::Plain);
    for span in spans {
        let (head, used) = prefix_within(&span.text, left);
        let cut = head.len() < span.text.len();
        left -= used;
        let (fill, pane_id) = (span.fill, span.pane_id);
        if !head.is_empty() {
            let text = head.to_owned();
            out.push(Span { text, ..span });
        }
        if cut {
            ellipsis = ellipsis.with_fill(fill).with_pane(pane_id);
            break;
        }
    }
    out.push(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthChar;
    use zellij_tile::graphemes::clusters;

    use super::*;
    use crate::model::AgentState;
    use crate::view::span::Fill;

    /// 複数の `char` で 1 文字になるもの。多くは、`char` ごとの幅の合計と表示幅が違う
    const CLUSTERS: [&str; 9] = [
        "❤\u{fe0f}",                // 異体字セレクタ（絵文字にする）
        "⌚\u{fe0e}",               // 異体字セレクタ（絵文字にしない）
        "👨\u{200d}👩\u{200d}👧",   // ZWJ で結合した絵文字
        "🏳\u{fe0f}\u{200d}🌈",      // 異体字セレクタ + ZWJ
        "🇯🇵",                       // 国旗
        "👍🏽",                       // 肌の色
        "1\u{fe0f}\u{20e3}",        // キーキャップ
        "か\u{3099}",               // 結合文字（濁点）
        "\u{1112}\u{1161}\u{11ab}", // ハングルの字母
    ];

    #[test]
    fn ellipsis_is_one_column_wide() {
        assert_eq!(ELLIPSIS.width_cjk(), Some(1));
    }

    /// `text` を切り詰めた `head` が、`text` の先頭からクラスタ単位で切ったものか
    fn assert_cut_between_clusters(text: &str, head: &str) {
        let expected: String = clusters(text)
            .map(|(cluster, _)| cluster)
            .take(clusters(head).count())
            .collect();
        assert_eq!(head, expected, "{text:?}");
    }

    #[test]
    fn truncate_fits_width_and_never_splits_clusters() {
        for cluster in CLUSTERS {
            let text = cluster.repeat(8);
            let full = display_width(&text);
            assert_eq!(truncate(&text, full), text);
            assert_eq!(truncate(&text, 0), "");
            for width in 1..full {
                let cut = truncate(&text, width);
                let cut_width = display_width(&cut);
                // 入りきらない文字の手前で切るので、幅 2 の文字なら 1 桁余ることがある
                assert!(
                    cut_width <= width && cut_width + 1 >= width,
                    "width={width}: {cut:?}"
                );
                assert_cut_between_clusters(&text, cut.strip_suffix(ELLIPSIS).unwrap());
            }
        }
    }

    #[test]
    fn clamp_fits_width_and_never_splits_clusters() {
        for cluster in CLUSTERS {
            let text = cluster.repeat(4);
            let full = display_width(&text) * 2;
            for cols in 0..full {
                // 前の Span で残りの幅を使い切っても、後ろの Span の途中で切れても収まること
                let spans = vec![
                    Span::new(text.as_str(), Role::Name),
                    Span::new(text.as_str(), Role::Msg),
                ];
                let out = clamp(spans, cols);
                assert!(width(&out) <= cols, "cols={cols}: {out:?}");
                for span in out.iter().filter(|s| s.role != Role::Plain) {
                    assert_cut_between_clusters(&text, &span.text);
                }
            }
        }
    }

    #[test]
    fn clamp_fills_ellipsis_like_cut_span() {
        let fill = Fill::State(AgentState::Error);
        let spans = vec![
            Span::new("abc", Role::Msg).with_fill(fill),
            Span::new("de", Role::Plain),
        ];
        // 塗った部分の途中で切れたら、~ も塗る
        assert_eq!(
            clamp(spans.clone(), 3),
            [
                Span::new("ab", Role::Msg).with_fill(fill),
                Span::new("~", Role::Plain).with_fill(fill),
            ]
        );
        // 塗っていない部分で切れたら、~ は塗らない
        assert_eq!(
            clamp(spans, 4),
            [
                Span::new("abc", Role::Msg).with_fill(fill),
                Span::new("~", Role::Plain),
            ]
        );
    }

    #[test]
    fn clamp_gives_ellipsis_pane_of_cut_span() {
        let spans = vec![
            Span::new("abc", Role::Name).with_pane(Some(1)),
            Span::new("  ", Role::Plain),
            Span::new("de", Role::Name).with_pane(Some(2)),
        ];
        // エージェントの途中で切れたら、~ もそのエージェントの部分にする
        assert_eq!(
            clamp(spans.clone(), 3),
            [
                Span::new("ab", Role::Name).with_pane(Some(1)),
                Span::new("~", Role::Plain).with_pane(Some(1)),
            ]
        );
        // 区切りで切れたら、~ はどのエージェントの部分でもない
        assert_eq!(
            clamp(spans, 5),
            [
                Span::new("abc", Role::Name).with_pane(Some(1)),
                Span::new(" ", Role::Plain),
                Span::new("~", Role::Plain),
            ]
        );
    }
}
