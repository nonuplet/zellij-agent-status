//! 描画の結果（[`render_line`] / [`render_message`] の出力。色を含む）のテスト。

use unicode_width::UnicodeWidthStr;

use super::test_support::{agent, with_msg};
use super::theme::Color;
use super::*;
use crate::model::AgentState;

/// ANSI エスケープを除いた文字列
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn test_theme() -> Theme {
    Theme {
        fg: Color::Indexed(7),
        bg: Color::Rgb(1, 2, 3),
        idle: Color::Indexed(8),
        working: Color::Indexed(6),
        blocked: Color::Indexed(3),
        done: Color::Indexed(2),
        error: Color::Indexed(1),
    }
}

#[test]
fn paint_colors_spans_and_fills_width() {
    let theme = test_theme();
    let a = [agent(1, AgentState::Error)];
    let line = render_line(&a, 20, &theme, &ViewOptions::default());
    // バッジは状態の色の背景に、行の背景色の文字（反転）。端の空白は太字にしない
    assert!(
        line.contains("\x1b[0;38;2;1;2;3;48;5;1m \x1b[0;1;38;2;1;2;3;48;5;1m✗"),
        "{line:?}"
    );
    // 名前もバッジの中なので反転させる（太字のまま）。その後ろが右端の空白
    assert!(
        line.contains("\x1b[0;1;38;2;1;2;3;48;5;1mclaude\x1b[0;38;2;1;2;3;48;5;1m "),
        "{line:?}"
    );
    assert!(line.ends_with("\x1b[0m"));
    let visible = strip_ansi(&line);
    assert_eq!(visible.width(), 20);
    assert_eq!(visible.trim_end(), "  ✗ claude");

    // 名前をバッジに入れなければ、アイコンの直後が右端の空白で、名前は前景色のまま
    let without = ViewOptions {
        badge_name: false,
        ..Default::default()
    };
    let line = render_line(&a, 20, &theme, &without);
    assert!(
        line.contains("\x1b[0;1;38;2;1;2;3;48;5;1m✗\x1b[0;38;2;1;2;3;48;5;1m "),
        "{line:?}"
    );
    assert!(
        line.contains("\x1b[0;1;38;5;7;48;2;1;2;3mclaude"),
        "{line:?}"
    );
    assert_eq!(strip_ansi(&line).trim_end(), "  ✗  claude");

    // Nerd Font モードの app のアイコンは太字にしない。バッジの中では反転させ、外では前景色
    let mut a = agent(1, AgentState::Error);
    a.app = Some("claude-code".to_owned());
    let nerd = ViewOptions {
        nerd_font: true,
        ..Default::default()
    };
    let line = render_line(&[a.clone()], 20, &theme, &nerd);
    assert!(
        line.contains("\x1b[0;38;2;1;2;3;48;5;1m\u{ec82}"),
        "{line:?}"
    );
    let line_without = render_line(
        &[a],
        20,
        &theme,
        &ViewOptions {
            badge_name: false,
            ..nerd.clone()
        },
    );
    assert!(
        line_without.contains("\x1b[0;38;5;7;48;2;1;2;3m\u{ec82}"),
        "{line_without:?}"
    );
    // バッジの端の文字（既定の slash）は、行の背景の上に状態の色の前景色で描き、太字にしない
    assert!(
        line.contains("\x1b[0;38;5;1;48;2;1;2;3m\u{e0ba}"),
        "{line:?}"
    );
    assert!(
        line.contains("\x1b[0;38;5;1;48;2;1;2;3m\u{e0bc}"),
        "{line:?}"
    );
}

#[test]
fn paint_inverts_highlighted_agent_and_skips_emoji_badge() {
    let theme = test_theme();
    let a = [with_msg(agent(1, AgentState::Blocked), "Allow?")];
    let highlight = ViewOptions {
        highlight_attention: true,
        ..Default::default()
    };
    let line = render_line(&a, 30, &theme, &highlight);
    // 名前・msg も状態の色の背景に、行の背景色の文字で描く
    assert!(
        line.contains("\x1b[0;1;38;2;1;2;3;48;5;3mclaude"),
        "{line:?}"
    );
    assert!(line.contains("\x1b[0;38;2;1;2;3;48;5;3mAllow?"), "{line:?}");

    // 絵文字アイコンはバッジにしない（今までどおり前景色だけ）
    let emoji = ViewOptions {
        icons: IconStyle::Emoji,
        ..Default::default()
    };
    let line = render_line(&a, 30, &theme, &emoji);
    assert!(line.contains("\x1b[0;1;38;5;3;48;2;1;2;3m❗"), "{line:?}");
    assert!(!line.contains("48;5;3"), "{line:?}");
}

#[test]
fn message_is_cleaned_truncated_and_filled() {
    let theme = Theme::default();
    let line = render_message("設定の\n警告です", 12, &theme);
    let visible = strip_ansi(&line);
    assert_eq!(visible.width(), 12);
    assert_eq!(visible, " 設定の 警~ ");

    assert_eq!(strip_ansi(&render_message("ok", 5, &theme)), " ok  ");
    assert_eq!(strip_ansi(&render_message(" \n", 3, &theme)), "   ");
    for cols in 0..30 {
        let visible = strip_ansi(&render_message("permission を待っています", cols, &theme));
        assert_eq!(visible.width(), cols, "cols={cols}");
    }
}
