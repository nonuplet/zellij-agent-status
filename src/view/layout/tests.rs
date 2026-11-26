//! 行全体（[`layout()`] の結果）のテスト。

use super::*;
use crate::view::badge::BadgeShape;
use crate::view::icon::{StateIconStyle, app_icon};
use crate::view::span::Fill;
use crate::view::test_support::{
    agent, base, option_combinations, state_icons, width_combinations, width_representative,
    with_cwd, with_msg,
};
use crate::view::text::ELLIPSIS;
use crate::view::{DEFAULT_MAX_AGENT_WIDTH, Element, MIN_MAX_AGENT_WIDTH, MsgMode};

fn plain(agents: &[AgentStatus], cols: usize) -> String {
    plain_with(agents, cols, &base())
}

fn plain_with(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
    plain_text(&layout(agents, cols, options))
}

/// 行の文字列。読みやすいように、バッジの端の文字は左端を `[`、右端を `]` にする
/// （端の文字はどれも幅 1 なので、幅は変わらない）。実際の端の文字は [`badge_ends`] で確かめる。
fn plain_text(spans: &[Span]) -> String {
    let mut open = false;
    spans
        .iter()
        .map(|s| match s.role {
            Role::BadgeEnd(_) => {
                open = !open;
                if open { "[" } else { "]" }
            }
            _ => &s.text,
        })
        .collect()
}

#[test]
fn empty_list_renders_nothing() {
    assert_eq!(plain(&[], 80), "");
}

#[test]
fn shows_icon_name_and_msg() {
    let a = with_msg(agent(1, AgentState::Working), "Running tests");
    assert_eq!(plain(&[a], 80), " [↻ claude] Running tests");
}

#[test]
fn shows_progress_after_icon() {
    let mut a = agent(1, AgentState::Working);
    a.progress = Some(42);
    assert_eq!(plain(&[a], 80), " [↻ 42% claude]");
}

#[test]
fn icon_per_state() {
    let cases = [
        (AgentState::Idle, "⋯", "I", "💤"),
        (AgentState::Working, "↻", "W", "⏳"),
        (AgentState::Blocked, "!", "B", "❗"),
        (AgentState::Done, "✓", "D", "✅"),
        (AgentState::Error, "✗", "E", "❌"),
    ];
    let alphabet = state_icons(StateIconStyle::Alphabet);
    let emoji = state_icons(StateIconStyle::Emoji);
    for (state, symbol, letter, emoji_icon) in cases {
        let a = [agent(1, state)];
        assert_eq!(plain(&a, 80), format!(" [{symbol} claude]"));
        assert_eq!(plain_with(&a, 80, &alphabet), format!(" [{letter} claude]"));
        assert_eq!(plain_with(&a, 80, &emoji), format!(" {emoji_icon} claude"));
    }
}

#[test]
fn emoji_icons_are_counted_as_two_columns() {
    let emoji = state_icons(StateIconStyle::Emoji);
    let mut a = agent(1, AgentState::Working);
    a.title = Some("long-project-name".to_owned());
    let b = agent(2, AgentState::Done);
    // 名前に回せる幅は 11（記号のときより 2 少ない）：均等に分けて短い "claude" に 5、残り 6 を長い方へ
    assert_eq!(
        plain_with(&[a.clone(), b.clone()], 20, &emoji),
        " ⏳ long-~  ✅ clau~"
    );
    assert_eq!(plain_with(&[a, b], 14, &emoji), " ⏳  ✅");
}

#[test]
fn emoji_icon_is_not_split_at_line_end() {
    let emoji = state_icons(StateIconStyle::Emoji);
    let agents: Vec<_> = (0..5).map(|i| agent(i, AgentState::Idle)).collect();
    assert_eq!(plain_with(&agents, 8, &emoji), " 💤  💤~");
    // 残り 1 桁には幅 2 のアイコンが入らないので、その手前で切る
    assert_eq!(plain_with(&agents, 7, &emoji), " 💤  ~");
}

fn items(badge_items: &[Element], text_items: &[Element]) -> ViewOptions {
    ViewOptions {
        badge_items: badge_items.to_vec(),
        text_items: text_items.to_vec(),
        ..base()
    }
}

#[test]
fn default_items_show_title_and_tab_name() {
    let mut a = agent(7, AgentState::Idle);
    a.title = Some("my-project".to_owned());
    a.tab_name = Some("work".to_owned());
    assert_eq!(plain(&[a.clone()], 80), " [⋯ my-project work]");
    a.title = Some("  ".to_owned());
    assert_eq!(plain(&[a.clone()], 80), " [⋯ work]");
    // 一覧にない app は、値があっても使わない
    a.tab_name = None;
    assert_eq!(plain(&[a], 80), " [⋯ #7]");
}

#[test]
fn tab_name_is_cleaned_and_skipped_when_empty() {
    let mut a = agent(7, AgentState::Idle);
    a.title = None;
    a.tab_name = Some(" \t\n".to_owned());
    assert_eq!(plain(&[a.clone()], 80), " [⋯ #7]");
    a.tab_name = Some("my\ntab".to_owned());
    assert_eq!(plain(&[a], 80), " [⋯ my tab]");
}

/// Blocked、タブ名 `work`、`app` が `claude-code`、cwd が `repo`、`title` なしで、msg を持つエージェント
fn issue_example() -> [AgentStatus; 1] {
    let mut a = with_msg(
        with_cwd(agent(1, AgentState::Blocked), "/home/user/repo"),
        "msg",
    );
    a.title = None;
    a.tab_name = Some("work".to_owned());
    a.app = Some("claude-code".to_owned());
    [a]
}

#[test]
fn items_are_shown_in_written_order_inside_and_outside_badge() {
    use Element::*;
    let a = issue_example();
    let cases = [
        (base(), " [! work] msg"),
        (items(&[TabName, AppName], &[]), " [! work claude-code] msg"),
        (items(&[AppName, TabName], &[]), " [! claude-code work] msg"),
        // バッジの外の要素の後ろでは、msg の前は `: `
        (
            items(&[TabName], &[AppName, Cwd]),
            " [! work] claude-code (repo): msg",
        ),
        (items(&[], &[TabName]), " [!] work: msg"),
        (items(&[Cwd], &[TabName]), " [! (repo)] work: msg"),
        (highlight(base()), " [! work: msg]"),
        (
            highlight(items(&[TabName], &[AppName])),
            " [! work claude-code: msg]",
        ),
        (state_icons(StateIconStyle::Emoji), " ❗ work: msg"),
        (
            ViewOptions {
                state_icon_style: StateIconStyle::Emoji,
                ..items(&[TabName], &[AppName])
            },
            " ❗ work claude-code: msg",
        ),
    ];
    for (options, expected) in cases {
        assert_eq!(plain_with(&a, 80, &options), expected, "{options:?}");
    }
}

#[test]
fn tab_name_and_app_name_are_shown_together() {
    use Element::*;
    let [mut a] = issue_example();
    a.msg = None;
    a.title = Some("my-project".to_owned());
    assert_eq!(
        plain_with(&[a], 80, &items(&[Title, TabName, AppName], &[])),
        " [! my-project work claude-code]"
    );
}

#[test]
fn elements_without_value_are_omitted_with_spaces_and_parens() {
    use Element::*;
    let mut a = with_msg(agent(1, AgentState::Idle), "msg");
    a.title = None;
    a.tab_name = Some("work".to_owned());
    let options = items(&[Title, Cwd, TabName], &[Cwd, AppName, Title]);
    // title と cwd がないので、前の空白も括弧も残らない
    assert_eq!(
        plain_with(&[a.clone()], 80, &options),
        " [⋯ work] claude: msg"
    );
    // バッジの外の要素がすべてなければ、msg はバッジの右端の直後になる
    a.app = None;
    assert_eq!(plain_with(&[a], 80, &options), " [⋯ work] msg");
}

