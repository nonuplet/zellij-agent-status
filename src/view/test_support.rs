//! view の複数のモジュールのテストで使うヘルパー。

use unicode_width::UnicodeWidthStr;

use super::{BadgeShape, IconStyle, MsgMode, ViewOptions};
use crate::model::{AgentState, AgentStatus};

pub(super) fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
    AgentStatus {
        pane_id,
        tab: 0,
        tab_name: None,
        cwd: None,
        state,
        kind: None,
        progress: None,
        app: Some("claude".to_owned()),
        title: None,
        msg: None,
        updated_ms: 0,
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

pub(super) fn icons(icons: IconStyle) -> ViewOptions {
    ViewOptions { icons, ..base() }
}

/// アイコン・cwd・Nerd Font モード（バッジの端の形）・強調・アイコンの後ろの空白・バッジの中の名前・
/// msg の表示モードの組み合わせ。
/// アイコンの後ろの空白が効かない組み合わせでは、同じ設定が 2 回入る（重複は気にしない）。
/// msg の表示モードの `None` は、msg のないエージェントだけになるのと同じなので入れない
pub(super) fn option_combinations() -> Vec<ViewOptions> {
    let styles = [
        IconStyle::Symbol,
        IconStyle::Alphabet,
        IconStyle::Emoji,
        IconStyle::Nerd,
    ];
    let modes = [
        (false, BadgeShape::Slash),
        (true, BadgeShape::Slash),
        (true, BadgeShape::Arrow),
        (true, BadgeShape::Round),
    ];
    let mut combinations = vec![];
    for style in styles {
        for show_cwd in [false, true] {
            for (nerd_font, badge_shape) in modes {
                for highlight_attention in [false, true] {
                    // アイコンの後ろの空白は Nerd Font のアイコンがあるときだけ効く
                    let uses_nerd_icon = nerd_font || style == IconStyle::Nerd;
                    for nerd_font_icon_padding in [false, uses_nerd_icon] {
                        for badge_name in [false, true] {
                            for msg_mode in [MsgMode::All, MsgMode::Attention] {
                                combinations.push(ViewOptions {
                                    show_cwd,
                                    nerd_font,
                                    badge_shape,
                                    highlight_attention,
                                    nerd_font_icon_padding,
                                    badge_name,
                                    msg_mode,
                                    ..icons(style)
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    combinations
}
