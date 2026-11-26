//! view の複数のモジュールのテストで使うヘルパー。

use unicode_width::UnicodeWidthStr;

use super::{BadgeShape, Element, MsgMode, StateIconStyle, ViewOptions};
use crate::model::{AgentState, AgentStatus};

/// 既定の `badge_items`（`title tab_name`）で名前が表示されるよう、`title` を持たせる。
/// `app` と同じ文字列にしているので、`app_name` を表示しても同じ名前になる
pub(super) fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
    AgentStatus {
        pane_id,
        tab: 0,
        tab_name: None,
        cwd: None,
        state,
        progress: None,
        app: Some("claude".to_owned()),
        title: Some("claude".to_owned()),
        msg: None,
    }
}

pub(super) fn with_msg(mut a: AgentStatus, msg: &str) -> AgentStatus {
    a.msg = Some(msg.to_owned());
    a
}

pub(super) fn with_cwd(mut a: AgentStatus, cwd: &str) -> AgentStatus {
    a.cwd = Some(cwd.into());
    a
}

/// 1 文字だけで、BMP の Private Use Area にあり、幅 1 として数えられること
pub(super) fn assert_nerd_font_glyph(glyph: &str) {
    let mut chars = glyph.chars();
    let c = chars.next().unwrap();
    assert_eq!(chars.next(), None, "{glyph:?}");
    assert!(('\u{e000}'..='\u{f8ff}').contains(&c), "{glyph:?}");
    assert_eq!(glyph.width(), 1, "{glyph:?}");
}

/// テストの基準にする設定。切り詰め方などを状態によらず確かめられるよう、msg はすべてのエージェントで表示する。
/// 既定値（[`MsgMode::Attention`]）での表示は `msg_mode_*` のテストで確かめる。
pub(super) fn base() -> ViewOptions {
    ViewOptions {
        msg_mode: MsgMode::All,
        ..Default::default()
    }
}

pub(super) fn state_icons(state_icon_style: StateIconStyle) -> ViewOptions {
    ViewOptions {
        state_icon_style,
        ..base()
    }
}

/// (`badge_items`, `text_items`) の代表的な並び
pub(super) fn item_lists() -> Vec<(Vec<Element>, Vec<Element>)> {
    use Element::*;
    vec![
        // 既定と、nerd_font の既定
        (vec![Title, TabName], vec![]),
        (vec![AppIcon, Title, TabName], vec![]),
        // すべてバッジの外
        (vec![], vec![Title, TabName]),
        // すべての要素をバッジの中に、外に
        (vec![AppIcon, Title, TabName, AppName, Cwd], vec![]),
        (vec![AppIcon], vec![Title, TabName, AppName, Cwd]),
        // 中と外に分け、app のアイコンをバッジの末尾に置く
        (vec![TabName, AppIcon], vec![AppName, Cwd]),
        // cwd が最後に残りやすい（括弧の分だけ広い）
        (vec![AppIcon, Cwd], vec![]),
        // 何もない（#<pane_id> だけになる）
        (vec![], vec![]),
    ]
}

/// アイコン・要素の並び・バッジの端の形・強調・アイコンの後ろの空白・msg の表示モードの組み合わせ。
/// アイコンの後ろの空白は、Nerd Font のアイコンがある（効く）組み合わせでだけ true も入れる。
/// msg の表示モードの `None` は、msg のないエージェントだけになるのと同じなので入れない
pub(super) fn option_combinations() -> Vec<ViewOptions> {
    let styles = [
        StateIconStyle::Symbol,
        StateIconStyle::Alphabet,
        StateIconStyle::Emoji,
        StateIconStyle::NerdFont,
    ];
    let shapes = [
        BadgeShape::Square,
        BadgeShape::Slash,
        BadgeShape::Arrow,
        BadgeShape::Round,
    ];
    let mut combinations = vec![];
    for style in styles {
        for (badge_items, text_items) in item_lists() {
            let app_icon =
                badge_items.contains(&Element::AppIcon) || text_items.contains(&Element::AppIcon);
            let paddings = if app_icon || style == StateIconStyle::NerdFont {
                &[false, true][..]
            } else {
                &[false]
            };
            for &nerd_font_icon_padding in paddings {
                for badge_shape in shapes {
                    for highlight_attention in [false, true] {
                        for msg_mode in [MsgMode::All, MsgMode::Attention] {
                            combinations.push(ViewOptions {
                                badge_items: badge_items.clone(),
                                text_items: text_items.clone(),
                                badge_shape,
                                highlight_attention,
                                nerd_font_icon_padding,
                                msg_mode,
                                ..state_icons(style)
                            });
                        }
                    }
                }
            }
        }
    }
    combinations
}

/// `options` と各部分の幅が同じになる設定の代表。文字だけが違う設定と、効かない設定をそろえる。
/// 幅が本当に同じことは、layout の `width_representative_has_same_widths` で確かめる
pub(super) fn width_representative(options: &ViewOptions) -> ViewOptions {
    let mut representative = options.clone();
    if options.state_icon_style == StateIconStyle::Alphabet {
        representative.state_icon_style = StateIconStyle::Symbol;
    }
    if matches!(options.badge_shape, BadgeShape::Arrow | BadgeShape::Round) {
        representative.badge_shape = BadgeShape::Slash;
    }
    // 絵文字アイコンはバッジにしないので、badge_shape は効かない
    if options.state_icon_style == StateIconStyle::Emoji {
        representative.badge_shape = BadgeShape::Square;
    }
    representative
}

/// [`option_combinations`] のうち、幅が同じ組み合わせを代表だけにしたもの。
/// 行の幅などと掛け合わせる総当たりのテストで、時間がかかりすぎないように使う
pub(super) fn width_combinations() -> Vec<ViewOptions> {
    option_combinations()
        .into_iter()
        .filter(|options| width_representative(options) == *options)
        .collect()
}