#[test]
fn same_element_in_both_lists_is_shown_only_in_badge() {
    use Element::*;
    let mut a = with_cwd(agent(1, AgentState::Idle), "/repo/main");
    a.app = Some("codex".to_owned());
    assert_eq!(
        plain_with(&[a], 80, &items(&[Title, Cwd], &[Cwd, AppName])),
        " [⋯ claude (main)] codex"
    );
}

#[test]
fn pane_id_is_shown_when_no_element_identifies_agent() {
    use Element::*;
    let mut a = with_msg(agent(7, AgentState::Idle), "msg");
    a.title = None;
    a.app = Some("claude-code".to_owned());
    a.tab_name = Some("work".to_owned());
    let cases = [
        // 一覧にない要素は、値があっても使わない
        (items(&[Title], &[]), " [⋯ #7] msg"),
        // app のアイコンだけでは見分けられないので、#<pane_id> も表示する（badge_items の末尾）
        (items(&[AppIcon], &[Cwd]), " [⋯ \u{ec82} #7] msg"),
        (items(&[AppIcon, Title], &[]), " [⋯ \u{ec82} #7] msg"),
        // badge_items が空なら text_items の末尾
        (items(&[], &[Title]), " [⋯] #7: msg"),
        (items(&[], &[AppIcon]), " [⋯] \u{ec82} #7: msg"),
        (items(&[], &[]), " [⋯] #7: msg"),
        // 表示できる要素が 1 つでもあれば表示しない
        (items(&[AppIcon], &[TabName]), " [⋯ \u{ec82}] work: msg"),
    ];
    for (options, expected) in cases {
        assert_eq!(
            plain_with(std::slice::from_ref(&a), 80, &options),
            expected,
            "{options:?}"
        );
    }
}

#[test]
fn shrinking_order_ignores_item_order_and_keeps_last_element() {
    use Element::*;
    let mut a = with_cwd(agent(1, AgentState::Idle), "/repo/main");
    a.title = None;
    a.tab_name = Some("work".to_owned());
    a.app = Some("claude-code".to_owned());
    // app しかないエージェント
    let mut b = agent(2, AgentState::Idle);
    b.title = None;
    b.app = Some("codex".to_owned());
    let agents = [a, b];
    let options = ViewOptions {
        max_agent_width: None,
        ..items(&[Cwd, AppName, TabName], &[])
    };
    let line = |cols| plain_with(&agents, cols, &options);

    assert_eq!(line(39), " [⋯ (main) claude-code work]  [⋯ codex]");
    // 並びの先頭でも、cwd → app_name の順に縮める
    assert_eq!(line(38), " [⋯ claude-code work]  [⋯ codex]");
    assert_eq!(line(31), " [⋯ claude-co~ work]  [⋯ codex]");
    // b の app は最後に残る要素なので、a の app を省いても残す
    assert_eq!(line(24), " [⋯ work]  [⋯ codex]");
    // 最後に残る要素同士を、まとめて切り詰める
    assert_eq!(line(19), " [⋯ work]  [⋯ cod~]");
    assert_eq!(line(18), " [⋯]  [⋯]");
}

#[test]
fn items_do_not_affect_order_or_filter() {
    let mut a = agent(5, AgentState::Idle);
    a.tab_name = Some("z".to_owned());
    let mut b = agent(1, AgentState::Idle);
    b.tab = 1;
    b.tab_name = Some("a".to_owned());
    let mut c = agent(9, AgentState::Idle);
    c.tab_name = Some("z".to_owned());
    c.app = Some("codex".to_owned());
    let mut agents = [b, c, a];
    for a in &mut agents {
        a.title = None;
    }

    // 同じタブに複数いると同じ名前が並ぶ（既知の制限）
    assert_eq!(plain(&agents, 80), " [⋯ z]  [⋯ z]  [⋯ a]");
    let only_claude = ViewOptions {
        app_filter: Some(vec!["claude".to_owned()]),
        ..base()
    };
    assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯ z]  [⋯ a]");

    // 表示する要素を変えても、並び順と絞り込みは変わらない
    let app_name = items(&[Element::AppName], &[]);
    assert_eq!(
        plain_with(&agents, 80, &app_name),
        " [⋯ claude]  [⋯ codex]  [⋯ claude]"
    );
    let only_claude = ViewOptions {
        app_filter: only_claude.app_filter,
        ..app_name
    };
    assert_eq!(
        plain_with(&agents, 80, &only_claude),
        " [⋯ claude]  [⋯ claude]"
    );
}

#[test]
fn app_filter_does_not_need_app_name_item() {
    let mut codex = agent(2, AgentState::Idle);
    codex.app = Some("codex".to_owned());
    let mut no_app = agent(3, AgentState::Idle);
    no_app.app = None;
    let mut agents = [agent(1, AgentState::Idle), codex, no_app];
    for a in &mut agents {
        a.title = None;
    }

    let only_claude = ViewOptions {
        app_filter: Some(vec!["claude".to_owned()]),
        ..base()
    };
    assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯ #1]");
    assert_eq!(plain(&agents, 80), " [⋯ #1]  [⋯ #2]  [⋯ #3]");
}

/// 既定の要素の後ろに cwd を表示する。
/// 行の幅による縮め方を見るため、`max_agent_width` の上限はなくしておく
fn show_cwd() -> ViewOptions {
    ViewOptions {
        max_agent_width: None,
        ..items(&[Element::Title, Element::TabName, Element::Cwd], &[])
    }
}

#[test]
fn shows_last_directory_of_cwd_after_name() {
    let a = with_msg(
        with_cwd(agent(1, AgentState::Working), "/repo/.worktrees/issue-21"),
        "Running tests",
    );
    assert_eq!(
        plain_with(std::slice::from_ref(&a), 80, &show_cwd()),
        " [↻ claude (issue-21)] Running tests"
    );
    // 既定では表示しない
    assert_eq!(plain(&[a], 80), " [↻ claude] Running tests");
}

#[test]
fn cwd_without_last_directory_or_unknown() {
    let options = show_cwd();
    let root = with_cwd(agent(1, AgentState::Idle), "/");
    assert_eq!(plain_with(&[root], 80, &options), " [⋯ claude (/)]");
    let trailing = with_cwd(agent(1, AgentState::Idle), "/repo/main/");
    assert_eq!(plain_with(&[trailing], 80, &options), " [⋯ claude (main)]");
    let control = with_cwd(agent(1, AgentState::Idle), "/repo/a\nb");
    assert_eq!(plain_with(&[control], 80, &options), " [⋯ claude (a b)]");
    // 取得できていなければ、括弧ごと表示しない
    let unknown = agent(1, AgentState::Idle);
    assert_eq!(plain_with(&[unknown], 80, &options), " [⋯ claude]");
}

#[test]
fn cwd_and_msg_are_placed_per_agent() {
    let a = with_msg(agent(1, AgentState::Working), "Running tests");
    let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
    assert_eq!(
        plain_with(&[a, b], 80, &show_cwd()),
        " [↻ claude] Running tests  [✓ claude (main)]"
    );
}

