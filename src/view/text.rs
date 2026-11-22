//! 文字列のユーティリティ：外部から届いた文字列の整形と、表示幅での切り詰め。

use std::path::Path;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::span::{Fill, Role, Span, width};

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

/// 先頭から幅 `width` に収まるだけの文字を取り出す。
fn take_width(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

/// 幅 `width` に収まらなければ、末尾を [`ELLIPSIS`] にして切り詰める。
pub(super) fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut text = take_width(text, width - 1);
    text.push(ELLIPSIS);
    text
}

/// 行全体が `cols` に収まらなければ、末尾を切って [`ELLIPSIS`] を付ける。
/// [`ELLIPSIS`] は切れた Span と同じ背景で塗る。バッジの途中で切れたときは、バッジの色の塊が
/// [`ELLIPSIS`] で終わる（右端の文字は付けない）ので、続きがあると分かる。
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
    let mut fill = Fill::None;
    for span in spans {
        let text = take_width(&span.text, left);
        let cut = text.len() < span.text.len();
        left -= text.width();
        let span_fill = span.fill;
        if !text.is_empty() {
            out.push(Span { text, ..span });
        }
        if cut {
            fill = span_fill;
            break;
        }
    }
    out.push(Span::new(ELLIPSIS, Role::Plain).with_fill(fill));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentState;

    #[test]
    fn ellipsis_is_one_column_wide() {
        assert_eq!(ELLIPSIS.width_cjk(), Some(1));
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
}
