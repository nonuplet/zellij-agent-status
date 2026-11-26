//! 状態バッジ：状態アイコンと進捗、[`ViewOptions::badge_items`] の要素を、状態の色で背景を塗ったバッジ
//! （`[↻ 42% work] msg`）にする。Span の組み立ては [`Item::spans`](super::item::Item::spans) で行う。
//!
//! バッジ・アイコンに関わる設定（`state_icon_style` / `badge_shape` / `highlight_attention` /
//! `nerd_font_icon_padding`）は互いに依存するので、その組み合わせは [`BadgeStyle::new`] でまとめて解決する。
//! [`Item`](super::item::Item) は解決した結果だけを見る。
//!
//! バッジの右端の直後に msg が続くときは、右端が境目になるので、間は `: ` ではなく空白 1 つにする
//! （[`BadgeExtent::msg_separator`]）。
//! バッジの両端には [`ViewOptions::badge_shape`] の形の端の文字を付ける（[`BadgeShape::spans`]）。
//! 絵文字アイコンは自分の色を持っていて、背景も塗ると色が二重になるので、バッジにせず端の文字も付けない。
//! 選択モードで選んでいるときだけ、状態の色ではなく、行の文字色と背景色を反転させて塗る（[`BadgeExtent::fill`]）。
//! ただし、[`ViewOptions::highlight_attention`] で強調するエージェントは、選んでいるときも状態の色で塗る。
//!
//! 既定でタブ名などをバッジに入れ、既定の `nerd_font` で `app` のアイコンを先頭に置くのは、アイコンの右が
//! バッジの右端の文字に重なったり切れたりするのを避けるため。
//! アイコンは 1 桁から右にはみ出して描かれることがある（[`app_icon`](super::icon::app_icon) を参照。記号のアイコンでも右の余白が少ないものがある）。
//! アイコンの後ろに文字が続けば、はみ出した分は次の空白に重なるだけで目立たない。
//! `badge_items` が空のときや、要素を省いたとき（[`fit`](super::fit) を参照）は、状態アイコンと進捗だけのバッジになる。
//!
//! [`ViewOptions::highlight_attention`] が true なら、`Blocked` / `Error` のエージェントは 1 つ分全体を
//! バッジにする（`[! work: msg]`）。要素と msg の間に境目がなくなるので、間は `: ` にする。
//! 強調するかどうかで変わるのは右端の文字の位置と msg の前の区切りだけで、msg がなければ幅は変わらない。

use super::ViewOptions;
use super::icon::StateIconStyle;
use super::span::{Fill, Role, Span};
use crate::model::AgentState;

/// バッジの端の形。どれも左右に 1 桁ずつの端の文字を付ける。
///
/// [`BadgeShape::Square`] 以外は Nerd Font の文字を使う（[`BadgeShape::glyphs`]）。
/// 前景色を状態の色にして行の背景の上に描き、状態の色の背景（バッジの内側）と行の背景の境目にするので、
/// Powerline の塗りつぶしの文字がそのまま使える。どの文字も 1 桁として扱う（`unicode-width` の `width()` と同じ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeShape {
    /// 状態の色で塗った空白。フォントによらず使える
    #[default]
    Square,
    /// `nf-ple-lower_right_triangle`（U+E0BA）/ `nf-ple-upper_left_triangle`（U+E0BC）
    Slash,
    /// `nf-pl-right_hard_divider`（U+E0B2）/ `nf-pl-left_hard_divider`（U+E0B0）
    Arrow,
    /// `nf-ple-left_half_circle_thick`（U+E0B6）/ `nf-ple-right_half_circle_thick`（U+E0B4）
    Round,
}

impl BadgeShape {
    /// (左端, 右端) の Nerd Font の文字。[`BadgeShape::Square`] は Nerd Font の文字を使わないので None
    pub(super) fn glyphs(self) -> Option<(&'static str, &'static str)> {
        match self {
            BadgeShape::Square => None,
            BadgeShape::Slash => Some(("\u{e0ba}", "\u{e0bc}")),
            BadgeShape::Arrow => Some(("\u{e0b2}", "\u{e0b0}")),
            BadgeShape::Round => Some(("\u{e0b6}", "\u{e0b4}")),
        }
    }

    /// (左端, 右端) の Span。Nerd Font の端の文字では、内側の空白も含める。
    /// 右端は要素を入れたり強調したりすると後ろに移るので、内側の空白も一緒に移す。
    pub(super) fn spans(self, state: AgentState) -> (Vec<Span>, Vec<Span>) {
        let role = Role::BadgeEnd(state);
        match self.glyphs() {
            // 空白の端はそれ自体が余白なので、内側に空白を置かない（置くと余白が広すぎる）
            None => {
                let space = Span::new(" ", role).with_fill(Fill::State(state));
                (vec![space.clone()], vec![space])
            }
            // Nerd Font の端の文字は前景色で描くので中の文字との間に余白がなく、詰まって見える。
            // そのため両端の文字の内側（左端の後ろと右端の手前）に、塗った空白を置く
            Some((left, right)) => {
                let pad = Span::new(" ", Role::State(state)).with_fill(Fill::State(state));
                (
                    vec![Span::new(left, role), pad.clone()],
                    vec![pad, Span::new(right, role)],
                )
            }
        }
    }
}