#[test]
fn msgs_then_cwds_then_names_are_shortened() {
    let options = show_cwd();
    let a = with_msg(
        with_cwd(agent(1, AgentState::Working), "/repo/feature-branch"),
        "Running tests",
    );
    let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
    let agents = [a, b];
    let line = |cols| plain_with(&agents, cols, &options);

    assert_eq!(
        line(61),
        " [↻ claude (feature-branch)] Running tests  [✓ claude (main)]"
    );
    // 1. msg を切り詰める（MIN_MSG_WIDTH まで）
    assert_eq!(
        line(54),
        " [↻ claude (feature-branch)] Runni~  [✓ claude (main)]"
    );
    // 2. msg を省き、cwd を均等に切り詰める
    assert_eq!(line(53), " [↻ claude (feature-branch)]  [✓ claude (main)]");
    assert_eq!(line(44), " [↻ claude (feature-br~)]  [✓ claude (main)]");
    assert_eq!(line(37), " [↻ claude (fea~)]  [✓ claude (main)]");
    // 3. cwd を省き、名前を切り詰める
    assert_eq!(line(36), " [↻ claude]  [✓ claude]");
    // 4. アイコンだけにする
    assert_eq!(line(16), " [↻]  [✓]");
}

#[test]
fn control_characters_are_replaced() {
    let a = with_msg(agent(1, AgentState::Done), "line1\nline2\x1b[31m");
    assert_eq!(plain(&[a], 80), " [✓ claude] line1 line2 [31m");
}

#[test]
fn sorted_by_tab_then_pane() {
    let mut a = agent(5, AgentState::Idle);
    a.title = Some("a".to_owned());
    let mut b = agent(1, AgentState::Idle);
    b.title = Some("b".to_owned());
    b.tab = 1;
    let mut c = agent(9, AgentState::Idle);
    c.title = Some("c".to_owned());
    assert_eq!(plain(&[b, c, a], 80), " [⋯ a]  [⋯ c]  [⋯ b]");
}

/// (タブ, ペイン) の順で a, b, c, d, e になるエージェント。状態は `states` の順に割り当てる
fn named_agents(states: [AgentState; 5]) -> Vec<AgentStatus> {
    let names = ["a", "b", "c", "d", "e"];
    let tabs_and_panes = [(0, 1), (0, 5), (1, 2), (1, 3), (2, 0)];
    names
        .into_iter()
        .zip(tabs_and_panes)
        .zip(states)
        .rev() // 受け取る順序に依存しないことも確かめる
        .map(|((name, (tab, pane)), state)| {
            let mut a = agent(pane, state);
            a.tab = tab;
            a.title = Some(name.to_owned());
            a
        })
        .collect()
}

#[test]
fn blocked_is_not_moved_by_default() {
    use AgentState::*;
    let agents = named_agents([Idle, Blocked, Working, Blocked, Error]);
    assert_eq!(plain(&agents, 80), " [⋯ a]  [! b]  [↻ c]  [! d]  [✗ e]");
}

#[test]
fn blocked_first_moves_blocked_to_front_keeping_order() {
    use AgentState::*;
    let blocked_first = ViewOptions {
        blocked_first: true,
        ..base()
    };
    let agents = named_agents([Idle, Blocked, Working, Blocked, Error]);
    assert_eq!(
        plain_with(&agents, 80, &blocked_first),
        " [! b]  [! d]  [⋯ a]  [↻ c]  [✗ e]"
    );
    // Blocked が解消されると元の位置に戻る
    let agents = named_agents([Idle, Working, Working, Blocked, Error]);
    assert_eq!(
        plain_with(&agents, 80, &blocked_first),
        " [! d]  [⋯ a]  [↻ b]  [↻ c]  [✗ e]"
    );
    let agents = named_agents([Idle, Working, Working, Done, Error]);
    assert_eq!(
        plain_with(&agents, 80, &blocked_first),
        " [⋯ a]  [↻ b]  [↻ c]  [✓ d]  [✗ e]"
    );
}

#[test]
fn blocked_first_keeps_blocked_visible_when_line_is_cut() {
    use AgentState::*;
    let blocked_first = ViewOptions {
        blocked_first: true,
        ..base()
    };
    let agents = named_agents([Idle, Idle, Idle, Idle, Blocked]);
    assert_eq!(plain(&agents, 12), " [⋯]  [⋯]  ~");
    assert_eq!(plain_with(&agents, 12, &blocked_first), " [!]  [⋯]  ~");
}

#[test]
fn filtered_by_app_filter() {
    let mut codex = agent(2, AgentState::Idle);
    codex.app = Some("codex".to_owned());
    codex.title = Some("codex".to_owned());
    let mut no_app = agent(3, AgentState::Idle);
    no_app.app = None;
    no_app.title = None;
    let agents = [agent(1, AgentState::Idle), codex, no_app];

    let only_claude = ViewOptions {
        app_filter: Some(vec!["claude".to_owned()]),
        ..base()
    };
    assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯ claude]");
    // 絞り込まなければ、app のないエージェントも表示する
    assert_eq!(plain(&agents, 80), " [⋯ claude]  [⋯ codex]  [⋯ #3]");
}

#[test]
fn msgs_are_shortened_evenly() {
    let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
    let b = with_msg(agent(2, AgentState::Blocked), "short");
    // 全体：" [↻ claude] " (12) + 20 + "  " (2) + "[! claude] " (11) + 5 = 50
    assert_eq!(
        plain(&[a.clone(), b.clone()], 50),
        " [↻ claude] abcdefghijklmnopqrst  [! claude] short"
    );
    // 短い msg はそのまま、長い msg が余りを使う
    assert_eq!(
        plain(&[a, b], 42),
        " [↻ claude] abcdefghijk~  [! claude] short"
    );
}

#[test]
fn msgs_are_dropped_when_too_narrow() {
    let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
    let b = with_msg(agent(2, AgentState::Blocked), "short");
    // msg に回せる幅が MIN_MSG_WIDTH 未満になる
    assert_eq!(plain(&[a, b], 35), " [↻ claude]  [! claude]");
}

#[test]
fn names_are_shortened_then_icons_only() {
    let mut a = agent(1, AgentState::Working);
    a.title = Some("long-project-name".to_owned());
    let b = agent(2, AgentState::Done);
    // 名前に回せる幅は 13：短い "claude" に 6、残り 7 を長い方へ
    assert_eq!(
        plain(&[a.clone(), b.clone()], 24),
        " [↻ long-p~]  [✓ claude]"
    );
    assert_eq!(plain(&[a, b], 16), " [↻]  [✓]");
}

#[test]
fn truncates_line_end_when_icons_do_not_fit() {
    let agents: Vec<_> = (0..5).map(|i| agent(i, AgentState::Idle)).collect();
    assert_eq!(plain(&agents, 12), " [⋯]  [⋯]  ~");
}

#[test]
fn wide_characters_are_counted_by_width() {
    let a = [with_msg(agent(1, AgentState::Blocked), "許可してください")];
    // " [! claude] " (12) + 16 = 28
    assert_eq!(plain(&a, 28), " [! claude] 許可してください");
    // msg の幅 8：全角 3 文字（6）+ "~"、残り 1 桁には全角が入らない
    assert_eq!(plain(&a, 20), " [! claude] 許可し~");
}

/// 幅を総当たりで確かめるための、進捗・app のアイコン・cwd・msg・全角文字・絵文字を含むエージェント。
/// 絵文字は、`char` ごとの幅の合計と表示幅が違うもの（❤️ は合計のほうが狭く、👨‍👩‍👧 は広い）
fn mixed_agents() -> Vec<AgentStatus> {
    let mut agents = vec![
        with_msg(
            agent(1, AgentState::Working),
            "テスト❤\u{fe0f}を実行しています",
        ),
        with_msg(
            agent(2, AgentState::Blocked),
            "Allow 👨\u{200d}👩\u{200d}👧 Bash(cargo test)?",
        ),
        agent(3, AgentState::Done),
    ];
    agents[0].progress = Some(100);
    agents[0].app = Some("claude-code".to_owned());
    agents[0].cwd = Some("/home/user/全角❤\u{fe0f}のディレクトリ".into());
    agents[2].title = Some("❤\u{fe0f}全角のタイトル".to_owned());
    agents[2].cwd = Some("/repo/main".into());
    agents
}

