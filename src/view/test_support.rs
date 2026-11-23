//! view の複数のモジュールのテストで使うヘルパー。

use unicode_width::UnicodeWidthStr;

use super::{BadgeShape, MsgMode, StateIconStyle, ViewOptions};
use crate::model::{AgentState, AgentStatus};

pub(super) fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
    AgentStatus {
        pane_id,
        tab: 0,
        tab_name: None,
        cwd: None,
        state,
        progress: None,
        app: Some("claude".to_owned()),
        title: None,
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

/// アイコン・cwd・バッジの端の形・`app` のアイコン・強調・アイコンの後ろの空白・バッジの中の名前・
/// msg の表示モードの組み合わせ。
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
        for show_app_icon in [false, true] {
            let paddings = if show_app_icon || style == StateIconStyle::NerdFont {
                &[false, true][..]
            } else {
                &[false]
            };
            for &nerd_font_icon_padding in paddings {
                for badge_shape in shapes {
                    for show_cwd in [false, true] {
                        for highlight_attention in [false, true] {
                            for name_in_badge in [false, true] {
                                for msg_mode in [MsgMode::All, MsgMode::Attention] {
                                    combinations.push(ViewOptions {
                                        show_cwd,
                                        badge_shape,
                                        show_app_icon,
                                        highlight_attention,
                                        nerd_font_icon_padding,
                                        name_in_badge,
                                        msg_mode,
                                        ..state_icons(style)
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    combinations
}
