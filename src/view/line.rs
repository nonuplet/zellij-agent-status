//! 描画する 1 行：Span 列と行の幅。色を付けて出力するのと、クリックされた列のエージェントを引くのに使う。
//!
//! クリックの判定は、描画した Span 列そのものから行う。範囲の一覧を別に作ると、行末の切り詰め（`clamp`）の後で
//! 一覧も切り直す必要があり、表示とずれやすいため。

use zellij_tile::graphemes::display_width;

use super::paint::paint;
use super::span::Span;
use super::theme::Theme;

/// [`render_line`](super::render_line) / [`render_message`](super::render_message) が作る 1 行。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    spans: Vec<Span>,
    cols: usize,
}

impl Line {
    pub(super) fn new(spans: Vec<Span>, cols: usize) -> Self {
        Line { spans, cols }
    }

    /// ANSI エスケープ付きの文字列にする。末尾は背景色の空白で埋める。
    /// `selected` が Some なら選択モードの色にする（行の色を変え、バッジは選んでいるエージェントだけに残す。[`paint`] を参照）。
    pub fn paint(&self, theme: &Theme, selected: Option<u32>) -> String {
        paint(&self.spans, self.cols, theme, selected)
    }

    /// 表示しているエージェントのペインの id を、左から順に並べたもの。キー操作で選べるのはこのエージェントだけ。
    /// 行末の切り詰めで一部だけ見えているエージェントも含む（クリックでもフォーカスできるため）。
    pub fn pane_ids(&self) -> Vec<u32> {
        let mut pane_ids: Vec<u32> = vec![];
        for pane_id in self.spans.iter().filter_map(|s| s.pane_id) {
            // 1 エージェント分の Span は続けて並ぶので、直前と同じかだけを見ればよい
            if pane_ids.last() != Some(&pane_id) {
                pane_ids.push(pane_id);
            }
        }
        pane_ids
    }

    /// 列 `col`（0 始まり）に表示しているエージェントのペインの id。
    /// 行頭の余白や区切り、行末より右など、どのエージェントにも属さない列なら None。
    /// 幅 2 の文字は、どちらの列でもその文字の Span に当たる。
    pub fn pane_at(&self, col: usize) -> Option<u32> {
        let mut end = 0;
        for span in &self.spans {
            end += display_width(&span.text);
            if col < end {
                return span.pane_id;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::span::Role;

    #[test]
    fn pane_at_finds_span_under_column() {
        let line = Line::new(
            vec![
                Span::new(" ", Role::Plain),
                Span::new("ab", Role::Name).with_pane(Some(1)),
                Span::new("  ", Role::Plain),
                Span::new("日本", Role::Name).with_pane(Some(2)),
            ],
            20,
        );
        let panes: Vec<Option<u32>> = (0..10).map(|col| line.pane_at(col)).collect();
        assert_eq!(
            panes,
            [
                None,
                Some(1),
                Some(1),
                None,
                None,
                // 幅 2 の文字は 2 列とも同じ Span に当たる
                Some(2),
                Some(2),
                Some(2),
                Some(2),
                // 行末より右（背景色の空白）
                None,
            ]
        );
    }

    #[test]
    fn empty_line_has_no_pane() {
        assert_eq!(Line::default().pane_at(0), None);
        assert!(Line::default().pane_ids().is_empty());
    }

    #[test]
    fn pane_ids_are_in_display_order() {
        let line = Line::new(
            vec![
                Span::new(" ", Role::Plain),
                Span::new("a", Role::Name).with_pane(Some(5)),
                Span::new("b", Role::Msg).with_pane(Some(5)),
                Span::new("  ", Role::Plain),
                Span::new("c", Role::Name).with_pane(Some(2)),
                Span::new("  ", Role::Plain),
                Span::new("d", Role::Name).with_pane(Some(9)),
            ],
            20,
        );
        assert_eq!(line.pane_ids(), [5, 2, 9]);
    }
}