/// 異体字セレクタ付きの絵文字を、行の端でも各部分の切り詰めでも、幅を超えずに切る
#[test]
fn emoji_with_variation_selector_never_exceeds_width() {
    let hearts = "❤\u{fe0f}".repeat(8);
    let mut name = agent(1, AgentState::Working);
    name.title = Some(hearts.clone());
    let msg = with_msg(agent(1, AgentState::Working), &hearts);
    for agent in [name, msg] {
        for cols in 0..40 {
            let text = plain(std::slice::from_ref(&agent), cols);
            assert!(display_width(&text) <= cols, "cols={cols}: {text:?}");
        }
    }
}

/// [`width_combinations`] で代表だけにまとめてよいこと：代表と、どの Span の幅も種類も同じになる
#[test]
fn width_representative_has_same_widths() {
    let agents = mixed_agents();
    let widths = |cols, options: &ViewOptions| -> Vec<_> {
        layout(&agents, cols, options)
            .into_iter()
            .map(|s| (display_width(&s.text), s.role, s.fill, s.pane_id))
            .collect()
    };
    for options in option_combinations() {
        let representative = width_representative(&options);
        // どの段階まで縮めても同じことを確かめるため、幅を変える
        for cols in [10, 20, 40, 60, 80, 120, 200] {
            assert_eq!(
                widths(cols, &options),
                widths(cols, &representative),
                "{options:?} cols={cols}"
            );
        }
    }
}

#[test]
fn never_exceeds_width() {
    let agents = mixed_agents();
    for options in width_combinations() {
        for cols in 0..120 {
            let text = plain_with(&agents, cols, &options);
            assert!(
                display_width(&text) <= cols,
                "{options:?} cols={cols}: {text:?}"
            );
        }
    }
}

fn max_agent_width(max_agent_width: usize) -> ViewOptions {
    ViewOptions {
        max_agent_width: Some(max_agent_width),
        ..base()
    }
}

/// 各エージェント分の幅（行頭の余白と区切りを除く）。行末の `~` は、切れたエージェントの分に数える
fn agent_widths(spans: &[Span]) -> Vec<usize> {
    let mut widths: Vec<(u32, usize)> = vec![];
    for span in spans {
        let Some(pane_id) = span.pane_id else {
            continue;
        };
        let span_width = display_width(&span.text);
        match widths.last_mut() {
            Some((id, width)) if *id == pane_id => *width += span_width,
            _ => widths.push((pane_id, span_width)),
        }
    }
    widths.into_iter().map(|(_, width)| width).collect()
}

#[test]
fn max_agent_width_truncates_msg_first() {
    let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
    let b = with_msg(agent(2, AgentState::Blocked), "short");
    // "[↻ claude] " (11) + msg 11 = 22。短い方はそのまま
    assert_eq!(
        plain_with(&[a, b], 80, &max_agent_width(22)),
        " [↻ claude] abcdefghij~  [! claude] short"
    );
}

#[test]
fn max_agent_width_drops_msg_then_truncates_name() {
    let mut a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
    // msg に回せる幅が 5（MIN_MSG_WIDTH 未満）なので省く
    assert_eq!(
        plain_with(&[a.clone()], 80, &max_agent_width(16)),
        " [↻ claude]"
    );
    // 収まる msg なら MIN_MSG_WIDTH 未満の幅でも表示する
    let short = with_msg(agent(1, AgentState::Working), "ok");
    assert_eq!(
        plain_with(&[short], 80, &max_agent_width(16)),
        " [↻ claude] ok"
    );

    a.title = Some("long-project-name".to_owned());
    assert_eq!(
        plain_with(&[a], 80, &max_agent_width(16)),
        " [↻ long-projec~]"
    );
}

#[test]
fn max_agent_width_counts_icon_and_progress() {
    let mut a = with_msg(agent(1, AgentState::Working), "Running");
    a.title = Some("my-project".to_owned());
    a.progress = Some(100);
    assert_eq!(
        plain_with(&[a.clone()], 80, &max_agent_width(18)),
        " [↻ 100% my-proje~]"
    );
    let emoji = ViewOptions {
        state_icon_style: StateIconStyle::Emoji,
        ..max_agent_width(18)
    };
    // 絵文字はバッジの端がない分、要素に回せる幅が広い
    assert_eq!(plain_with(&[a.clone()], 80, &emoji), " ⏳ 100% my-project");
    // 端が Nerd Font の文字のバッジの内側の空白の 2 と、app のアイコンと前の空白の 2 も数える
    a.app = Some("claude-code".to_owned());
    let nerd = ViewOptions {
        max_agent_width: Some(18),
        ..nerd_font(BadgeShape::Slash)
    };
    assert_eq!(
        plain_with(&[a.clone()], 80, &nerd),
        " [ \u{f013} 100% \u{ec82} my-p~ ]"
    );
    // アイコンの後ろの空白も数える（title は MIN_NAME_WIDTH まで縮む）
    let padded = ViewOptions {
        nerd_font_icon_padding: true,
        ..nerd
    };
    assert_eq!(
        plain_with(&[a.clone()], 80, &padded),
        " [ \u{f013} 100% \u{ec82}  my-~ ]"
    );
    // cwd が最後に残るときは括弧の分も数える。MIN_MAX_AGENT_WIDTH はこの幅に合わせている
    a.title = None;
    let cwd = with_cwd(a, "/repo/feature");
    let cwd_last = ViewOptions {
        badge_items: vec![Element::AppIcon, Element::Cwd],
        max_agent_width: Some(MIN_MAX_AGENT_WIDTH),
        ..padded
    };
    assert_eq!(
        plain_with(&[cwd], 80, &cwd_last),
        " [ \u{f013} 100% \u{ec82}  (fea~) ]"
    );
}

#[test]
fn max_agent_width_is_combined_with_row_width() {
    let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
    let b = with_msg(agent(2, AgentState::Blocked), "short");
    let options = max_agent_width(22);
    // 上限で切り詰めた幅（msg 11 と 5）を、行の幅に合わせてさらに均等に縮める
    assert_eq!(
        plain_with(&[a.clone(), b.clone()], 38, &options),
        " [↻ claude] abcdefg~  [! claude] short"
    );
    assert_eq!(plain_with(&[a, b], 24, &options), " [↻ claude]  [! claude]");
}

#[test]
fn max_agent_width_defaults_to_50_and_none_is_unlimited() {
    let msg = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let a = with_msg(agent(1, AgentState::Working), msg);
    // "[↻ claude] " (11) + msg 39 = 50
    assert_eq!(
        plain(std::slice::from_ref(&a), 80),
        " [↻ claude] abcdefghijklmnopqrstuvwxyzABCDEFGHIJKL~"
    );
    let unlimited = ViewOptions {
        max_agent_width: None,
        ..base()
    };
    assert_eq!(
        plain_with(&[a], 80, &unlimited),
        format!(" [↻ claude] {msg}")
    );
}

#[test]
fn max_agent_width_leaves_short_agents_unchanged() {
    let a = with_msg(agent(1, AgentState::Done), "ok");
    assert_eq!(plain_with(&[a], 80, &max_agent_width(40)), " [✓ claude] ok");
}

