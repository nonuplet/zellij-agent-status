//! レイアウト：どのエージェントを、どの順で、どの文字列をどの役割で並べるかを決める。
//!
//! 幅が足りないときは、次の順に縮める。
//! 1. msg を各エージェントで均等に切り詰める
//! 2. msg を省き、名前を均等に切り詰める
//! 3. アイコン（と進捗）だけにする
//! 4. それでも収まらなければ、行の末尾を切る

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::ViewOptions;
use crate::model::{AgentState, AgentStatus};

/// 行頭の余白
pub(super) const PAD: &str = " ";
/// エージェント同士の区切り
const SEP: &str = "  ";
/// 切り詰めたことを示す記号。
/// `…`（U+2026）は East Asian Width が Ambiguous で、端末によっては 2 桁になるため使わない。
const ELLIPSIS: char = '~';
/// 切り詰めた msg / 名前がこの幅を下回るなら、切り詰めずに次の段階へ進む
const MIN_MSG_WIDTH: usize = 6;
const MIN_NAME_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// 余白や区切り
    Plain,
    /// アイコンと進捗。状態の色で塗る。
    State(AgentState),
    Name,
    Msg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) text: String,
    pub(super) role: Role,
}

impl Span {
    pub(super) fn new(text: impl Into<String>, role: Role) -> Self {
        Span {
            text: text.into(),
            role,
        }
    }
}

/// 1 エージェント分の表示内容（切り詰める前）
struct Item {
    state: AgentState,
    icon: &'static str,
    /// `42%` の形
    progress: Option<String>,
    name: String,
    msg: Option<String>,
}

impl Item {
    /// 名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決める（`show_app` が false なら `app` を飛ばす）。
    /// Claude Code は `title` を送らないため、タブ名で複数のエージェントを見分けられるようにしている。
    /// 名前は必ずある前提で [`plan`] と [`Item::spans`] を書いているので、最後は `#<pane_id>` にする。
    fn new(agent: &AgentStatus, options: &ViewOptions) -> Self {
        let app = agent.app.as_deref().filter(|_| options.show_app);
        let name = clean(agent.title.as_deref())
            .or_else(|| clean(agent.tab_name.as_deref()))
            .or_else(|| clean(app))
            .unwrap_or_else(|| format!("#{}", agent.pane_id));
        Item {
            state: agent.state,
            icon: options.icons.icon(agent.state),
            progress: agent.progress.map(|p| format!("{p}%")),
            name,
            msg: clean(agent.msg.as_deref()),
        }
    }

    /// アイコンと進捗の幅（常に表示する部分）
    fn head_width(&self) -> usize {
        self.icon.width() + self.progress.as_ref().map_or(0, |p| 1 + p.width())
    }

    /// `name` / `msg` は表示する幅の上限。None なら表示しない。
    fn spans(&self, name: Option<usize>, msg: Option<usize>) -> Vec<Span> {
        let mut spans = vec![Span::new(self.icon, Role::State(self.state))];
        if let Some(progress) = &self.progress {
            spans.push(Span::new(format!(" {progress}"), Role::State(self.state)));
        }
        if let Some(width) = name {
            spans.push(Span::new(" ", Role::Plain));
            spans.push(Span::new(truncate(&self.name, width), Role::Name));
            if let (Some(text), Some(width)) = (&self.msg, msg) {
                spans.push(Span::new(": ", Role::Plain));
                spans.push(Span::new(truncate(text, width), Role::Msg));
            }
        }
        spans
    }
}

