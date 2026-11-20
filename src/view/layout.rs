//! レイアウト：どのエージェントを、どの順で、どの文字列をどの役割で並べるかを決める。
//!
//! 1 エージェント分は `アイコン 進捗 名前 (cwd): msg` の形（進捗・cwd・msg はあるときだけ）。
//! cwd はエージェントを見分けるためのものなので、縮めるときは msg より後まで残す。
//!
//! [`ViewOptions::max_length`] を指定しているときは、まず 1 エージェント分（アイコン・進捗を含む）を
//! その幅に収める。msg を切り詰め、[`MIN_MSG_WIDTH`] を下回るなら msg を省いて cwd を切り詰め、
//! [`MIN_CWD_WIDTH`] を下回るなら cwd も省いて名前を切り詰める。
//!
//! そのうえで行の幅が足りないときは、次の順に縮める。
//! 1. msg を各エージェントで均等に切り詰める
//! 2. msg を省き、cwd を均等に切り詰める
//! 3. cwd を省き、名前を均等に切り詰める
//! 4. アイコン（と進捗）だけにする
//! 5. それでも収まらなければ、行の末尾を切る

use std::path::Path;

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
/// 切り詰めた msg / cwd / 名前がこの幅を下回るなら、切り詰めずに次の段階へ進む
const MIN_MSG_WIDTH: usize = 6;
const MIN_CWD_WIDTH: usize = 4;
const MIN_NAME_WIDTH: usize = 4;
/// 名前と msg の間に入る `: ` の幅
const MSG_OVERHEAD: usize = 2;
/// 名前の後ろに付ける cwd を囲む ` (` と `)` の幅
const CWD_OVERHEAD: usize = 3;
/// [`ViewOptions::max_length`] の下限。
/// アイコン（最大 2）と進捗（最大 ` 255%` の 5）に、名前の前の空白と [`MIN_NAME_WIDTH`] を足した幅。
///
/// これより小さい上限を許すと、次の問題が起きるため受け付けない（config で検証する）。
/// - アイコンと進捗だけで上限を超えることがある（例：絵文字アイコン + ` 100%` で 7）
/// - 上限に収めるために、名前が [`MIN_NAME_WIDTH`] より短く切り詰められて見分けられなくなる
pub const MIN_MAX_LENGTH: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// 余白や区切り
    Plain,
    /// アイコンと進捗。状態の色で塗る。
    State(AgentState),
    Name,
    Cwd,
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
    /// カレントディレクトリの末尾のディレクトリ名（`show_cwd` が false なら None）
    cwd: Option<String>,
    msg: Option<String>,
}