fn max_agent_width_with_cwd(max: usize) -> ViewOptions {
    ViewOptions {
        max_agent_width: Some(max),
        ..show_cwd()
    }
}

#[test]
fn max_agent_width_drops_msg_then_truncates_cwd_then_drops_cwd() {
    let a = with_msg(
        with_cwd(agent(1, AgentState::Working), "/repo/feature-branch"),
        "Running tests",
    );
    let line = |max| plain_with(std::slice::from_ref(&a), 80, &max_agent_width_with_cwd(max));

    assert_eq!(line(41), " [↻ claude (feature-branch)] Running tests");
    // msg を切り詰める（MIN_MSG_WIDTH まで）
    assert_eq!(line(36), " [↻ claude (feature-branch)] Running~");
    // msg を省き、cwd を切り詰める（MIN_CWD_WIDTH まで）
    assert_eq!(line(32), " [↻ claude (feature-branch)]");
    assert_eq!(line(22), " [↻ claude (feature-~)]");
    assert_eq!(line(17), " [↻ claude (fea~)]");
    // cwd も省き、名前を切り詰める
    assert_eq!(line(16), " [↻ claude]");
    let mut long = a.clone();
    long.title = Some("long-project-name".to_owned());
    assert_eq!(
        plain_with(&[long], 80, &max_agent_width_with_cwd(16)),
        " [↻ long-projec~]"
    );
}

#[test]
fn max_agent_width_with_cwd_is_combined_with_row_width() {
    let a = with_cwd(agent(1, AgentState::Working), "/repo/feature-branch");
    let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
    let options = max_agent_width_with_cwd(22);
    // 上限で切り詰めた cwd（9 と 4）を、行の幅に合わせてさらに均等に縮める
    assert_eq!(
        plain_with(&[a.clone(), b.clone()], 80, &options),
        " [↻ claude (feature-~)]  [✓ claude (main)]"
    );
    assert_eq!(
        plain_with(&[a, b], 40, &options),
        " [↻ claude (featur~)]  [✓ claude (main)]"
    );
}

#[test]
fn never_exceeds_max_agent_width() {
    let agents = mixed_agents();
    for base in width_combinations() {
        for max in MIN_MAX_AGENT_WIDTH..40 {
            let options = ViewOptions {
                max_agent_width: Some(max),
                ..base.clone()
            };
            for cols in 0..100 {
                // 回数が多いので、layout は 1 回だけ呼び、失敗時のメッセージは失敗したときだけ作る
                let spans = layout(&agents, cols, &options);
                let text = plain_text(&spans);
                assert!(
                    display_width(&text) <= cols,
                    "{options:?} cols={cols}: {text:?}"
                );
                for width in agent_widths(&spans) {
                    assert!(width <= max, "{options:?} cols={cols}: {text:?}");
                }
            }
        }
    }
}

/// `nerd_font` が true のときの既定値と同じく、状態アイコンを Nerd Font の文字にし、
/// バッジの先頭に app のアイコンを置く。バッジの端は `badge_shape` の形にする
fn nerd_font(badge_shape: BadgeShape) -> ViewOptions {
    ViewOptions {
        badge_shape,
        badge_items: vec![Element::AppIcon, Element::Title, Element::TabName],
        ..state_icons(StateIconStyle::NerdFont)
    }
}

#[test]
fn app_icon_is_placed_where_listed() {
    use Element::*;
    let mut a = with_msg(agent(1, AgentState::Working), "Running tests");
    a.app = Some("claude-code".to_owned());
    let mut b = agent(2, AgentState::Done);
    b.app = Some("codex".to_owned());
    let agents = [a, b];
    // 対応するアイコンのない app（codex）には付けない。
    // エージェント同士の区切りは、バッジの端が Nerd Font の文字でも空白
    for shape in SHAPES {
        assert_eq!(
            plain_with(&agents, 80, &nerd_font(shape)),
            " [ \u{f013} \u{ec82} claude ] Running tests  [ \u{f00c} claude ]",
            "{shape:?}"
        );
    }
    let with_dir = [with_cwd(agents[0].clone(), "/repo/main")];
    let options = |badge_items: &[Element], text_items: &[Element]| ViewOptions {
        badge_items: badge_items.to_vec(),
        text_items: text_items.to_vec(),
        ..nerd_font(BadgeShape::Slash)
    };
    assert_eq!(
        plain_with(&with_dir, 80, &options(&[AppIcon, Title, Cwd], &[])),
        " [ \u{f013} \u{ec82} claude (main) ] Running tests"
    );
    // バッジの外でも、書いた位置に置く（msg の前は `: `）
    assert_eq!(
        plain_with(&with_dir, 80, &options(&[], &[AppIcon, Title, Cwd])),
        " [ \u{f013} ] \u{ec82} claude (main): Running tests"
    );
    assert_eq!(
        plain_with(&with_dir, 80, &options(&[Title], &[Cwd, AppIcon])),
        " [ \u{f013} claude ] (main) \u{ec82}: Running tests"
    );
    // バッジの末尾にも置ける（アイコンの右がバッジの右端に重なることがある。src/view/badge.rs を参照）
    assert_eq!(
        plain_with(&with_dir, 80, &options(&[Title, AppIcon], &[])),
        " [ \u{f013} claude \u{ec82} ] Running tests"
    );
    // 一覧になければ、ほかを Nerd Font の文字にしていても app のアイコンを付けない
    assert_eq!(
        plain_with(&agents, 80, &options(&[Title, TabName], &[])),
        " [ \u{f013} claude ] Running tests  [ \u{f00c} claude ]"
    );
}

#[test]
fn state_icon_style_badge_shape_and_app_icon_are_independent() {
    let mut a = with_msg(agent(1, AgentState::Blocked), "msg");
    a.title = None;
    a.app = Some("claude-code".to_owned());
    let a = [a];
    let styles = [
        (StateIconStyle::Symbol, "!"),
        (StateIconStyle::Alphabet, "B"),
        (StateIconStyle::NerdFont, "\u{f071}"),
    ];
    let shapes = [
        (BadgeShape::Square, " ", " "),
        (BadgeShape::Slash, "\u{e0ba} ", " \u{e0bc}"),
        (BadgeShape::Arrow, "\u{e0b2} ", " \u{e0b0}"),
        (BadgeShape::Round, "\u{e0b6} ", " \u{e0b4}"),
    ];
    let raw = |state_icon_style, badge_shape, badge_items: &[Element]| -> String {
        let options = ViewOptions {
            state_icon_style,
            badge_shape,
            badge_items: badge_items.to_vec(),
            ..base()
        };
        layout(&a, 80, &options)
            .into_iter()
            .map(|s| s.text)
            .collect()
    };
    let item_lists = [
        (&[Element::AppName][..], ""),
        (&[Element::AppIcon, Element::AppName][..], "\u{ec82} "),
    ];
    for (shape, left, right) in shapes {
        for (badge_items, app_icon) in item_lists {
            for (style, icon) in styles {
                assert_eq!(
                    raw(style, shape, badge_items),
                    format!(" {left}{icon} {app_icon}claude-code{right} msg"),
                    "{style:?} {shape:?} {badge_items:?}"
                );
            }
            // 絵文字アイコンはバッジにならないので、badge_shape は効かない
            assert_eq!(
                raw(StateIconStyle::Emoji, shape, badge_items),
                format!(" ❗ {app_icon}claude-code: msg"),
                "{shape:?} {badge_items:?}"
            );
        }
    }
}

