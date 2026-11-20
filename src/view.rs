//! 表示：エージェントの一覧と幅から、1 行分の文字列（ANSI エスケープ付き）を作る。
//!
//! 副作用のない関数だけで構成し、`render` からは [`render_line`] の結果を出力するだけにする。
//! 内部は「どの文字列をどの役割で並べるか（[`layout`]）」と「色付け（[`paint`]）」に分けてある。
//!
//! 幅が足りないときは、次の順に縮める。
//! 1. msg を各エージェントで均等に切り詰める
//! 2. msg を省き、名前を均等に切り詰める
//! 3. アイコン（と進捗）だけにする
//! 4. それでも収まらなければ、行の末尾を切る

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zellij_tile::prelude::{PaletteColor, Styling};

use crate::model::{AgentState, AgentStatus};

/// 行頭の余白
const PAD: &str = " ";
/// エージェント同士の区切り
const SEP: &str = "  ";
/// 切り詰めたことを示す記号。
/// `…`（U+2026）は East Asian Width が Ambiguous で、端末によっては 2 桁になるため使わない。
const ELLIPSIS: char = '~';
/// 切り詰めた msg / 名前がこの幅を下回るなら、切り詰めずに次の段階へ進む
const MIN_MSG_WIDTH: usize = 6;
const MIN_NAME_WIDTH: usize = 4;

/// アイコンの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconStyle {
    /// 記号。East Asian Width が Ambiguous の文字（`●` など）は、端末によって幅がずれるので使わない。
    #[default]
    Symbol,
    /// アルファベット 1 文字。記号のフォントがない環境向け。
    Alphabet,
}

impl IconStyle {
    fn icon(self, state: AgentState) -> &'static str {
        match (self, state) {
            (IconStyle::Symbol, AgentState::Idle) => "◌",
            (IconStyle::Symbol, AgentState::Working) => "⟳",
            (IconStyle::Symbol, AgentState::Blocked) => "!",
            (IconStyle::Symbol, AgentState::Done) => "✓",
            (IconStyle::Symbol, AgentState::Error) => "✗",
            (IconStyle::Alphabet, AgentState::Idle) => "I",
            (IconStyle::Alphabet, AgentState::Working) => "W",
            (IconStyle::Alphabet, AgentState::Blocked) => "B",
            (IconStyle::Alphabet, AgentState::Done) => "D",
            (IconStyle::Alphabet, AgentState::Error) => "E",
        }
    }
}

/// 表示の設定。`load` の configuration から作る（`src/config.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewOptions {
    pub icons: IconStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
    /// 256 色のインデックス
    Indexed(u8),
}

impl From<PaletteColor> for Color {
    fn from(color: PaletteColor) -> Self {
        match color {
            PaletteColor::Rgb((r, g, b)) => Color::Rgb(r, g, b),
            PaletteColor::EightBit(n) => Color::Indexed(n),
        }
    }
}

impl Color {
    /// SGR のパラメータ。`base` は前景色なら 38、背景色なら 48。
    fn sgr(self, base: u8) -> String {
        match self {
            Color::Rgb(r, g, b) => format!("{base};2;{r};{g};{b}"),
            Color::Indexed(n) => format!("{base};5;{n}"),
        }
    }
}

/// 表示に使う色。`ModeUpdate` で届く `ModeInfo.style.colors` から作る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub idle: Color,
    pub working: Color,
    pub blocked: Color,
    pub done: Color,
    pub error: Color,
}

impl Theme {
    /// 色の対応は zellij-utils の `From<Palette> for Styling` を参照
    /// （`text_unselected` の emphasis_0〜2 がそれぞれ orange / cyan / green、`exit_code_error.base` が red）。
    pub fn from_styling(styling: &Styling) -> Self {
        let text = &styling.text_unselected;
        Theme {
            fg: text.base.into(),
            bg: text.background.into(),
            idle: text.base.into(),
            working: text.emphasis_1.into(),
            blocked: text.emphasis_0.into(),
            done: text.emphasis_2.into(),
            error: styling.exit_code_error.base.into(),
        }
    }