/// バッジに入れる範囲。1 エージェント分を `アイコン 進捗 badge_items` / `text_items` / `区切り msg` の
/// 3 つの部分に分け、先頭から続けて入れる（[`Item::spans`](super::item::Item::spans)）。
/// 先頭から続くことを型で表すため、部分ごとの bool ではなく、どこまで入れるかで表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BadgeExtent {
    /// バッジにしない（絵文字アイコン）
    None,
    /// 状態アイコンと進捗、[`ViewOptions::badge_items`] の要素
    Badge,
    /// msg まで、エージェント 1 つ分全体（[`ViewOptions::highlight_attention`]）
    Whole,
}

impl BadgeExtent {
    /// (先頭から何部分を塗るか, その塗り方)。
    /// バッジにしないときは、選択モードで選んでいるときだけ、既定のバッジと同じ範囲を塗る。
    /// 先頭から塗るので、要素を省いてもアイコンが塗られ、どれを選んでいるかが分かる。
    /// msg まで塗ると、msg の文字のほうが要素より目立ってしまうので塗らない。
    pub(super) fn fill(self, state: AgentState) -> (usize, Fill) {
        match self {
            BadgeExtent::None => (1, Fill::Selected),
            BadgeExtent::Badge => (1, Fill::State(state)),
            BadgeExtent::Whole => (3, Fill::State(state)),
        }
    }

    /// msg の前の区切り。`has_text` は、バッジと msg の間に [`ViewOptions::text_items`] の要素があるか。
    /// バッジの右端の直後に msg が続く（強調せず、`text_items` の要素もない）ときは、右端が境目になるので空白 1 つにする。
    /// それ以外（要素と msg が同じ背景で続く）は `: ` にする。
    pub(super) fn msg_separator(self, has_text: bool) -> &'static str {
        match self {
            BadgeExtent::Badge if !has_text => " ",
            _ => ": ",
        }
    }
}

/// バッジ・アイコンに関わる設定の組み合わせを解決したもの（モジュールの説明を参照）。
/// 行全体で 1 つ作り、各エージェントの [`Item`](super::item::Item) はここから自分の見た目を決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BadgeStyle {
    state_icon_style: StateIconStyle,
    /// バッジの端の形。絵文字アイコンなら None（バッジにしない）
    shape: Option<BadgeShape>,
    /// 強調しないエージェントでバッジに入れる範囲
    extent: BadgeExtent,
    /// [`ViewOptions::highlight_attention`]
    highlight: bool,
    /// Nerd Font のアイコンの右隣に空白を置くか（[`ViewOptions::nerd_font_icon_padding`]）
    icon_padding: bool,
}

impl BadgeStyle {
    pub(super) fn new(options: &ViewOptions) -> Self {
        // badge_shape は、バッジにしない（絵文字アイコン）ときは効かない
        let shape = match options.state_icon_style {
            StateIconStyle::Emoji => None,
            _ => Some(options.badge_shape),
        };
        let extent = match shape {
            None => BadgeExtent::None,
            Some(_) => BadgeExtent::Badge,
        };
        BadgeStyle {
            state_icon_style: options.state_icon_style,
            shape,
            extent,
            highlight: options.highlight_attention,
            icon_padding: options.nerd_font_icon_padding,
        }
    }

    pub(super) fn icon(&self, state: AgentState) -> &'static str {
        self.state_icon_style.icon(state)
    }

    pub(super) fn shape(&self) -> Option<BadgeShape> {
        self.shape
    }

    /// 状態が `state` のエージェントでバッジに入れる範囲。
    /// 強調するエージェントは全体を入れる（絵文字アイコンでも、端の文字なしで全体を塗る）。
    pub(super) fn extent(&self, state: AgentState) -> BadgeExtent {
        if self.highlight && state.needs_attention() {
            BadgeExtent::Whole
        } else {
            self.extent
        }
    }

    /// 状態アイコンの後ろに空白を置くか（Nerd Font のアイコンで、[`BadgeStyle::icon_padding`] が true のとき）。
    /// 進捗があればその前の空白が右隣になるので、置かない。
    /// 強調して要素が続くときも置く（幅を状態によらず同じにするため。要素との間は空白 2 つになる）。
    pub(super) fn pads_state_icon(&self, has_progress: bool) -> bool {
        self.icon_padding && self.state_icon_style == StateIconStyle::NerdFont && !has_progress
    }

    /// `app` のアイコンの後ろに空白を置くか
    pub(super) fn pads_app_icon(&self) -> bool {
        self.icon_padding
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
            let (left, right) = shape.glyphs().unwrap();
            assert_eq!([left, right], expected, "{shape:?}");
            assert_nerd_font_glyph(left);
            assert_nerd_font_glyph(right);
        }
        assert_eq!(BadgeShape::Square.glyphs(), None);
    }
}
