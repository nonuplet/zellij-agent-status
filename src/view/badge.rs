//! 状態バッジ：状態アイコンから名前（と `app` のアイコン・cwd）までを、状態の色で背景を塗ったバッジ
//! （`[↻ 42% 名前 (cwd)] msg`）にする。Span の組み立ては [`Item::spans`](super::item::Item::spans) で行う。
//!
//! バッジの右端が名前と msg の境目になるので、間は `: ` ではなく空白 1 つにする
//! （[`Item::msg_separator`](super::item::Item::msg_separator)）。
//! バッジの両端には端の文字（[`BadgeEnds`]。Nerd Font モードでは [`ViewOptions::badge_shape`] の形）を付ける。
//! Nerd Font モードでは、両端の文字の内側（左端の後ろと右端の手前）に塗った空白を 1 つずつ置く（[`BadgeEnds::inner_pad`]）。
//! 絵文字アイコンは自分の色を持っていて、背景も塗ると色が二重になるので、バッジにせず端の文字も付けない。
//!
//! 名前までバッジに入れるのは、アイコンの右がバッジの右端の文字に重なったり切れたりするのを避けるため。
//! Mono 版でない Nerd Font などでは、アイコンが 1 桁からはみ出して 1.5〜2 桁ほどの幅で描かれることがある
//! （記号のアイコンでも右の余白が少ないものがある）。アイコンの後ろに文字が続けば、はみ出した分は
//! 次の空白に重なるだけで目立たない。`app` のアイコンを名前の前に置くのも同じ理由で、バッジの末尾がアイコンにならないようにしている。
//! [`ViewOptions::badge_name`] が false なら、状態アイコンと進捗だけをバッジにする（`[↻ 42%] 名前: msg`）。
//! 名前を省いたとき（縮める段階 4。[`fit`](super::fit) を参照）も、状態アイコンと進捗だけのバッジになる。
//!
//! [`ViewOptions::highlight_attention`] が true なら、`Blocked` / `Error` のエージェントは 1 つ分全体を
//! バッジにする（`[! 名前: msg]`）。名前と msg の間に境目がなくなるので、間は `: ` にする。
//! 名前を入れるかどうかと強調するかどうかで変わるのは右端の文字の位置と msg の前の区切りだけで、
//! msg がなければ幅は変わらない。msg があると、名前までバッジに入れて強調しないときだけ、区切りが 1 桁狭い。

use super::ViewOptions;
use super::icon::IconStyle;
use super::span::{Fill, Role, Span};
use crate::model::AgentState;

/// Nerd Font モードのバッジの端の形。
///
/// 端の文字は前景色を状態の色にして行の背景の上に描き、状態の色の背景（バッジの内側）と行の背景の境目にする。
/// そのため Powerline の塗りつぶしの文字がそのまま使える。どの文字も 1 桁として扱う（`unicode-width` の `width()` と同じ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeShape {
    /// `nf-ple-lower_right_triangle`（U+E0BA）/ `nf-ple-upper_left_triangle`（U+E0BC）
    #[default]
    Slash,
    /// `nf-pl-right_hard_divider`（U+E0B2）/ `nf-pl-left_hard_divider`（U+E0B0）
    Arrow,
    /// `nf-ple-left_half_circle_thick`（U+E0B6）/ `nf-ple-right_half_circle_thick`（U+E0B4）
    Round,
}

impl BadgeShape {
    /// (左端, 右端) の文字
    pub(super) fn glyphs(self) -> (&'static str, &'static str) {
        match self {
            BadgeShape::Slash => ("\u{e0ba}", "\u{e0bc}"),
            BadgeShape::Arrow => ("\u{e0b2}", "\u{e0b0}"),
            BadgeShape::Round => ("\u{e0b6}", "\u{e0b4}"),
        }
    }
}

/// バッジの両端に付ける文字。どちらも左右 1 桁ずつ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BadgeEnds {
    /// 空白を状態の色で塗る
    Space,
    /// Nerd Font モードの端の文字（[`ViewOptions::badge_shape`]）。
    /// 前景色を状態の色にして、行の背景の上に描く。
    Glyph(BadgeShape),
}

impl BadgeEnds {
    /// 両端の文字と、その内側の空白（[`BadgeEnds::inner_pad`]）の幅の合計
    pub(super) fn width(self) -> usize {
        2 + 2 * usize::from(self.inner_pad())
    }

    /// 両端の文字の内側（左端の後ろと右端の手前）に、塗った空白を置くか。
    /// Nerd Font の端の文字は前景色で描くので中の文字との間に余白がなく、詰まって見えるため置く。
    /// 空白の端はそれ自体が余白なので置かない（置くと余白が広すぎる）。
    fn inner_pad(self) -> bool {
        matches!(self, BadgeEnds::Glyph(_))
    }

    /// 絵文字アイコンはバッジにしないので None
    pub(super) fn new(options: &ViewOptions) -> Option<Self> {
        match (options.icons, options.nerd_font) {
            (IconStyle::Emoji, _) => None,
            (_, true) => Some(BadgeEnds::Glyph(options.badge_shape)),
            (_, false) => Some(BadgeEnds::Space),
        }
    }

    /// (左端, 右端) の Span。内側の空白（[`BadgeEnds::inner_pad`]）も含める。
    /// 右端は名前を入れたり強調したりすると後ろに移るので、内側の空白も一緒に移す。
    pub(super) fn spans(self, state: AgentState) -> (Vec<Span>, Vec<Span>) {
        let role = Role::BadgeEnd(state);
        match self {
            BadgeEnds::Space => {
                let space = Span::new(" ", role).with_fill(Fill::State(state));
                (vec![space.clone()], vec![space])
            }
            BadgeEnds::Glyph(shape) => {
                let (left, right) = shape.glyphs();
                let pad = Span::new(" ", Role::State(state)).with_fill(Fill::State(state));
                (
                    vec![Span::new(left, role), pad.clone()],
                    vec![pad, Span::new(right, role)],
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_support::assert_nerd_font_glyph;

    #[test]
    fn badge_shape_glyphs() {
        let cases = [
            (BadgeShape::Slash, ["\u{e0ba}", "\u{e0bc}"]),
            (BadgeShape::Arrow, ["\u{e0b2}", "\u{e0b0}"]),
            (BadgeShape::Round, ["\u{e0b6}", "\u{e0b4}"]),
        ];
        for (shape, expected) in cases {
            let (left, right) = shape.glyphs();
            assert_eq!([left, right], expected, "{shape:?}");
            assert_nerd_font_glyph(left);
            assert_nerd_font_glyph(right);
        }
        assert_eq!(BadgeShape::default(), BadgeShape::Slash);
    }
}