    fn state(&self, state: AgentState) -> Color {
        match state {
            AgentState::Idle => self.idle,
            AgentState::Working => self.working,
            AgentState::Blocked => self.blocked,
            AgentState::Done => self.done,
            AgentState::Error => self.error,
        }
    }
}

impl Default for Theme {
    /// `ModeUpdate` が届くまでに使う色
    fn default() -> Self {
        Theme::from_styling(&Styling::default())
    }
}

/// 幅 `cols` の 1 行を作る。末尾は背景色の空白で埋める。
pub fn render_line(
    agents: &[AgentStatus],
    cols: usize,
    theme: &Theme,
    options: &ViewOptions,
) -> String {
    paint(&layout(agents, cols, options), cols, theme)
}

/// エージェント一覧の代わりに出す 1 行のメッセージ（permission の案内や設定の警告）を作る。
/// 幅を超える分は切り詰め、末尾は背景色の空白で埋める。
pub fn render_message(text: &str, cols: usize, theme: &Theme) -> String {
    let spans = match clean(Some(text)) {
        Some(text) => clamp(
            vec![Span::new(PAD, Role::Plain), Span::new(text, Role::Msg)],
            cols,
        ),
        None => vec![],
    };
    paint(&spans, cols, theme)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// 余白や区切り
    Plain,
    /// アイコンと進捗。状態の色で塗る。
    State(AgentState),
    Name,
    Msg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    text: String,
    role: Role,
}

