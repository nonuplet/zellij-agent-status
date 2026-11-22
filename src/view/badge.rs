//! 状態バッジ：状態アイコンから名前（と `app` のアイコン・cwd）までを、状態の色で背景を塗ったバッジ
//! （`[↻ 42% 名前 (cwd)] msg`）にする。Span の組み立ては [`Item::spans`](super::item::Item::spans) で行う。
//!
//! バッジ・アイコンに関わる設定（`icons` / `nerd_font` / `badge_shape` / `badge_name` / `highlight_attention` /
//! `nerd_font_icon_padding`）は互いに依存するので、その組み合わせは [`BadgeStyle::new`] でまとめて解決する。
//! [`Item`](super::item::Item) は解決した結果だけを見る。
//!
//! バッジの右端が名前と msg の境目になるので、間は `: ` ではなく空白 1 つにする
//! （[`BadgeExtent::msg_separator`]）。
//! バッジの両端には端の文字（[`BadgeEnds`]。Nerd Font モードでは [`ViewOptions::badge_shape`] の形）を付ける。
//! Nerd Font モードでは、両端の文字の内側（左端の後ろと右端の手前）に塗った空白を 1 つずつ置く（[`BadgeEnds::spans`]）。
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
use super::icon::{IconStyle, app_icon};
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
    /// (左端, 右端) の Span。Nerd Font の端の文字では、内側の空白も含める。
    /// 右端は名前を入れたり強調したりすると後ろに移るので、内側の空白も一緒に移す。
    pub(super) fn spans(self, state: AgentState) -> (Vec<Span>, Vec<Span>) {
        let role = Role::BadgeEnd(state);
        match self {
            // 空白の端はそれ自体が余白なので、内側に空白を置かない（置くと余白が広すぎる）
            BadgeEnds::Space => {
                let space = Span::new(" ", role).with_fill(Fill::State(state));
                (vec![space.clone()], vec![space])
            }
            // Nerd Font の端の文字は前景色で描くので中の文字との間に余白がなく、詰まって見える。
            // そのため両端の文字の内側（左端の後ろと右端の手前）に、塗った空白を置く
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

/// バッジに入れる範囲。1 エージェント分を `アイコン 進捗` / ` <app> 名前 (cwd)` / `: msg` の 3 つの部分に分け、
/// 先頭から続けて入れる（[`Item::spans`](super::item::Item::spans)）。
/// 先頭から続くことを型で表すため、部分ごとの bool ではなく、どこまで入れるかで表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BadgeExtent {
    /// バッジにしない（絵文字アイコン）
    None,
    /// 状態アイコンと進捗
    Head,
    /// 名前（と `app` のアイコン・cwd）まで（[`ViewOptions::badge_name`]）
    Label,
    /// msg まで、エージェント 1 つ分全体（[`ViewOptions::highlight_attention`]）
    Whole,
}

impl BadgeExtent {
    /// 先頭から何部分をバッジに入れるか
    pub(super) fn parts(self) -> usize {
        match self {
            BadgeExtent::None => 0,
            BadgeExtent::Head => 1,
            BadgeExtent::Label => 2,
            BadgeExtent::Whole => 3,
        }
    }

    /// 名前（と cwd）と msg の間の区切り。
    /// 名前までをバッジにする（強調しない）ときは、バッジの右端が境目になるので空白 1 つにする。
    /// それ以外（名前と msg が同じ背景で続く）は `: ` にする。強調すると幅が 1 広がる。
    pub(super) fn msg_separator(self) -> &'static str {
        match self {
            BadgeExtent::Label => " ",
            _ => ": ",
        }
    }
}

/// バッジ・アイコンに関わる設定の組み合わせを解決したもの（モジュールの説明を参照）。
/// 行全体で 1 つ作り、各エージェントの [`Item`](super::item::Item) はここから自分の見た目を決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BadgeStyle {
    icons: IconStyle,
    /// バッジの端の文字。絵文字アイコンなら None（バッジにしない）
    ends: Option<BadgeEnds>,
    /// 強調しないエージェントでバッジに入れる範囲
    extent: BadgeExtent,
    /// [`ViewOptions::highlight_attention`]
    highlight: bool,
    /// 名前の前に `app` のアイコンを付けるか（Nerd Font モード）
    app_icon: bool,
    /// Nerd Font のアイコンの右隣に空白を置くか（[`ViewOptions::nerd_font_icon_padding`]）
    icon_padding: bool,
}

impl BadgeStyle {
    pub(super) fn new(options: &ViewOptions) -> Self {
        let ends = match (options.icons, options.nerd_font) {
            (IconStyle::Emoji, _) => None,
            (_, true) => Some(BadgeEnds::Glyph(options.badge_shape)),
            (_, false) => Some(BadgeEnds::Space),
        };
        // badge_name は、バッジにしない（絵文字アイコン）ときは効かない
        let extent = match ends {
            None => BadgeExtent::None,
            Some(_) if options.badge_name => BadgeExtent::Label,
            Some(_) => BadgeExtent::Head,
        };
        BadgeStyle {
            icons: options.icons,
            ends,
            extent,
            highlight: options.highlight_attention,
            app_icon: options.nerd_font,
            icon_padding: options.nerd_font_icon_padding,
        }
    }

    pub(super) fn icon(&self, state: AgentState) -> &'static str {
        self.icons.icon(state)
    }

    pub(super) fn ends(&self) -> Option<BadgeEnds> {
        self.ends
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
    /// 強調して名前が続くときも置く（幅を状態によらず同じにするため。名前との間は空白 2 つになる）。
    pub(super) fn pads_state_icon(&self, has_progress: bool) -> bool {
        self.icon_padding && self.icons == IconStyle::Nerd && !has_progress
    }

    /// 名前の前に付ける `app` のアイコン（Nerd Font モードで、対応するアイコンがあるときだけ）
    pub(super) fn app_icon(&self, app: Option<&str>) -> Option<&'static str> {
        app.filter(|_| self.app_icon).and_then(app_icon)
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
            let (left, right) = shape.glyphs();
            assert_eq!([left, right], expected, "{shape:?}");
            assert_nerd_font_glyph(left);
            assert_nerd_font_glyph(right);
        }
        assert_eq!(BadgeShape::default(), BadgeShape::Slash);
    }
}