#[test]
fn nerd_font_widths_are_counted_when_shortening() {
    let mut a = agent(1, AgentState::Working);
    a.app = Some("claude-code".to_owned());
    a.title = Some("long-project-name".to_owned());
    let b = agent(2, AgentState::Done);
    let agents = [a, b];
    let options = ViewOptions {
        max_agent_width: None,
        ..nerd_font(BadgeShape::Slash)
    };
    // 固定部分：" " (1) + "[ \u{f013} ]" (5) + "  " (2) + "[ \u{f00c} ]" (5) = 13
    // title に付く部分：" \u{ec82}" + " " (3) と " " (1) = 4
    // title に回せる幅は 29 - 13 - 4 = 12：短い "claude" に 6、残り 6 を長い方へ
    assert_eq!(
        plain_with(&agents, 29, &options),
        " [ \u{f013} \u{ec82} long-~ ]  [ \u{f00c} claude ]"
    );
    // アイコンの後ろに空白を置くと、その分も数える
    // 固定部分は 15（状態アイコンの後ろの空白 2）、title に付く部分は 5（app のアイコンの後ろの空白 1）
    let padded = ViewOptions {
        nerd_font_icon_padding: true,
        ..options.clone()
    };
    assert_eq!(
        plain_with(&agents, 32, &padded),
        " [ \u{f013}  \u{ec82}  long-~ ]  [ \u{f00c}  claude ]"
    );
    // 状態アイコンだけにする。app のアイコンも最後に残る要素と一緒に省く
    assert_eq!(
        plain_with(&agents, 20, &options),
        " [ \u{f013} ]  [ \u{f00c} ]"
    );
}

#[test]
fn nerd_font_icon_padding_adds_space_after_icons() {
    let mut a = with_msg(agent(1, AgentState::Blocked), "Allow?");
    a.app = Some("claude-code".to_owned());
    let mut b = with_cwd(agent(2, AgentState::Done), "/repo/main");
    b.app = Some("claude-code".to_owned());
    b.progress = Some(42);
    let mut agents = [a, b];
    for a in &mut agents {
        a.title = None;
    }
    let padded = ViewOptions {
        badge_items: vec![Element::AppIcon, Element::AppName, Element::Cwd],
        nerd_font_icon_padding: true,
        ..nerd_font(BadgeShape::Slash)
    };
    // 進捗があれば、その前の空白が状態アイコンの右隣になるので足さない
    assert_eq!(
        plain_with(&agents, 80, &padded),
        " [ \u{f071}  \u{ec82}  claude-code ] Allow?  [ \u{f00c} 42% \u{ec82}  claude-code (main) ]"
    );
    // 強調すると msg の前が `: ` になる（状態アイコンと app のアイコンの間は空白 2 つ）
    let highlighted = ViewOptions {
        highlight_attention: true,
        ..padded.clone()
    };
    assert_eq!(
        plain_with(&agents, 80, &highlighted),
        " [ \u{f071}  \u{ec82}  claude-code: Allow? ]  [ \u{f00c} 42% \u{ec82}  claude-code (main) ]"
    );
    // 状態アイコンと app のアイコンのうち、Nerd Font のアイコンの後ろにだけ置く
    let status_icon_only = ViewOptions {
        badge_items: vec![Element::AppName, Element::Cwd],
        ..padded.clone()
    };
    assert_eq!(
        plain_with(&agents, 80, &status_icon_only),
        " [ \u{f071}  claude-code ] Allow?  [ \u{f00c} 42% claude-code (main) ]"
    );
    let app_icon_only = ViewOptions {
        state_icon_style: StateIconStyle::Symbol,
        ..padded.clone()
    };
    assert_eq!(
        plain_with(&agents, 80, &app_icon_only),
        " [ ! \u{ec82}  claude-code ] Allow?  [ ✓ 42% \u{ec82}  claude-code (main) ]"
    );
    // Nerd Font のアイコンを使わなければ、バッジの端が Nerd Font の文字でも何も変わらない
    for badge_shape in [BadgeShape::Square, BadgeShape::Slash] {
        let options = ViewOptions {
            badge_shape,
            ..base()
        };
        let padded = ViewOptions {
            nerd_font_icon_padding: true,
            ..options.clone()
        };
        assert_eq!(
            plain_with(&agents, 80, &padded),
            plain_with(&agents, 80, &options),
            "{badge_shape:?}"
        );
    }
}

#[test]
fn nerd_font_icon_padding_puts_space_after_every_nerd_icon() {
    use AgentState::*;
    let states = [Idle, Working, Blocked, Done, Error];
    let icons: Vec<char> = states
        .iter()
        .map(|&s| StateIconStyle::NerdFont.icon(s))
        .chain(app_icon("claude-code"))
        .flat_map(str::chars)
        .collect();
    let mut agents = vec![];
    for (i, &state) in states.iter().enumerate() {
        let mut a = agent(i as u32, state);
        a.app = Some("claude-code".to_owned());
        agents.push(with_msg(a.clone(), "msg"));
        a.progress = Some(42);
        agents.push(a);
    }
    let padded = option_combinations()
        .into_iter()
        .filter(|o| o.nerd_font_icon_padding);
    for options in padded {
        for cols in [20, 40, 80, 200] {
            let text: String = layout(&agents, cols, &options)
                .into_iter()
                .map(|s| s.text)
                .collect();
            let chars: Vec<char> = text.chars().collect();
            for (i, c) in chars.iter().enumerate() {
                if icons.contains(c) {
                    // 行末で切ったときの ~ は例外（それ以上続かない）
                    let next = chars.get(i + 1).copied();
                    assert!(
                        matches!(next, Some(' ') | Some(ELLIPSIS) | None),
                        "{options:?} cols={cols}: {text:?}"
                    );
                }
            }
        }
    }
}

