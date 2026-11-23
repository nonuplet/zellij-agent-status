//! 描画の結果（[`render_line`] / [`render_message`] の出力を [`Line::paint`] したもの。色を含む）と、
//! クリックの判定（[`Line::pane_at`]）のテスト。

use zellij_tile::graphemes::display_width;

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
        select_fg: Color::Indexed(15),
        select_bg: Color::Indexed(236),
        idle: Color::Indexed(8),
        working: Color::Indexed(6),
        blocked: Color::Indexed(3),
        done: Color::Indexed(2),
        error: Color::Indexed(1),
    }
}

fn painted(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
    render_line(agents, cols, options).paint(&test_theme(), None)
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
    assert_eq!(display_width(&visible), 20);
    assert_eq!(visible.trim_end(), "  ✗ claude");

    // 名前をバッジに入れなければ、アイコンの直後が右端の空白で、名前は前景色のまま
    let without = ViewOptions {
        name_in_badge: false,
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

    // app のアイコンは太字にしない。バッジの中では反転させ、外では前景色
    let mut a = agent(1, AgentState::Error);
    a.app = Some("claude-code".to_owned());
    let nerd = ViewOptions {
        badge_shape: BadgeShape::Slash,
        show_app_icon: true,
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
            name_in_badge: false,
            ..nerd.clone()
        },
    );
    assert!(
        line_without.contains("\x1b[0;38;5;7;48;2;1;2;3m\u{ec82}"),
        "{line_without:?}"
    );
    // Nerd Font の端の文字は、行の背景の上に状態の色の前景色で描き、太字にしない
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
        state_icon_style: StateIconStyle::Emoji,
        ..Default::default()
    };
    let line = painted(&a, 30, &emoji);
    assert!(line.contains("\x1b[0;1;38;5;3;48;2;1;2;3m❗"), "{line:?}");
    assert!(!line.contains("48;5;3"), "{line:?}");
}

#[test]
fn message_is_cleaned_truncated_and_filled() {
    let message =
        |text, cols| strip_ansi(&render_message(text, cols).paint(&Theme::default(), None));
    let visible = message("設定の\n警告です", 12);
    assert_eq!(display_width(&visible), 12);
    assert_eq!(visible, " 設定の 警~ ");

    assert_eq!(message("ok", 5), " ok  ");
    assert_eq!(message(" \n", 3), "   ");
    for text in [
        "permission を待っています",
        "❤\u{fe0f}❤\u{fe0f}❤\u{fe0f}❤\u{fe0f}",
    ] {
        for cols in 0..30 {
            let visible = message(text, cols);
            assert_eq!(display_width(&visible), cols, "{text:?} cols={cols}");
        }
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
        strip_ansi(&line.paint(&test_theme(), None)),
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
    let visible = strip_ansi(&line.paint(&test_theme(), None));
    assert!(visible.ends_with('~'), "{visible:?}");
    assert_eq!(line.pane_at(29), Some(2));
    // 一部だけ見えているエージェントも選べる
    assert_eq!(line.pane_ids(), [1, 2]);
}

#[test]
fn selection_mode_recolors_line_and_keeps_only_selected_badge() {
    let titled = |pane_id, title: &str| {
        let mut a = agent(pane_id, AgentState::Working);
        a.title = Some(title.to_owned());
        a
    };
    let a = [titled(1, "one"), titled(2, "two")];
    // 選択モードの色（文字 15、背景 236）
    let select = "38;5;15;48;5;236m";

    // 名前をバッジに入れないので、名前は行の色で描かれる
    let options = ViewOptions {
        name_in_badge: false,
        ..Default::default()
    };
    let line = render_line(&a, 40, &options);
    let unselected = line.paint(&test_theme(), None);
    assert!(!unselected.contains("48;5;236"), "{unselected:?}");

    let selected = line.paint(&test_theme(), Some(2));
    // 文字は変えない
    assert_eq!(strip_ansi(&selected), strip_ansi(&unselected));
    // バッジ以外は、選んでいるエージェントの部分も含めて選択モードの色
    assert!(
        selected.contains(&format!("\x1b[0;1;{select}one")),
        "{selected:?}"
    );
    assert!(
        selected.contains(&format!("\x1b[0;1;{select}two")),
        "{selected:?}"
    );
    // 普段の背景色は残らない
    assert!(!selected.contains("48;2;1;2;3"), "{selected:?}");
    // 行頭の余白・区切り・行末の余りも選択モードの色
    assert!(
        selected.starts_with(&format!("\x1b[0;{select} ")),
        "{selected:?}"
    );
    assert!(
        selected.ends_with(&format!(
            "\x1b[0;48;5;236m{}\x1b[0m",
            " ".repeat(40 - display_width(strip_ansi(&unselected).trim_end()))
        )),
        "{selected:?}"
    );

    // 既定（名前までバッジ）で msg もない場合でも、色の塊（バッジ）は選んでいるエージェントだけに残る
    let line = render_line(&a, 40, &ViewOptions::default());
    let unselected = line.paint(&test_theme(), None);
    let selected = line.paint(&test_theme(), Some(2));
    let badge = |name| format!("\x1b[0;1;38;2;1;2;3;48;5;6m{name}");
    assert!(unselected.contains(&badge("one")), "{unselected:?}");
    assert!(unselected.contains(&badge("two")), "{unselected:?}");
    assert!(selected.contains(&badge("two")), "{selected:?}");
    assert!(!selected.contains(&badge("one")), "{selected:?}");
    // 選んでいないエージェントは、状態の色のアイコンと、選択モードの文字色の名前を、選択モードの背景に描く
    assert!(
        selected.contains("\x1b[0;1;38;5;6;48;5;236m↻"),
        "{selected:?}"
    );
    assert!(
        selected.contains(&format!("\x1b[0;1;{select}one")),
        "{selected:?}"
    );
    // 状態の色の背景（48;5;6）は、選んでいるエージェントの部分にしかない
    let filled: Vec<&str> = selected
        .split('\x1b')
        .filter(|s| s.contains("48;5;6m"))
        .map(|s| &s[s.find('m').unwrap() + 1..])
        .collect();
    assert_eq!(filled.concat(), " ↻ two ", "{selected:?}");
}

#[test]
fn selection_mode_hides_badge_ends_of_unselected_agents() {
    let a = [agent(1, AgentState::Error), agent(2, AgentState::Idle)];
    let nerd = ViewOptions {
        badge_shape: BadgeShape::Slash,
        ..Default::default()
    };
    let line = render_line(&a, 40, &nerd);
    let selected = line.paint(&test_theme(), Some(2));
    assert_eq!(
        strip_ansi(&selected),
        strip_ansi(&line.paint(&test_theme(), None))
    );
    // 選んでいないエージェントのバッジの端は、背景と同じ色（236）で描いて見えなくする
    assert!(
        selected.contains("\x1b[0;38;5;236;48;5;236m\u{e0ba}"),
        "{selected:?}"
    );
    assert!(
        selected.contains("\x1b[0;38;5;236;48;5;236m\u{e0bc}"),
        "{selected:?}"
    );
    // 選んでいるエージェントのバッジの端は、状態の色（idle は 8）で描き、外側は選択モードの背景（236）にする
    assert!(
        selected.contains("\x1b[0;38;5;8;48;5;236m\u{e0ba}"),
        "{selected:?}"
    );
    assert!(
        selected.contains("\x1b[0;38;5;8;48;5;236m\u{e0bc}"),
        "{selected:?}"
    );
}