impl Span {
    fn new(text: impl Into<String>, role: Role) -> Self {
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
    fn new(agent: &AgentStatus, icons: IconStyle) -> Self {
        let name = clean(agent.title.as_deref())
            .or_else(|| clean(agent.app.as_deref()))
            .unwrap_or_else(|| format!("#{}", agent.pane_id));
        Item {
            state: agent.state,
            icon: icons.icon(agent.state),
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
fn clean(text: Option<&str>) -> Option<String> {
    let text: String = text?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// 並べる順は (タブ, ペイン) の順で固定する。
/// 状態の優先度で並べ替えると、状態が変わるたびに位置が入れ替わって追いにくいため。
fn layout(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> Vec<Span> {
    let mut agents: Vec<&AgentStatus> = agents.iter().collect();
    agents.sort_by_key(|a| (a.tab, a.pane_id));
    let items: Vec<Item> = agents
        .into_iter()
        .map(|a| Item::new(a, options.icons))
        .collect();
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
fn clamp(spans: Vec<Span>, cols: usize) -> Vec<Span> {
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

/// Span 列に色を付け、幅 `cols` まで背景色の空白で埋める。
fn paint(spans: &[Span], cols: usize, theme: &Theme) -> String {
    let bg = theme.bg.sgr(48);
    let mut out = String::new();
    let mut used = 0;
    for span in spans {
        let (fg, bold) = match span.role {
            Role::Plain | Role::Msg => (theme.fg, false),
            Role::Name => (theme.fg, true),
            Role::State(state) => (theme.state(state), true),
        };
        let bold = if bold { "1;" } else { "" };
        out.push_str(&format!("\x1b[0;{bold}{};{bg}m{}", fg.sgr(38), span.text));
        used += span.text.width();
    }
    out.push_str(&format!(
        "\x1b[0;{bg}m{}\x1b[0m",
        " ".repeat(cols.saturating_sub(used))
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
        AgentStatus {
            pane_id,
            tab: 0,
            state,
            kind: None,
            progress: None,
            app: Some("claude".to_owned()),
            title: None,
            msg: None,
            updated_ms: 0,
        }
    }

    fn with_msg(mut a: AgentStatus, msg: &str) -> AgentStatus {
        a.msg = Some(msg.to_owned());
        a
    }

    fn plain(agents: &[AgentStatus], cols: usize) -> String {
        plain_with(agents, cols, &ViewOptions::default())
    }

    fn plain_with(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
        layout(agents, cols, options)
            .into_iter()
            .map(|s| s.text)
            .collect()
    }

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

    #[test]
    fn icon_per_state() {
        let cases = [
            (AgentState::Idle, "◌", "I"),
            (AgentState::Working, "⟳", "W"),
            (AgentState::Blocked, "!", "B"),
            (AgentState::Done, "✓", "D"),
            (AgentState::Error, "✗", "E"),
        ];
        let alphabet = ViewOptions {
            icons: IconStyle::Alphabet,
        };
        for (state, symbol, letter) in cases {
            let a = [agent(1, state)];
            assert_eq!(plain(&a, 80), format!(" {symbol} claude"));
            assert_eq!(plain_with(&a, 80, &alphabet), format!(" {letter} claude"));
        }
    }

    #[test]
    fn symbols_are_one_column_wide() {
        for state in [
            AgentState::Idle,
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Done,
            AgentState::Error,
        ] {
            assert_eq!(IconStyle::Symbol.icon(state).width(), 1, "{state:?}");
            // 端末によって幅が変わらないこと（Ambiguous でないこと）
            assert_eq!(IconStyle::Symbol.icon(state).width_cjk(), 1, "{state:?}");
        }
        assert_eq!(ELLIPSIS.width_cjk(), Some(1));
    }

    #[test]
    fn name_falls_back_from_title_to_app_to_pane_id() {
        let mut a = agent(7, AgentState::Idle);
        a.title = Some("my-project".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " ◌ my-project");
        a.title = Some("  ".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " ◌ claude");
        a.app = None;
        assert_eq!(plain(&[a], 80), " ◌ #7");
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
        for cols in 0..100 {
            let text = plain(&agents, cols);
            assert!(text.width() <= cols, "cols={cols}: {text:?}");
        }
    }

    #[test]
    fn fair_share_gives_leftover_to_longer_items() {
        assert_eq!(fair_share(&[3, 20, 20], 20), vec![3, 8, 9]);
        assert_eq!(fair_share(&[3, 4], 100), vec![3, 4]);
        assert_eq!(fair_share(&[], 10), Vec::<usize>::new());
    }

    #[test]
    fn paint_colors_spans_and_fills_width() {
        let theme = Theme {
            fg: Color::Indexed(7),
            bg: Color::Rgb(1, 2, 3),
            idle: Color::Indexed(8),
            working: Color::Indexed(6),
            blocked: Color::Indexed(3),
            done: Color::Indexed(2),
            error: Color::Indexed(1),
        };
        let a = [agent(1, AgentState::Error)];
        let line = render_line(&a, 20, &theme, &ViewOptions::default());
        assert!(line.contains("\x1b[0;1;38;5;1;48;2;1;2;3m✗"), "{line:?}");
        assert!(
            line.contains("\x1b[0;1;38;5;7;48;2;1;2;3mclaude"),
            "{line:?}"
        );
        assert!(line.ends_with("\x1b[0m"));
        let visible = strip_ansi(&line);
        assert_eq!(visible.width(), 20);
        assert_eq!(visible.trim_end(), " ✗ claude");
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

    #[test]
    fn theme_follows_styling() {
        let mut styling = Styling::default();
        styling.text_unselected.base = PaletteColor::EightBit(1);
        styling.text_unselected.background = PaletteColor::EightBit(2);
        styling.text_unselected.emphasis_0 = PaletteColor::EightBit(3);
        styling.text_unselected.emphasis_1 = PaletteColor::EightBit(4);
        styling.text_unselected.emphasis_2 = PaletteColor::EightBit(5);
        styling.exit_code_error.base = PaletteColor::Rgb((6, 7, 8));
        let theme = Theme::from_styling(&styling);
        assert_eq!(theme.fg, Color::Indexed(1));
        assert_eq!(theme.bg, Color::Indexed(2));
        assert_eq!(theme.idle, Color::Indexed(1));
        assert_eq!(theme.blocked, Color::Indexed(3));
        assert_eq!(theme.working, Color::Indexed(4));
        assert_eq!(theme.done, Color::Indexed(5));
        assert_eq!(theme.error, Color::Rgb(6, 7, 8));
    }
}