/// 各部分を表示する幅の上限。None なら表示しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Widths {
    name: Option<usize>,
    cwd: Option<usize>,
    msg: Option<usize>,
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
        let cwd = agent
            .cwd
            .as_deref()
            .filter(|_| options.show_cwd)
            .and_then(|cwd| clean(Some(&dir_name(cwd))));
        Item {
            state: agent.state,
            icon: options.icons.icon(agent.state),
            progress: agent.progress.map(|p| format!("{p}%")),
            name,
            cwd,
            msg: clean(agent.msg.as_deref()),
        }
    }

    /// アイコンと進捗の幅（常に表示する部分）
    fn head_width(&self) -> usize {
        self.icon.width() + self.progress.as_ref().map_or(0, |p| 1 + p.width())
    }

    /// 各部分の幅の上限（名前は常に Some）。`max_length` を指定していれば、1 エージェント分がその幅に収まるようにする。
    /// msg を切り詰め、[`MIN_MSG_WIDTH`] を下回るなら msg を省いて cwd を切り詰め、
    /// [`MIN_CWD_WIDTH`] を下回るなら cwd も省いて名前を切り詰める。
    fn widths(&self, max_length: Option<usize>) -> Widths {
        let name = self.name.width();
        let cwd = self.cwd.as_ref().map(|c| c.width());
        let msg = self.msg.as_ref().map(|m| m.width());
        let Some(max_length) = max_length else {
            return Widths {
                name: Some(name),
                cwd,
                msg,
            };
        };
        // 名前（と cwd・msg）に使える幅。名前の前の空白を除く
        let room = max_length.saturating_sub(self.head_width() + 1);
        let cwd_total = cwd.map_or(0, |c| c + CWD_OVERHEAD);
        let fits = |width: usize, room: usize, min: usize| width <= room || room >= min;

        if let Some(msg) = msg
            && let Some(msg_room) = room.checked_sub(name + cwd_total + MSG_OVERHEAD)
            && fits(msg, msg_room, MIN_MSG_WIDTH)
        {
            return Widths {
                name: Some(name),
                cwd,
                msg: Some(msg.min(msg_room)),
            };
        }
        if let Some(cwd) = cwd
            && let Some(cwd_room) = room.checked_sub(name + CWD_OVERHEAD)
            && fits(cwd, cwd_room, MIN_CWD_WIDTH)
        {
            return Widths {
                name: Some(name),
                cwd: Some(cwd.min(cwd_room)),
                msg: None,
            };
        }
        Widths {
            name: Some(name.min(room)),
            cwd: None,
            msg: None,
        }
    }

    /// cwd と msg は名前を表示するときだけ表示する。
    fn spans(&self, widths: Widths) -> Vec<Span> {
        let mut spans = vec![Span::new(self.icon, Role::State(self.state))];
        if let Some(progress) = &self.progress {
            spans.push(Span::new(format!(" {progress}"), Role::State(self.state)));
        }
        if let Some(width) = widths.name {
            spans.push(Span::new(" ", Role::Plain));
            spans.push(Span::new(truncate(&self.name, width), Role::Name));
            if let (Some(text), Some(width)) = (&self.cwd, widths.cwd) {
                spans.push(Span::new(" (", Role::Plain));
                spans.push(Span::new(truncate(text, width), Role::Cwd));
                spans.push(Span::new(")", Role::Plain));
            }
            if let (Some(text), Some(width)) = (&self.msg, widths.msg) {
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

/// パスの末尾のディレクトリ名。ルート（`/`）のように名前がなければ、パス全体を使う。
/// フルパスは長く、ホームを `~` に縮めるには環境変数を読む permission が要るため、末尾だけにしている。
fn dir_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
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
    let plans = plan(&items, cols, options.max_length);
    for (i, (item, widths)) in items.iter().zip(plans).enumerate() {
        if i > 0 {
            spans.push(Span::new(SEP, Role::Plain));
        }
        spans.extend(item.spans(widths));
    }
    clamp(spans, cols)
}

/// 各エージェントの各部分の幅を決める（縮める順はモジュールの説明を参照）。
/// [`Item::widths`] で `max_length` に収めた幅を、行の幅に合わせてさらに縮める。
fn plan(items: &[Item], cols: usize, max_length: Option<usize>) -> Vec<Widths> {
    let fixed = PAD.width()
        + SEP.width() * (items.len() - 1)
        + items.iter().map(Item::head_width).sum::<usize>();
    let caps: Vec<Widths> = items.iter().map(|i| i.widths(max_length)).collect();
    let names = shown(&caps, |w| w.name);
    // 名前の前の空白を含む
    let names_total = names.iter().map(|w| 1 + w).sum::<usize>();
    let cwds = shown(&caps, |w| w.cwd);
    let cwds_overhead = CWD_OVERHEAD * cwds.len();
    let cwds_total = cwds.iter().sum::<usize>() + cwds_overhead;
    let msgs = shown(&caps, |w| w.msg);
    let msgs_overhead = MSG_OVERHEAD * msgs.len();

    if let Some(room) = cols.checked_sub(fixed + names_total + cwds_total + msgs_overhead)
        && let Some(msgs) = fit(&msgs, room, MIN_MSG_WIDTH)
    {
        return assemble(&caps, Some(names), Some(cwds), Some(msgs));
    }

    if let Some(room) = cols.checked_sub(fixed + names_total + cwds_overhead)
        && let Some(cwds) = fit(&cwds, room, MIN_CWD_WIDTH)
    {
        return assemble(&caps, Some(names), Some(cwds), None);
    }

    if let Some(room) = cols.checked_sub(fixed + items.len())
        && let Some(names) = fit(&names, room, MIN_NAME_WIDTH)
    {
        return assemble(&caps, Some(names), None, None);
    }

    assemble(&caps, None, None, None)
}

/// `part` を表示するエージェントについての、その幅の一覧。
fn shown(caps: &[Widths], part: impl Fn(&Widths) -> Option<usize>) -> Vec<usize> {
    caps.iter().filter_map(part).collect()
}

/// [`plan`] の各段階の結果を、エージェントごとの [`Widths`] にする。
/// `names` / `cwds` / `msgs` は、`caps`（[`Item::widths`] の結果）でその部分を表示するエージェントの分だけを並べたもの。
/// None はその部分を表示しない。
fn assemble(
    caps: &[Widths],
    names: Option<Vec<usize>>,
    cwds: Option<Vec<usize>>,
    msgs: Option<Vec<usize>>,
) -> Vec<Widths> {
    let mut names = names.map(Vec::into_iter);
    let mut cwds = cwds.map(Vec::into_iter);
    let mut msgs = msgs.map(Vec::into_iter);
    let next = |cap: Option<usize>, shares: &mut Option<std::vec::IntoIter<usize>>| {
        cap.and_then(|_| shares.as_mut().and_then(Iterator::next))
    };
    caps.iter()
        .map(|cap| Widths {
            name: next(cap.name, &mut names),
            cwd: next(cap.cwd, &mut cwds),
            msg: next(cap.msg, &mut msgs),
        })
        .collect()
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
    use crate::view::tests::{agent, with_cwd, with_msg};

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

    /// 行の幅による縮め方を見るため、`max_length` の上限はなくしておく
    fn show_cwd() -> ViewOptions {
        ViewOptions {
            show_cwd: true,
            max_length: None,
            ..Default::default()
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
            " ⟳ claude (issue-21): Running tests"
        );
        // 既定では表示しない
        assert_eq!(plain(&[a], 80), " ⟳ claude: Running tests");
    }

    #[test]
    fn cwd_without_last_directory_or_unknown() {
        let options = show_cwd();
        let root = with_cwd(agent(1, AgentState::Idle), "/");
        assert_eq!(plain_with(&[root], 80, &options), " ◌ claude (/)");
        let trailing = with_cwd(agent(1, AgentState::Idle), "/repo/main/");
        assert_eq!(plain_with(&[trailing], 80, &options), " ◌ claude (main)");
        let control = with_cwd(agent(1, AgentState::Idle), "/repo/a\nb");
        assert_eq!(plain_with(&[control], 80, &options), " ◌ claude (a b)");
        // 取得できていなければ、括弧ごと表示しない
        let unknown = agent(1, AgentState::Idle);
        assert_eq!(plain_with(&[unknown], 80, &options), " ◌ claude");
    }

    #[test]
    fn cwd_and_msg_are_placed_per_agent() {
        let a = with_msg(agent(1, AgentState::Working), "Running tests");
        let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
        assert_eq!(
            plain_with(&[a, b], 80, &show_cwd()),
            " ⟳ claude: Running tests  ✓ claude (main)"
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
            line(58),
            " ⟳ claude (feature-branch): Running tests  ✓ claude (main)"
        );
        // 1. msg を切り詰める（MIN_MSG_WIDTH まで）
        assert_eq!(
            line(51),
            " ⟳ claude (feature-branch): Runni~  ✓ claude (main)"
        );
        // 2. msg を省き、cwd を均等に切り詰める
        assert_eq!(line(50), " ⟳ claude (feature-branch)  ✓ claude (main)");
        assert_eq!(line(40), " ⟳ claude (feature-br~)  ✓ claude (main)");
        assert_eq!(line(33), " ⟳ claude (fea~)  ✓ claude (main)");
        // 3. cwd を省き、名前を切り詰める
        assert_eq!(line(32), " ⟳ claude  ✓ claude");
        // 4. アイコンだけにする
        assert_eq!(line(12), " ⟳  ✓");
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
        agents[0].cwd = Some("/home/user/全角のディレクトリ".into());
        agents[2].title = Some("全角のタイトル".to_owned());
        agents[2].cwd = Some("/repo/main".into());
        for style in [IconStyle::Symbol, IconStyle::Alphabet, IconStyle::Emoji] {
            for show_cwd in [false, true] {
                let options = ViewOptions {
                    show_cwd,
                    ..icons(style)
                };
                for cols in 0..120 {
                    let text = plain_with(&agents, cols, &options);
                    assert!(
                        text.width() <= cols,
                        "{style:?} show_cwd={show_cwd} cols={cols}: {text:?}"
                    );
                }
            }
        }
    }

    fn max_length(max_length: usize) -> ViewOptions {
        ViewOptions {
            max_length: Some(max_length),
            ..Default::default()
        }
    }

    /// 各エージェント分の幅（行頭の余白と区切りを除く）
    fn agent_widths(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> Vec<usize> {
        let mut widths = vec![0];
        for span in layout(agents, cols, options).into_iter().skip(1) {
            if span.role == Role::Plain && span.text == SEP {
                widths.push(0);
            } else {
                *widths.last_mut().unwrap() += span.text.width();
            }
        }
        widths
    }

    #[test]
    fn max_length_truncates_msg_first() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        // "⟳ claude: " (10) + msg 10 = 20。短い方はそのまま
        assert_eq!(
            plain_with(&[a, b], 80, &max_length(20)),
            " ⟳ claude: abcdefghi~  ! claude: short"
        );
    }

    #[test]
    fn max_length_drops_msg_then_truncates_name() {
        let mut a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        // msg に回せる幅が 4（MIN_MSG_WIDTH 未満）なので省く
        assert_eq!(plain_with(&[a.clone()], 80, &max_length(14)), " ⟳ claude");
        // 収まる msg なら MIN_MSG_WIDTH 未満の幅でも表示する
        let short = with_msg(agent(1, AgentState::Working), "ok");
        assert_eq!(plain_with(&[short], 80, &max_length(14)), " ⟳ claude: ok");

        a.title = Some("long-project-name".to_owned());
        assert_eq!(plain_with(&[a], 80, &max_length(14)), " ⟳ long-projec~");
    }

    #[test]
    fn max_length_counts_icon_and_progress() {
        let mut a = with_msg(agent(1, AgentState::Working), "Running");
        a.progress = Some(100);
        assert_eq!(
            plain_with(&[a.clone()], 80, &max_length(MIN_MAX_LENGTH)),
            " ⟳ 100% clau~"
        );
        let emoji = ViewOptions {
            icons: IconStyle::Emoji,
            ..max_length(MIN_MAX_LENGTH)
        };
        assert_eq!(plain_with(&[a], 80, &emoji), " ⏳ 100% cla~");
    }

    #[test]
    fn max_length_is_combined_with_row_width() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        let options = max_length(20);
        // 上限で切り詰めた幅（msg 10 と 5）を、行の幅に合わせてさらに均等に縮める
        assert_eq!(
            plain_with(&[a.clone(), b.clone()], 36, &options),
            " ⟳ claude: abcdefg~  ! claude: short"
        );
        assert_eq!(plain_with(&[a, b], 20, &options), " ⟳ claude  ! claude");
    }

    #[test]
    fn max_length_defaults_to_50_and_none_is_unlimited() {
        let msg = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let a = with_msg(agent(1, AgentState::Working), msg);
        // "⟳ claude: " (10) + msg 40 = 50
        assert_eq!(
            plain(std::slice::from_ref(&a), 80),
            " ⟳ claude: abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLM~"
        );
        let unlimited = ViewOptions {
            max_length: None,
            ..Default::default()
        };
        assert_eq!(
            plain_with(&[a], 80, &unlimited),
            format!(" ⟳ claude: {msg}")
        );
    }

    #[test]
    fn max_length_leaves_short_agents_unchanged() {
        let a = with_msg(agent(1, AgentState::Done), "ok");
        assert_eq!(plain_with(&[a], 80, &max_length(40)), " ✓ claude: ok");
    }

    #[test]
    fn min_max_length_fits_widest_head_and_min_name() {
        let mut a = agent(1, AgentState::Working);
        a.progress = Some(u8::MAX);
        let item = Item::new(&a, &icons(IconStyle::Emoji));
        assert!(item.head_width() + 1 + MIN_NAME_WIDTH <= MIN_MAX_LENGTH);
    }

    fn max_length_with_cwd(max: usize) -> ViewOptions {
        ViewOptions {
            show_cwd: true,
            ..max_length(max)
        }
    }

    #[test]
    fn max_length_drops_msg_then_truncates_cwd_then_drops_cwd() {
        let a = with_msg(
            with_cwd(agent(1, AgentState::Working), "/repo/feature-branch"),
            "Running tests",
        );
        let line = |max| plain_with(std::slice::from_ref(&a), 80, &max_length_with_cwd(max));

        assert_eq!(line(40), " ⟳ claude (feature-branch): Running tests");
        // msg を切り詰める（MIN_MSG_WIDTH まで）
        assert_eq!(line(35), " ⟳ claude (feature-branch): Running~");
        // msg を省き、cwd を切り詰める（MIN_CWD_WIDTH まで）
        assert_eq!(line(30), " ⟳ claude (feature-branch)");
        assert_eq!(line(20), " ⟳ claude (feature-~)");
        assert_eq!(line(15), " ⟳ claude (fea~)");
        // cwd も省き、名前を切り詰める
        assert_eq!(line(14), " ⟳ claude");
        let mut long = a.clone();
        long.title = Some("long-project-name".to_owned());
        assert_eq!(
            plain_with(&[long], 80, &max_length_with_cwd(14)),
            " ⟳ long-projec~"
        );
    }

    #[test]
    fn max_length_with_cwd_is_combined_with_row_width() {
        let a = with_cwd(agent(1, AgentState::Working), "/repo/feature-branch");
        let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
        let options = max_length_with_cwd(20);
        // 上限で切り詰めた cwd（9 と 4）を、行の幅に合わせてさらに均等に縮める
        assert_eq!(
            plain_with(&[a.clone(), b.clone()], 80, &options),
            " ⟳ claude (feature-~)  ✓ claude (main)"
        );
        assert_eq!(
            plain_with(&[a, b], 36, &options),
            " ⟳ claude (featur~)  ✓ claude (main)"
        );
    }

    #[test]
    fn never_exceeds_max_length() {
        let mut agents = vec![
            with_msg(agent(1, AgentState::Working), "テストを実行しています"),
            with_msg(agent(2, AgentState::Blocked), "Allow Bash(cargo test)?"),
            agent(3, AgentState::Done),
        ];
        agents[0].progress = Some(100);
        agents[0].cwd = Some("/home/user/全角のディレクトリ".into());
        agents[2].title = Some("全角のタイトル".to_owned());
        agents[2].cwd = Some("/repo/main".into());
        for style in [IconStyle::Symbol, IconStyle::Alphabet, IconStyle::Emoji] {
            for show_cwd in [false, true] {
                for max in MIN_MAX_LENGTH..40 {
                    let options = ViewOptions {
                        icons: style,
                        show_cwd,
                        ..max_length(max)
                    };
                    for cols in 0..100 {
                        let text = plain_with(&agents, cols, &options);
                        let case = format!("{style:?} show_cwd={show_cwd} max={max} cols={cols}");
                        assert!(text.width() <= cols, "{case}");
                        for width in agent_widths(&agents, cols, &options) {
                            assert!(width <= max, "{case}: {text:?}");
                        }
                    }
                }
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