/// 外部から届いた文字列を 1 行の表示用にする。
/// 制御文字（改行や ESC など）はそのまま出すと画面が崩れるので空白に置き換える。
/// 空になったら None。
pub(super) fn clean(text: Option<&str>) -> Option<String> {
    let text: String = text?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// [`ViewOptions::shows`] が true のエージェントだけを、(タブ, ペイン) の順に並べる。
///
/// 受け取る一覧の順序には依存しない（Collector は順序を持たない）。
/// 状態の優先度で並べ替えると、状態が変わるたびに位置が入れ替わって追いにくいため、既定では状態を見ない。
/// [`ViewOptions::blocked_first`] が true のときだけ、見落としやすく省略されやすい後ろのほうに
/// 操作待ちが埋もれないよう、`Blocked` を先頭に割り込ませる。`Blocked` 同士、それ以外同士は (タブ, ペイン) の順のまま。
/// `Error` などは操作待ちではないので割り込ませない。
pub(super) fn layout<'a>(
    agents: impl IntoIterator<Item = &'a AgentStatus>,
    cols: usize,
    options: &ViewOptions,
) -> Vec<Span> {
    let mut agents: Vec<&AgentStatus> = agents.into_iter().filter(|a| options.shows(a)).collect();
    agents.sort_by_key(|a| {
        let blocked = options.blocked_first && a.state == AgentState::Blocked;
        (!blocked, a.tab, a.pane_id)
    });
    let items: Vec<Item> = agents.into_iter().map(|a| Item::new(a, options)).collect();
    if items.is_empty() {
        return vec![];
    }

    let mut spans = vec![Span::new(PAD, Role::Plain)];
    for (i, (item, (name, msg))) in items.iter().zip(plan(&items, cols)).enumerate() {
        if i > 0 {
            spans.push(Span::new(SEP, Role::Plain));
        }
        spans.extend(item.spans(name, msg));
    }
    clamp(spans, cols)
}

/// 各エージェントの (名前の幅, msg の幅) を決める。None は表示しない。
fn plan(items: &[Item], cols: usize) -> Vec<(Option<usize>, Option<usize>)> {
    let fixed = PAD.width()
        + SEP.width() * (items.len() - 1)
        + items.iter().map(Item::head_width).sum::<usize>();
    let names: Vec<usize> = items.iter().map(|i| i.name.width()).collect();
    // 名前の前の空白を含む
    let names_total = names.iter().map(|w| 1 + w).sum::<usize>();
    let msgs: Vec<usize> = items
        .iter()
        .filter_map(|i| i.msg.as_ref().map(|m| m.width()))
        .collect();
    // msg の前の ": "
    let msgs_overhead = 2 * msgs.len();

    if let Some(room) = cols.checked_sub(fixed + names_total + msgs_overhead)
        && let Some(shares) = fit(&msgs, room, MIN_MSG_WIDTH)
    {
        let mut shares = shares.into_iter();
        return items
            .iter()
            .zip(names)
            .map(|(item, name)| (Some(name), item.msg.as_ref().and_then(|_| shares.next())))
            .collect();
    }

    if let Some(room) = cols.checked_sub(fixed + items.len())
        && let Some(shares) = fit(&names, room, MIN_NAME_WIDTH)
    {
        return shares.into_iter().map(|w| (Some(w), None)).collect();
    }

    vec![(None, None); items.len()]
}

/// `room` を [`fair_share`] で分け、切り詰めた結果が `min` を下回るものがあれば None。
fn fit(widths: &[usize], room: usize, min: usize) -> Option<Vec<usize>> {
    let shares = fair_share(widths, room);
    let ok = widths
        .iter()
        .zip(&shares)
        .all(|(&width, &share)| share >= width || share >= min);
    ok.then_some(shares)
}

/// 合計 `total` の幅を、各要素の幅（`widths`）を上限として均等に分ける。
/// 短いものに余った分は、長いものに回す。
fn fair_share(widths: &[usize], total: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..widths.len()).collect();
    order.sort_by_key(|&i| widths[i]);
    let mut shares = vec![0; widths.len()];
    let mut left = total;
    for (k, &i) in order.iter().enumerate() {
        let share = (left / (order.len() - k)).min(widths[i]);
        shares[i] = share;
        left -= share;
    }
    shares
}

/// 先頭から幅 `width` に収まるだけの文字を取り出す。
fn take_width(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

/// 幅 `width` に収まらなければ、末尾を [`ELLIPSIS`] にして切り詰める。
fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut text = take_width(text, width - 1);
    text.push(ELLIPSIS);
    text
}

