//! 描画の結果（[`render_line`] / [`render_message`] の出力を [`Line::paint`] したもの。色を含む）と、
//! クリックの判定（[`Line::pane_at`]）のテスト。

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

fn painted(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
    render_line(agents, cols, options).paint(&test_theme())
}

#[test]
fn paint_colors_spans_and_fills_width() {
    let a = [agent(1, AgentState::Error)];
    let line = painted(&a, 20, &ViewOptions::default());
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
    let line = painted(&a, 20, &without);
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
    let line = painted(&[a.clone()], 20, &nerd);
    assert!(
        line.contains("\x1b[0;38;2;1;2;3;48;5;1m\u{ec82}"),
        "{line:?}"
    );
    let line_without = painted(
        &[a],
        20,
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
    let a = [with_msg(agent(1, AgentState::Blocked), "Allow?")];
    let highlight = ViewOptions {
        highlight_attention: true,
        ..Default::default()
    };
    let line = painted(&a, 30, &highlight);
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
    let line = painted(&a, 30, &emoji);
    assert!(line.contains("\x1b[0;1;38;5;3;48;2;1;2;3m❗"), "{line:?}");
    assert!(!line.contains("48;5;3"), "{line:?}");
}

#[test]
fn message_is_cleaned_truncated_and_filled() {
    let message = |text, cols| strip_ansi(&render_message(text, cols).paint(&Theme::default()));
    let visible = message("設定の\n警告です", 12);
    assert_eq!(visible.width(), 12);
    assert_eq!(visible, " 設定の 警~ ");

    assert_eq!(message("ok", 5), " ok  ");
    assert_eq!(message(" \n", 3), "   ");
    for cols in 0..30 {
        let visible = message("permission を待っています", cols);
        assert_eq!(visible.width(), cols, "cols={cols}");
    }
}

#[test]
fn message_has_no_pane() {
    let line = render_message("permission を待っています", 30);
    assert!((0..30).all(|col| line.pane_at(col).is_none()));
}

/// 各列のペインを、ペインの id（どのエージェントにも属さない列は `.`）を並べた文字列にする
fn pane_map(line: &Line, cols: usize) -> String {
    (0..cols)
        .map(|col| match line.pane_at(col) {
            Some(id) => char::from_digit(id, 10).unwrap(),
            None => '.',
        })
        .collect()
}

#[test]
fn whole_agent_is_clickable_except_padding_and_separators() {
    let a = [
        with_msg(agent(1, AgentState::Working), "Running"),
        with_msg(agent(2, AgentState::Blocked), "Allow?"),
    ];
    let options = ViewOptions {
        msg_mode: MsgMode::All,
        ..Default::default()
    };
    let line = render_line(&a, 40, &options);
    assert_eq!(
        strip_ansi(&line.paint(&test_theme())),
        "  ↻ claude  Running   ! claude  Allow?  "
    );
    // バッジの端から msg の末尾までがそのエージェントの部分。行頭の余白・区切り・行末の空白は当たらない
    assert_eq!(
        pane_map(&line, 40),
        ".111111111111111111..22222222222222222.."
    );
}

#[test]
fn ellipsis_belongs_to_cut_agent() {
    let a = [
        agent(1, AgentState::Working),
        with_msg(agent(2, AgentState::Blocked), "Allow this command?"),
    ];
    let options = ViewOptions {
        msg_mode: MsgMode::All,
        ..Default::default()
    };
    // 2 つ目の msg の途中で切れる。末尾の ~ も 2 つ目の部分
    let line = render_line(&a, 30, &options);
    let visible = strip_ansi(&line.paint(&test_theme()));
    assert!(visible.ends_with('~'), "{visible:?}");
    assert_eq!(line.pane_at(29), Some(2));
}