/// 状態の色で塗った部分を、続いている範囲ごとにまとめたもの（選んでいるときだけ塗る部分は含まない）
fn filled_runs(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> Vec<String> {
    let mut runs: Vec<String> = vec![];
    let mut prev = Fill::None;
    for span in layout(agents, cols, options) {
        if matches!(span.fill, Fill::State(_)) {
            match runs.last_mut() {
                Some(run) if span.fill == prev => run.push_str(&span.text),
                _ => runs.push(span.text),
            }
        }
        prev = span.fill;
    }
    runs
}

/// バッジの端の文字（[`plain_with`] で置き換える前の実際の文字）
fn badge_ends(agents: &[AgentStatus], options: &ViewOptions) -> Vec<String> {
    layout(agents, 80, options)
        .into_iter()
        .filter(|s| matches!(s.role, Role::BadgeEnd(_)))
        .map(|s| s.text)
        .collect()
}

/// 端に Nerd Font の文字を使う形
const SHAPES: [BadgeShape; 3] = [BadgeShape::Slash, BadgeShape::Arrow, BadgeShape::Round];

#[test]
fn badge_ends_follow_badge_shape() {
    let a = [agent(1, AgentState::Working)];
    let shaped = |badge_shape, state_icon_style| ViewOptions {
        badge_shape,
        state_icon_style,
        ..base()
    };
    // 既定は空白
    assert_eq!(badge_ends(&a, &base()), [" ", " "]);
    for style in [
        StateIconStyle::Symbol,
        StateIconStyle::Alphabet,
        StateIconStyle::NerdFont,
    ] {
        let square = shaped(BadgeShape::Square, style);
        assert_eq!(badge_ends(&a, &square), [" ", " "], "{style:?}");
        for shape in SHAPES {
            let (left, right) = shape.glyphs().unwrap();
            assert_eq!(
                badge_ends(&a, &shaped(shape, style)),
                [left, right],
                "{style:?} {shape:?}"
            );
        }
    }
    // 絵文字はバッジにしない
    for shape in [BadgeShape::Square].into_iter().chain(SHAPES) {
        assert!(
            badge_ends(&a, &shaped(shape, StateIconStyle::Emoji)).is_empty(),
            "{shape:?}"
        );
    }
}

#[test]
fn badge_without_items_fills_icon_and_progress() {
    let mut a = with_msg(agent(1, AgentState::Working), "Running");
    a.progress = Some(42);
    let a = [a];
    let options = outside(base());
    assert_eq!(plain_with(&a, 80, &options), " [↻ 42%] claude: Running");
    assert_eq!(filled_runs(&a, 80, &options), [" ↻ 42% "]);
    // Nerd Font の端の文字は塗らずに前景色で描くので、塗るのはその内側だけ
    for shape in SHAPES {
        assert_eq!(
            filled_runs(&a, 80, &outside(nerd_font(shape))),
            [" \u{f013} 42% "],
            "{shape:?}"
        );
    }
    assert!(filled_runs(&a, 80, &outside(state_icons(StateIconStyle::Emoji))).is_empty());
}

fn highlight(options: ViewOptions) -> ViewOptions {
    ViewOptions {
        highlight_attention: true,
        ..options
    }
}

#[test]
fn highlight_attention_fills_whole_blocked_and_error_agents() {
    use AgentState::*;
    let mut agents = named_agents([Idle, Blocked, Working, Done, Error]);
    for a in &mut agents {
        if a.title.as_deref() == Some("b") {
            a.msg = Some("Allow Bash?".to_owned());
        }
    }
    // 強調の範囲が分かりやすいよう、要素はバッジに入れない（入れるときは badge_items_with_highlight_attention）
    let base = outside(base());
    let options = highlight(base.clone());
    let line = plain_with(&agents, 80, &options);
    assert_eq!(line, " [⋯] a  [! b: Allow Bash?]  [↻] c  [✓] d  [✗ e]");
    assert_eq!(
        filled_runs(&agents, 80, &options),
        [" ⋯ ", " ! b: Allow Bash? ", " ↻ ", " ✓ ", " ✗ e "]
    );
    // 端の文字を移すだけなので、強調しても幅は変わらない
    assert_eq!(
        display_width(&line),
        display_width(&plain_with(&agents, 80, &base))
    );
    // 既定では強調しない
    assert_eq!(
        plain_with(&agents, 80, &base),
        " [⋯] a  [!] b: Allow Bash?  [↻] c  [✓] d  [✗] e"
    );
}

#[test]
fn highlight_attention_with_nerd_font_moves_badge_ends_to_agent_end() {
    let mut a = with_msg(agent(1, AgentState::Blocked), "Allow?");
    a.app = Some("claude-code".to_owned());
    let a = [a];
    for shape in SHAPES {
        let options = highlight(nerd_font(shape));
        let raw: String = layout(&a, 80, &options)
            .into_iter()
            .map(|s| s.text)
            .collect();
        let (left, right) = shape.glyphs().unwrap();
        assert_eq!(
            raw,
            format!(" {left} \u{f071} \u{ec82} claude: Allow? {right}"),
            "{shape:?}"
        );
        assert_eq!(
            filled_runs(&a, 80, &options),
            [" \u{f071} \u{ec82} claude: Allow? "],
            "{shape:?}"
        );
    }
}

#[test]
fn highlight_attention_with_emoji_fills_without_ends() {
    let agents = [
        with_msg(agent(1, AgentState::Error), "failed"),
        agent(2, AgentState::Done),
    ];
    let options = highlight(state_icons(StateIconStyle::Emoji));
    assert_eq!(
        plain_with(&agents, 80, &options),
        " ❌ claude: failed  ✅ claude"
    );
    assert_eq!(filled_runs(&agents, 80, &options), ["❌ claude: failed"]);
}

/// `badge_items` の要素を、同じ順で `text_items` の先頭に移す
fn outside(options: ViewOptions) -> ViewOptions {
    let mut text_items = options.badge_items.clone();
    text_items.extend(&options.text_items);
    ViewOptions {
        badge_items: vec![],
        text_items,
        ..options
    }
}

#[test]
fn badge_items_go_into_badge() {
    let mut a = with_msg(agent(1, AgentState::Working), "Running tests");
    a.progress = Some(42);
    let mut b = agent(2, AgentState::Done);
    b.title = Some("codex".to_owned());
    let agents = [a, b];
    // 既定で title とタブ名をバッジに入れる
    let line = plain(&agents, 80);
    // バッジの右端が要素と msg の境目になるので、msg の前は `: ` ではなく空白
    assert_eq!(line, " [↻ 42% claude] Running tests  [✓ codex]");
    assert_eq!(
        filled_runs(&agents, 80, &base()),
        [" ↻ 42% claude ", " ✓ codex "]
    );
    // バッジの外に移すと、状態アイコンと進捗だけをバッジにし、msg の前は `: `
    let without = outside(base());
    let line_without = plain_with(&agents, 80, &without);
    assert_eq!(line_without, " [↻ 42%] claude: Running tests  [✓] codex");
    // 違いは端の文字の位置と msg の前の区切りだけなので、区切りの差の 1 だけ狭い
    assert_eq!(display_width(&line) + 1, display_width(&line_without));
}

#[test]
fn badge_items_include_cwd_and_follow_shortening() {
    let options = show_cwd();
    let a = with_msg(
        with_cwd(agent(1, AgentState::Working), "/repo/feature-branch"),
        "Running tests",
    );
    let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
    let agents = [a, b];
    let line = |cols| plain_with(&agents, cols, &options);

    assert_eq!(
        line(61),
        " [↻ claude (feature-branch)] Running tests  [✓ claude (main)]"
    );
    assert_eq!(
        filled_runs(&agents, 61, &options),
        [" ↻ claude (feature-branch) ", " ✓ claude (main) "]
    );
    // 1. msg を切り詰める
    assert_eq!(
        line(54),
        " [↻ claude (feature-branch)] Runni~  [✓ claude (main)]"
    );
    // 2. msg を省き、cwd を切り詰める（~ もバッジの中）
    assert_eq!(line(53), " [↻ claude (feature-branch)]  [✓ claude (main)]");
    assert_eq!(line(37), " [↻ claude (fea~)]  [✓ claude (main)]");
    // 3. cwd を省き、名前を切り詰める
    assert_eq!(line(36), " [↻ claude]  [✓ claude]");
    // 4. アイコンだけにすると、状態アイコンと進捗だけのバッジになる
    assert_eq!(line(16), " [↻]  [✓]");
    assert_eq!(filled_runs(&agents, 16, &options), [" ↻ ", " ✓ "]);
}

#[test]
fn badge_items_put_truncated_element_into_badge() {
    let mut a = agent(1, AgentState::Working);
    a.title = Some("long-project-name".to_owned());
    let b = agent(2, AgentState::Done);
    let agents = [a, b];
    let options = base();
    assert_eq!(
        plain_with(&agents, 24, &options),
        " [↻ long-p~]  [✓ claude]"
    );
    assert_eq!(
        filled_runs(&agents, 24, &options),
        [" ↻ long-p~ ", " ✓ claude "]
    );
    // 縮めたときも max_agent_width で切り詰めたときも、端の文字を移すだけ
    let long = [agents[0].clone()];
    assert_eq!(
        plain_with(&long, 80, &max_agent_width(16)),
        " [↻ long-projec~]"
    );
}

#[test]
fn moving_items_out_of_badge_only_moves_badge_end_without_msg() {
    // msg があると区切り（BadgeExtent::msg_separator）の幅も変わるので、msg のないエージェントで確かめる
    let mut agents = mixed_agents();
    for a in &mut agents {
        a.msg = None;
    }
    // 右端と一緒に内側の空白（Nerd Font の端の文字のとき）も移るので、端の文字と空白を除いた中身と幅で比べる
    let strip = |line: &str| -> String {
        line.chars()
            .filter(|&c| !matches!(c, '[' | ']' | ' '))
            .collect()
    };
    let max_agent_widths = [
        None,
        Some(MIN_MAX_AGENT_WIDTH),
        Some(DEFAULT_MAX_AGENT_WIDTH),
    ];
    // mixed_agents はどれも title を持つので、#<pane_id> の位置（badge_items が空かどうかで変わる）は関係しない
    for options in width_combinations()
        .into_iter()
        .filter(|o| !o.badge_items.is_empty())
    {
        for max_agent_width in max_agent_widths {
            let options = ViewOptions {
                max_agent_width,
                ..options.clone()
            };
            let moved = outside(options.clone());
            for cols in 0..120 {
                let with = plain_with(&agents, cols, &options);
                let without = plain_with(&agents, cols, &moved);
                assert_eq!(
                    display_width(&with),
                    display_width(&without),
                    "{options:?} cols={cols}"
                );
                assert_eq!(strip(&with), strip(&without), "{options:?} cols={cols}");
            }
        }
    }
}

#[test]
fn default_nerd_font_items_end_badge_with_text() {
    let mut a = with_msg(agent(1, AgentState::Working), "Running tests");
    a.app = Some("claude-code".to_owned());
    let a = [a];
    let options = nerd_font(BadgeShape::Round);
    let raw = |options: &ViewOptions| -> String {
        layout(&a, 80, options)
            .into_iter()
            .map(|s| s.text)
            .collect()
    };
    // 既定では app のアイコンを先頭に置くので、バッジの右端の手前はアイコンにならない
    assert_eq!(
        raw(&options),
        " \u{e0b6} \u{f013} \u{ec82} claude \u{e0b4} Running tests"
    );
    // 両端の内側の空白も含めて、左端と右端の間を塗る
    assert_eq!(
        filled_runs(&a, 80, &options),
        [" \u{f013} \u{ec82} claude "]
    );
    // アイコンの後ろの空白もバッジの中に入る
    let padded = ViewOptions {
        nerd_font_icon_padding: true,
        ..options
    };
    assert_eq!(
        raw(&padded),
        " \u{e0b6} \u{f013}  \u{ec82}  claude \u{e0b4} Running tests"
    );
}

#[test]
fn badge_items_with_emoji_are_shown_like_text_items() {
    let agents = [
        with_msg(agent(1, AgentState::Error), "failed"),
        agent(2, AgentState::Done),
    ];
    let emoji = state_icons(StateIconStyle::Emoji);
    assert_eq!(
        plain_with(&agents, 80, &emoji),
        plain_with(&agents, 80, &outside(emoji.clone()))
    );
    assert!(filled_runs(&agents, 80, &emoji).is_empty());
    // 強調するときは、強調したエージェントだけ全体を塗る
    assert_eq!(
        filled_runs(&agents, 80, &highlight(emoji)),
        ["❌ claude: failed"]
    );
}

#[test]
fn badge_items_with_highlight_attention() {
    use AgentState::*;
    let mut agents = named_agents([Idle, Blocked, Working, Done, Error]);
    for a in &mut agents {
        match a.title.as_deref() {
            Some("b") => a.msg = Some("Allow Bash?".to_owned()),
            Some("c") => a.msg = Some("Running".to_owned()),
            _ => {}
        }
    }
    let options = highlight(base());
    // 強調したエージェント（b, e）は全体を塗り、要素と msg の間に境目がないので `: ` にする。
    // それ以外は badge_items までをバッジにし、msg の前は空白
    assert_eq!(
        plain_with(&agents, 80, &options),
        " [⋯ a]  [! b: Allow Bash?]  [↻ c] Running  [✓ d]  [✗ e]"
    );
    assert_eq!(
        filled_runs(&agents, 80, &options),
        [" ⋯ a ", " ! b: Allow Bash? ", " ↻ c ", " ✓ d ", " ✗ e "]
    );
}

/// a〜d がそれぞれ Working / Blocked / Done / Error で、msg を持つ
fn agents_with_msgs() -> Vec<AgentStatus> {
    use AgentState::*;
    [
        ("a", Working, "Running tests"),
        ("b", Blocked, "Allow Bash?"),
        ("c", Done, "Done"),
        ("d", Error, "build failed"),
    ]
    .into_iter()
    .zip(1..)
    .map(|((name, state, msg), pane)| {
        let mut a = with_msg(agent(pane, state), msg);
        a.title = Some(name.to_owned());
        a
    })
    .collect()
}

fn msg_mode(msg_mode: MsgMode) -> ViewOptions {
    ViewOptions { msg_mode, ..base() }
}

#[test]
fn msg_mode_selects_agents_showing_msg() {
    let agents = agents_with_msgs();
    let attention = " [↻ a]  [! b] Allow Bash?  [✓ c]  [✗ d] build failed";
    assert_eq!(
        plain_with(&agents, 80, &msg_mode(MsgMode::Attention)),
        attention
    );
    // 既定は attention
    assert_eq!(plain_with(&agents, 80, &ViewOptions::default()), attention);
    assert_eq!(
        plain_with(&agents, 80, &msg_mode(MsgMode::All)),
        " [↻ a] Running tests  [! b] Allow Bash?  [✓ c] Done  [✗ d] build failed"
    );
    assert_eq!(
        plain_with(&agents, 80, &msg_mode(MsgMode::None)),
        " [↻ a]  [! b]  [✓ c]  [✗ d]"
    );
}

#[test]
fn hidden_msg_leaves_room_for_others() {
    let a = with_msg(
        with_cwd(agent(1, AgentState::Working), "/repo/feature-branch"),
        "Running tests",
    );
    let b = with_msg(
        with_cwd(agent(2, AgentState::Blocked), "/repo/main"),
        "Allow Bash?",
    );
    let agents = [a, b];
    let options = |msg_mode| ViewOptions {
        msg_mode,
        ..show_cwd()
    };
    // all では 2 つの msg で幅を分け合い、どちらも MIN_MSG_WIDTH を下回るので msg を省く
    assert_eq!(
        plain_with(&agents, 59, &options(MsgMode::All)),
        " [↻ claude (feature-branch)]  [! claude (main)]"
    );
    // attention では Working の msg の分が空き、cwd を縮めずに Blocked の msg を表示できる
    assert_eq!(
        plain_with(&agents, 59, &options(MsgMode::Attention)),
        " [↻ claude (feature-branch)]  [! claude (main)] Allow Bash?"
    );
    assert_eq!(
        plain_with(&agents, 54, &options(MsgMode::Attention)),
        " [↻ claude (feature-branch)]  [! claude (main)] Allow~"
    );
}

#[test]
fn msg_mode_attention_matches_highlighted_agents() {
    let agents = agents_with_msgs();
    let options = highlight(msg_mode(MsgMode::Attention));
    assert_eq!(
        plain_with(&agents, 80, &options),
        " [↻ a]  [! b: Allow Bash?]  [✓ c]  [✗ d: build failed]"
    );
    assert_eq!(
        filled_runs(&agents, 80, &options),
        [
            " ↻ a ",
            " ! b: Allow Bash? ",
            " ✓ c ",
            " ✗ d: build failed "
        ]
    );
}