/// 行全体が `cols` に収まらなければ、末尾を切って [`ELLIPSIS`] を付ける。
pub(super) fn clamp(spans: Vec<Span>, cols: usize) -> Vec<Span> {
    if spans.iter().map(|s| s.text.width()).sum::<usize>() <= cols {
        return spans;
    }
    if cols == 0 {
        return vec![];
    }
    let mut left = cols - 1;
    let mut out = vec![];
    for span in spans {
        let text = take_width(&span.text, left);
        let width = text.width();
        let cut = text.len() < span.text.len();
        if !text.is_empty() {
            out.push(Span::new(text, span.role));
        }
        left -= width;
        if cut {
            break;
        }
    }
    out.push(Span::new(ELLIPSIS, Role::Plain));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::IconStyle;
    use crate::view::tests::{agent, with_msg};

    fn plain(agents: &[AgentStatus], cols: usize) -> String {
        plain_with(agents, cols, &ViewOptions::default())
    }

    fn plain_with(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
        layout(agents, cols, options)
            .into_iter()
            .map(|s| s.text)
            .collect()
    }

    #[test]
    fn empty_list_renders_nothing() {
        assert_eq!(plain(&[], 80), "");
    }

    #[test]
    fn shows_icon_name_and_msg() {
        let a = with_msg(agent(1, AgentState::Working), "Running tests");
        assert_eq!(plain(&[a], 80), " ⟳ claude: Running tests");
    }

    #[test]
    fn shows_progress_after_icon() {
        let mut a = agent(1, AgentState::Working);
        a.progress = Some(42);
        assert_eq!(plain(&[a], 80), " ⟳ 42% claude");
    }

    fn icons(icons: IconStyle) -> ViewOptions {
        ViewOptions {
            icons,
            ..Default::default()
        }
    }

    #[test]
    fn icon_per_state() {
        let cases = [
            (AgentState::Idle, "◌", "I", "💤"),
            (AgentState::Working, "⟳", "W", "⏳"),
            (AgentState::Blocked, "!", "B", "❗"),
            (AgentState::Done, "✓", "D", "✅"),
            (AgentState::Error, "✗", "E", "❌"),
        ];
        let alphabet = icons(IconStyle::Alphabet);
        let emoji = icons(IconStyle::Emoji);
        for (state, symbol, letter, emoji_icon) in cases {
            let a = [agent(1, state)];
            assert_eq!(plain(&a, 80), format!(" {symbol} claude"));
            assert_eq!(plain_with(&a, 80, &alphabet), format!(" {letter} claude"));
            assert_eq!(plain_with(&a, 80, &emoji), format!(" {emoji_icon} claude"));
        }
    }

    #[test]
    fn emoji_icons_are_counted_as_two_columns() {
        let emoji = icons(IconStyle::Emoji);
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
        let emoji = icons(IconStyle::Emoji);
        let agents: Vec<_> = (0..5).map(|i| agent(i, AgentState::Idle)).collect();
        assert_eq!(plain_with(&agents, 8, &emoji), " 💤  💤~");
        // 残り 1 桁には幅 2 のアイコンが入らないので、その手前で切る
        assert_eq!(plain_with(&agents, 7, &emoji), " 💤  ~");
    }

    #[test]
    fn ellipsis_is_one_column_wide() {
        assert_eq!(ELLIPSIS.width_cjk(), Some(1));
    }

    #[test]
    fn name_falls_back_from_title_to_tab_name_to_app_to_pane_id() {
        let mut a = agent(7, AgentState::Idle);
        a.title = Some("my-project".to_owned());
        a.tab_name = Some("work".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " ◌ my-project");
        a.title = Some("  ".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " ◌ work");
        a.tab_name = None;
        assert_eq!(plain(&[a.clone()], 80), " ◌ claude");
        a.app = None;
        assert_eq!(plain(&[a], 80), " ◌ #7");
    }

    #[test]
    fn tab_name_is_cleaned_and_skipped_when_empty() {
        let mut a = agent(7, AgentState::Idle);
        a.tab_name = Some(" \t\n".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " ◌ claude");
        a.tab_name = Some("my\ntab".to_owned());
        assert_eq!(plain(&[a], 80), " ◌ my tab");
    }

    #[test]
    fn hidden_app_is_skipped_as_name() {
        let hide_app = ViewOptions {
            show_app: false,
            ..Default::default()
        };
        let mut a = with_msg(agent(7, AgentState::Idle), "waiting");
        assert_eq!(plain_with(&[a.clone()], 80, &hide_app), " ◌ #7: waiting");
        a.tab_name = Some("work".to_owned());
        assert_eq!(plain_with(&[a.clone()], 80, &hide_app), " ◌ work: waiting");
        a.title = Some("my-project".to_owned());
        assert_eq!(plain_with(&[a], 80, &hide_app), " ◌ my-project: waiting");
    }

    #[test]
    fn tab_name_does_not_affect_order_or_filter() {
        let mut a = agent(5, AgentState::Idle);
        a.tab_name = Some("z".to_owned());
        let mut b = agent(1, AgentState::Idle);
        b.tab = 1;
        b.tab_name = Some("a".to_owned());
        let mut c = agent(9, AgentState::Idle);
        c.tab_name = Some("z".to_owned());
        c.app = Some("codex".to_owned());
        let agents = [b, c, a];

        // 同じタブに複数いると同じ名前が並ぶ（既知の制限）
        assert_eq!(plain(&agents, 80), " ◌ z  ◌ z  ◌ a");
        let only_claude = ViewOptions {
            apps: Some(vec!["claude".to_owned()]),
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &only_claude), " ◌ z  ◌ a");
    }

    #[test]
    fn hidden_app_does_not_affect_filter() {
        let mut codex = agent(2, AgentState::Idle);
        codex.app = Some("codex".to_owned());
        let mut no_app = agent(3, AgentState::Idle);
        no_app.app = None;
        let agents = [agent(1, AgentState::Idle), codex, no_app];

        let only_claude = ViewOptions {
            apps: Some(vec!["claude".to_owned()]),
            show_app: false,
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &only_claude), " ◌ #1");
        let hide_app = ViewOptions {
            show_app: false,
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &hide_app), " ◌ #1  ◌ #2  ◌ #3");
    }

    #[test]
    fn control_characters_are_replaced() {
        let a = with_msg(agent(1, AgentState::Done), "line1\nline2\x1b[31m");
        assert_eq!(plain(&[a], 80), " ✓ claude: line1 line2 [31m");
    }

    #[test]
    fn sorted_by_tab_then_pane() {
        let mut a = agent(5, AgentState::Idle);
        a.app = Some("a".to_owned());
        let mut b = agent(1, AgentState::Idle);
        b.app = Some("b".to_owned());
        b.tab = 1;
        let mut c = agent(9, AgentState::Idle);
        c.app = Some("c".to_owned());
        assert_eq!(plain(&[b, c, a], 80), " ◌ a  ◌ c  ◌ b");
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
        assert_eq!(plain(&agents, 80), " ◌ a  ! b  ⟳ c  ! d  ✗ e");
    }

    #[test]
    fn blocked_first_moves_blocked_to_front_keeping_order() {
        use AgentState::*;
        let blocked_first = ViewOptions {
            blocked_first: true,
            ..Default::default()
        };
        let agents = named_agents([Idle, Blocked, Working, Blocked, Error]);
        assert_eq!(
            plain_with(&agents, 80, &blocked_first),
            " ! b  ! d  ◌ a  ⟳ c  ✗ e"
        );
        // Blocked が解消されると元の位置に戻る
        let agents = named_agents([Idle, Working, Working, Blocked, Error]);
        assert_eq!(
            plain_with(&agents, 80, &blocked_first),
            " ! d  ◌ a  ⟳ b  ⟳ c  ✗ e"
        );
        let agents = named_agents([Idle, Working, Working, Done, Error]);
        assert_eq!(
            plain_with(&agents, 80, &blocked_first),
            " ◌ a  ⟳ b  ⟳ c  ✓ d  ✗ e"
        );
    }

    #[test]
    fn blocked_first_keeps_blocked_visible_when_line_is_cut() {
        use AgentState::*;
        let blocked_first = ViewOptions {
            blocked_first: true,
            ..Default::default()
        };
        let agents = named_agents([Idle, Idle, Idle, Idle, Blocked]);
        assert_eq!(plain(&agents, 8), " ◌  ◌  ~");
        assert_eq!(plain_with(&agents, 8, &blocked_first), " !  ◌  ~");
    }

    #[test]
    fn filtered_by_apps() {
        let mut codex = agent(2, AgentState::Idle);
        codex.app = Some("codex".to_owned());
        let mut no_app = agent(3, AgentState::Idle);
        no_app.app = None;
        let agents = [agent(1, AgentState::Idle), codex, no_app];

        let only_claude = ViewOptions {
            apps: Some(vec!["claude".to_owned()]),
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &only_claude), " ◌ claude");
        // 絞り込まなければ、app のないエージェントも表示する
        assert_eq!(plain(&agents, 80), " ◌ claude  ◌ codex  ◌ #3");
    }

    #[test]
    fn msgs_are_shortened_evenly() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        // 全体：" ⟳ claude: " (11) + 20 + "  " (2) + "! claude: " (10) + 5 = 48
        assert_eq!(
            plain(&[a.clone(), b.clone()], 48),
            " ⟳ claude: abcdefghijklmnopqrst  ! claude: short"
        );
        // 短い msg はそのまま、長い msg が余りを使う
        assert_eq!(
            plain(&[a, b], 40),
            " ⟳ claude: abcdefghijk~  ! claude: short"
        );
    }

    #[test]
    fn msgs_are_dropped_when_too_narrow() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        // msg に回せる幅が MIN_MSG_WIDTH 未満になる
        assert_eq!(plain(&[a, b], 32), " ⟳ claude  ! claude");
    }

    #[test]
    fn names_are_shortened_then_icons_only() {
        let mut a = agent(1, AgentState::Working);
        a.title = Some("long-project-name".to_owned());
        let b = agent(2, AgentState::Done);
        // 名前に回せる幅は 13：短い "claude" に 6、残り 7 を長い方へ
        assert_eq!(plain(&[a.clone(), b.clone()], 20), " ⟳ long-p~  ✓ claude");
        assert_eq!(plain(&[a, b], 12), " ⟳  ✓");
    }

    #[test]
    fn truncates_line_end_when_icons_do_not_fit() {
        let agents: Vec<_> = (0..5).map(|i| agent(i, AgentState::Idle)).collect();
        assert_eq!(plain(&agents, 8), " ◌  ◌  ~");
    }

    #[test]
    fn wide_characters_are_counted_by_width() {
        let a = [with_msg(agent(1, AgentState::Blocked), "許可してください")];
        // " ! claude: " (11) + 16 = 27
        assert_eq!(plain(&a, 27), " ! claude: 許可してください");
        // msg の幅 8：全角 3 文字（6）+ "~"、残り 1 桁には全角が入らない
        assert_eq!(plain(&a, 19), " ! claude: 許可し~");
    }

    #[test]
    fn never_exceeds_width() {
        let mut agents = vec![
            with_msg(agent(1, AgentState::Working), "テストを実行しています"),
            with_msg(agent(2, AgentState::Blocked), "Allow Bash(cargo test)?"),
            agent(3, AgentState::Done),
        ];
        agents[0].progress = Some(100);
        agents[2].title = Some("全角のタイトル".to_owned());
        for style in [IconStyle::Symbol, IconStyle::Alphabet, IconStyle::Emoji] {
            for cols in 0..100 {
                let text = plain_with(&agents, cols, &icons(style));
                assert!(text.width() <= cols, "{style:?} cols={cols}: {text:?}");
            }
        }
    }

    #[test]
    fn fair_share_gives_leftover_to_longer_items() {
        assert_eq!(fair_share(&[3, 20, 20], 20), vec![3, 8, 9]);
        assert_eq!(fair_share(&[3, 4], 100), vec![3, 4]);
        assert_eq!(fair_share(&[], 10), Vec::<usize>::new());
    }
}
