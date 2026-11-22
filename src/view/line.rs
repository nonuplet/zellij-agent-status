//! 描画する 1 行：Span 列と行の幅。色を付けて出力するのと、クリックされた列のエージェントを引くのに使う。
//!
//! クリックの判定は、描画した Span 列そのものから行う。範囲の一覧を別に作ると、行末の切り詰め（`clamp`）の後で
//! 一覧も切り直す必要があり、表示とずれやすいため。

use unicode_width::UnicodeWidthStr;

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
    pub fn paint(&self, theme: &Theme) -> String {
        paint(&self.spans, self.cols, theme)
    }

    /// 列 `col`（0 始まり）に表示しているエージェントのペインの id。
    /// 行頭の余白や区切り、行末より右など、どのエージェントにも属さない列なら None。
    /// 幅 2 の文字は、どちらの列でもその文字の Span に当たる。
    pub fn pane_at(&self, col: usize) -> Option<u32> {
        let mut end = 0;
        for span in &self.spans {
            end += span.text.width();
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
    }
}
