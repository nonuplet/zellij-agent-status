//! レイアウト：どのエージェントを、どの順で、どの文字列をどの役割で並べるかを決める。
//!
//! 1 エージェント分は `アイコン 進捗 名前 (cwd): msg` の形（進捗・cwd・msg はあるときだけ）。
//! Nerd Font モードでは、名前の後ろに `app` のアイコンを付けて `名前 <app> (cwd)` の形にする（対応するアイコンがあるときだけ）。
//! `app` のアイコンは名前と一体で扱い、名前を省くときは一緒に省く。
//! [`ViewOptions::nerd_font_icon_padding`] が true なら、Nerd Font のアイコン（状態アイコンと `app` のアイコン）の
//! 右隣に必ず空白を置く（`[<icon> ] 名前 <app> : msg`。[`app_icon`] を参照）。
//! cwd はエージェントを見分けるためのものなので、縮めるときは msg より後まで残す。
//!
//! 状態アイコンと進捗は、状態の色で背景を塗ったバッジ（`[↻ 42%] 名前`）にする。バッジの両端には
//! 端の文字（[`BadgeEnds`]）を付ける。絵文字アイコンは自分の色を持っていて、背景も塗ると色が二重になるので、
//! バッジにせず端の文字も付けない。
//! [`ViewOptions::highlight_attention`] が true なら、`Blocked` / `Error` のエージェントは 1 つ分全体を
//! バッジにする（`[! 名前: msg]`）。端の文字をエージェントの両端に移すだけなので、強調しても幅は変わらない。
//!
//! [`ViewOptions::max_length`] を指定しているときは、まず 1 エージェント分（アイコン・進捗を含む）を
//! その幅に収める。msg を切り詰め、[`MIN_MSG_WIDTH`] を下回るなら msg を省いて cwd を切り詰め、
//! [`MIN_CWD_WIDTH`] を下回るなら cwd も省いて名前を切り詰める。
//!
//! そのうえで行の幅が足りないときは、次の順に縮める。
//! 1. msg を各エージェントで均等に切り詰める
//! 2. msg を省き、cwd を均等に切り詰める
//! 3. cwd を省き、名前を均等に切り詰める
//! 4. 状態アイコン（と進捗）だけにする
//! 5. それでも収まらなければ、行の末尾を切る（末尾の [`ELLIPSIS`] は切れた部分と同じ背景で塗る）

use std::path::Path;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::ViewOptions;
use super::icon::{IconStyle, app_icon};
use crate::model::{AgentState, AgentStatus};

/// 行頭の余白
pub(super) const PAD: &str = " ";
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
/// 状態アイコン（絵文字で 2、それ以外は 1）とバッジの両端（絵文字以外で 2）、進捗（最大 ` 255%` の 5）に、
/// 名前の前の空白、[`MIN_NAME_WIDTH`]、名前の後ろの `app` のアイコンと前の空白（Nerd Font モードで 2）、
/// アイコンの後ろの空白（[`ViewOptions::nerd_font_icon_padding`] で 1）を足した幅。
/// 最も広いのは絵文字以外のアイコン（1 + 2）で、アイコンの後ろに空白を置くとき。
/// 設定ごとに下限を変えると config の読み込みが設定の順番に依存するので、最も広い場合に合わせている。
/// Nerd Font の状態アイコンの後ろの空白は、進捗がないときだけ付けるので、進捗の最大幅を超えない。
///
/// これより小さい上限を許すと、次の問題が起きるため受け付けない（config で検証する）。
/// - アイコンと進捗だけで上限を超えることがある（例：バッジ + ` 100%` で 8）
/// - 上限に収めるために、名前が [`MIN_NAME_WIDTH`] より短く切り詰められて見分けられなくなる
pub const MIN_MAX_LENGTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// 余白や区切り
    Plain,
    /// 名前の後ろの `app` のアイコン（Nerd Font モード）
    App,
    /// 状態アイコンと進捗。状態の色で塗る。
    State(AgentState),
    /// バッジの端の文字（[`BadgeEnds`]）。状態の色で塗る（太字にしない）。
    BadgeEnd(AgentState),
    Name,
    Cwd,
    Msg,
}

/// Span の背景
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Fill {
    /// 行の背景色のまま
    #[default]
    None,
    /// 状態の色で塗る（バッジ）。文字は行の背景色にして、色を反転させる。
    State(AgentState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) text: String,
    pub(super) role: Role,
    pub(super) fill: Fill,
}

impl Span {
    pub(super) fn new(text: impl Into<String>, role: Role) -> Self {
        Span {
            text: text.into(),
            role,
            fill: Fill::None,
        }
    }

    fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = fill;
        self
    }
}

/// バッジの両端に付ける文字。どちらも左右 1 桁ずつ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BadgeEnds {
    /// 空白を状態の色で塗る
    Space,
    /// Nerd Font モードの半円（`nf-ple-left_half_circle_thick` U+E0B6 / `nf-ple-right_half_circle_thick` U+E0B4）。
    /// 前景色を状態の色にして、行の背景の上に描く。
    HalfCircle,
}

const LEFT_HALF_CIRCLE: &str = "\u{e0b6}";
const RIGHT_HALF_CIRCLE: &str = "\u{e0b4}";

impl BadgeEnds {
    /// 両端の幅の合計
    const WIDTH: usize = 2;

    /// 絵文字アイコンはバッジにしないので None
    fn new(options: &ViewOptions) -> Option<Self> {
        match (options.icons, options.nerd_font) {
            (IconStyle::Emoji, _) => None,
            (_, true) => Some(BadgeEnds::HalfCircle),
            (_, false) => Some(BadgeEnds::Space),
        }
    }

    /// (左端, 右端) の Span
    fn spans(self, state: AgentState) -> (Span, Span) {
        let role = Role::BadgeEnd(state);
        match self {
            BadgeEnds::Space => {
                let space = Span::new(" ", role).with_fill(Fill::State(state));
                (space.clone(), space)
            }
            BadgeEnds::HalfCircle => (
                Span::new(LEFT_HALF_CIRCLE, role),
                Span::new(RIGHT_HALF_CIRCLE, role),
            ),
        }
    }
}

/// 1 エージェント分の表示内容（切り詰める前）
struct Item {
    state: AgentState,
    /// バッジの端の文字。絵文字アイコンなら None（バッジにしない）
    ends: Option<BadgeEnds>,
    /// エージェント 1 つ分全体をバッジにするか（[`ViewOptions::highlight_attention`]）
    highlight: bool,
    icon: &'static str,
    /// 状態アイコンが Nerd Font か
    nerd_icon: bool,
    /// Nerd Font のアイコンの右隣に空白を置くか（[`ViewOptions::nerd_font_icon_padding`]）
    icon_padding: bool,
    /// `42%` の形
    progress: Option<String>,
    name: String,
    /// 名前の後ろに付ける `app` のアイコン（Nerd Font モードで、対応するアイコンがあるときだけ）
    app_icon: Option<&'static str>,
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
    /// `app` のアイコンは名前の候補とは別なので、`show_app` に関係なく付ける。
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
            ends: BadgeEnds::new(options),
            highlight: options.highlight_attention
                && matches!(agent.state, AgentState::Blocked | AgentState::Error),
            icon: options.icons.icon(agent.state),
            nerd_icon: options.icons == IconStyle::Nerd,
            icon_padding: options.nerd_font_icon_padding,
            progress: agent.progress.map(|p| format!("{p}%")),
            name,
            app_icon: agent
                .app
                .as_deref()
                .filter(|_| options.nerd_font)
                .and_then(app_icon),
            cwd,
            msg: clean(agent.msg.as_deref()),
        }
    }

    /// アイコンと進捗、バッジの両端の幅（常に表示する部分）。
    /// 強調したときはバッジの右端がエージェントの末尾に移るが、幅は同じなのでここに含める。
    fn head_width(&self) -> usize {
        self.icon.width()
            + self.progress.as_ref().map_or(0, |p| 1 + p.width())
            + usize::from(self.icon_pad())
            + self.ends.map_or(0, |_| BadgeEnds::WIDTH)
    }

    /// 状態アイコンの後ろに空白を付けるか（Nerd Font のアイコンで、[`Item::icon_padding`] が true のとき）。
    /// 進捗があればその前の空白が右隣になるので、付けない。
    /// 強調して名前が続くときも付ける（幅を状態によらず同じにするため。名前との間は空白 2 つになる）。
    fn icon_pad(&self) -> bool {
        self.icon_padding && self.nerd_icon && self.progress.is_none()
    }

    /// 名前に付く部分（前の空白と、後ろの `app` のアイコンとその前の空白、
    /// [`Item::icon_padding`] なら後ろの空白も）の幅。名前と一緒に表示し、一緒に省く
    fn name_overhead(&self) -> usize {
        1 + self
            .app_icon
            .map_or(0, |i| 1 + i.width() + usize::from(self.icon_padding))
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
        // 名前（と cwd・msg）に使える幅。名前の前の空白と後ろの app のアイコンを除く
        let room = max_length.saturating_sub(self.head_width() + self.name_overhead());
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

    /// `app` のアイコン・cwd・msg は名前を表示するときだけ表示する。
    ///
    /// バッジは `左端 アイコン 進捗 右端`。強調するときは右端をエージェントの末尾に移し、全体を塗る。
    /// 絵文字アイコン（端の文字なし）でも、強調するときは全体を塗る。
    fn spans(&self, widths: Widths) -> Vec<Span> {
        let state = Fill::State(self.state);
        let badge = if self.ends.is_some() || self.highlight {
            state
        } else {
            Fill::None
        };
        let (left, right) = self.ends.map(|e| e.spans(self.state)).unzip();

        let mut spans: Vec<Span> = left.into_iter().collect();
        spans.push(Span::new(self.icon, Role::State(self.state)).with_fill(badge));
        if let Some(progress) = &self.progress {
            spans.push(Span::new(format!(" {progress}"), Role::State(self.state)).with_fill(badge));
        }
        if self.icon_pad() {
            spans.push(Span::new(" ", Role::State(self.state)).with_fill(badge));
        }
        // 強調しないときは、ここでバッジを閉じる
        let right = if self.highlight {
            right
        } else {
            spans.extend(right);
            None
        };
        // ここから後ろ（名前・cwd・msg）は、強調するときだけ塗る
        let rest = spans.len();
        if let Some(width) = widths.name {
            spans.push(Span::new(" ", Role::Plain));
            spans.push(Span::new(truncate(&self.name, width), Role::Name));
            if let Some(icon) = self.app_icon {
                spans.push(Span::new(" ", Role::Plain));
                spans.push(Span::new(icon, Role::App));
                if self.icon_padding {
                    spans.push(Span::new(" ", Role::Plain));
                }
            }
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
        if self.highlight {
            for span in &mut spans[rest..] {
                span.fill = state;
            }
        }
        spans.extend(right);
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

    let sep = options.separator_text();
    let mut spans = vec![Span::new(PAD, Role::Plain)];
    let plans = plan(&items, cols, sep.width(), options.max_length);
    for (i, (item, widths)) in items.iter().zip(plans).enumerate() {
        if i > 0 {
            spans.push(Span::new(sep, Role::Plain));
        }
        spans.extend(item.spans(widths));
    }
    clamp(spans, cols)
}

/// 各エージェントの各部分の幅を決める（縮める順はモジュールの説明を参照）。
/// [`Item::widths`] で `max_length` に収めた幅を、行の幅に合わせてさらに縮める。
/// `sep_width` はエージェント同士の区切りの幅。
fn plan(items: &[Item], cols: usize, sep_width: usize, max_length: Option<usize>) -> Vec<Widths> {
    let fixed = PAD.width()
        + sep_width * (items.len() - 1)
        + items.iter().map(Item::head_width).sum::<usize>();
    let caps: Vec<Widths> = items.iter().map(|i| i.widths(max_length)).collect();
    // 名前は常に表示する前提（Item::widths で常に Some）
    let names = shown(&caps, |w| w.name);
    // 名前の前の空白と後ろの app のアイコン
    let names_overhead = items.iter().map(Item::name_overhead).sum::<usize>();
    let names_total = names.iter().sum::<usize>() + names_overhead;
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

    if let Some(room) = cols.checked_sub(fixed + names_overhead)
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
/// [`ELLIPSIS`] は切れた Span と同じ背景で塗る。バッジの途中で切れたときは、バッジの色の塊が
/// [`ELLIPSIS`] で終わる（右端の文字は付けない）ので、続きがあると分かる。
pub(super) fn clamp(spans: Vec<Span>, cols: usize) -> Vec<Span> {
    if spans.iter().map(|s| s.text.width()).sum::<usize>() <= cols {
        return spans;
    }
    if cols == 0 {
        return vec![];
    }
    let mut left = cols - 1;
    let mut out = vec![];
    let mut fill = Fill::None;
    for span in spans {
        let text = take_width(&span.text, left);
        let width = text.width();
        let cut = text.len() < span.text.len();
        if !text.is_empty() {
            out.push(Span::new(text, span.role).with_fill(span.fill));
        }
        left -= width;
        if cut {
            fill = span.fill;
            break;
        }
    }
    out.push(Span::new(ELLIPSIS, Role::Plain).with_fill(fill));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Separator;
    use crate::view::icon::tests::assert_nerd_font_glyph;
    use crate::view::tests::{agent, with_cwd, with_msg};

    fn plain(agents: &[AgentStatus], cols: usize) -> String {
        plain_with(agents, cols, &ViewOptions::default())
    }

    /// 行の文字列。読みやすいように、バッジの端の文字は左端を `[`、右端を `]` にする
    /// （端の文字はどれも幅 1 なので、幅は変わらない）。実際の端の文字は [`badge_ends`] で確かめる。
    fn plain_with(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> String {
        let mut open = false;
        layout(agents, cols, options)
            .into_iter()
            .map(|s| match s.role {
                Role::BadgeEnd(_) => {
                    open = !open;
                    if open { "[" } else { "]" }.to_owned()
                }
                _ => s.text,
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
        assert_eq!(plain(&[a], 80), " [↻] claude: Running tests");
    }

    #[test]
    fn shows_progress_after_icon() {
        let mut a = agent(1, AgentState::Working);
        a.progress = Some(42);
        assert_eq!(plain(&[a], 80), " [↻ 42%] claude");
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
            (AgentState::Idle, "⋯", "I", "💤"),
            (AgentState::Working, "↻", "W", "⏳"),
            (AgentState::Blocked, "!", "B", "❗"),
            (AgentState::Done, "✓", "D", "✅"),
            (AgentState::Error, "✗", "E", "❌"),
        ];
        let alphabet = icons(IconStyle::Alphabet);
        let emoji = icons(IconStyle::Emoji);
        for (state, symbol, letter, emoji_icon) in cases {
            let a = [agent(1, state)];
            assert_eq!(plain(&a, 80), format!(" [{symbol}] claude"));
            assert_eq!(plain_with(&a, 80, &alphabet), format!(" [{letter}] claude"));
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
        assert_eq!(plain(&[a.clone()], 80), " [⋯] my-project");
        a.title = Some("  ".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " [⋯] work");
        a.tab_name = None;
        assert_eq!(plain(&[a.clone()], 80), " [⋯] claude");
        a.app = None;
        assert_eq!(plain(&[a], 80), " [⋯] #7");
    }

    #[test]
    fn tab_name_is_cleaned_and_skipped_when_empty() {
        let mut a = agent(7, AgentState::Idle);
        a.tab_name = Some(" \t\n".to_owned());
        assert_eq!(plain(&[a.clone()], 80), " [⋯] claude");
        a.tab_name = Some("my\ntab".to_owned());
        assert_eq!(plain(&[a], 80), " [⋯] my tab");
    }

    #[test]
    fn hidden_app_is_skipped_as_name() {
        let hide_app = ViewOptions {
            show_app: false,
            ..Default::default()
        };
        let mut a = with_msg(agent(7, AgentState::Idle), "waiting");
        assert_eq!(plain_with(&[a.clone()], 80, &hide_app), " [⋯] #7: waiting");
        a.tab_name = Some("work".to_owned());
        assert_eq!(
            plain_with(&[a.clone()], 80, &hide_app),
            " [⋯] work: waiting"
        );
        a.title = Some("my-project".to_owned());
        assert_eq!(plain_with(&[a], 80, &hide_app), " [⋯] my-project: waiting");
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
        assert_eq!(plain(&agents, 80), " [⋯] z  [⋯] z  [⋯] a");
        let only_claude = ViewOptions {
            apps: Some(vec!["claude".to_owned()]),
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯] z  [⋯] a");
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
        assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯] #1");
        let hide_app = ViewOptions {
            show_app: false,
            ..Default::default()
        };
        assert_eq!(
            plain_with(&agents, 80, &hide_app),
            " [⋯] #1  [⋯] #2  [⋯] #3"
        );
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
            " [↻] claude (issue-21): Running tests"
        );
        // 既定では表示しない
        assert_eq!(plain(&[a], 80), " [↻] claude: Running tests");
    }

    #[test]
    fn cwd_without_last_directory_or_unknown() {
        let options = show_cwd();
        let root = with_cwd(agent(1, AgentState::Idle), "/");
        assert_eq!(plain_with(&[root], 80, &options), " [⋯] claude (/)");
        let trailing = with_cwd(agent(1, AgentState::Idle), "/repo/main/");
        assert_eq!(plain_with(&[trailing], 80, &options), " [⋯] claude (main)");
        let control = with_cwd(agent(1, AgentState::Idle), "/repo/a\nb");
        assert_eq!(plain_with(&[control], 80, &options), " [⋯] claude (a b)");
        // 取得できていなければ、括弧ごと表示しない
        let unknown = agent(1, AgentState::Idle);
        assert_eq!(plain_with(&[unknown], 80, &options), " [⋯] claude");
    }

    #[test]
    fn cwd_and_msg_are_placed_per_agent() {
        let a = with_msg(agent(1, AgentState::Working), "Running tests");
        let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
        assert_eq!(
            plain_with(&[a, b], 80, &show_cwd()),
            " [↻] claude: Running tests  [✓] claude (main)"
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
            line(62),
            " [↻] claude (feature-branch): Running tests  [✓] claude (main)"
        );
        // 1. msg を切り詰める（MIN_MSG_WIDTH まで）
        assert_eq!(
            line(55),
            " [↻] claude (feature-branch): Runni~  [✓] claude (main)"
        );
        // 2. msg を省き、cwd を均等に切り詰める
        assert_eq!(line(54), " [↻] claude (feature-branch)  [✓] claude (main)");
        assert_eq!(line(44), " [↻] claude (feature-br~)  [✓] claude (main)");
        assert_eq!(line(37), " [↻] claude (fea~)  [✓] claude (main)");
        // 3. cwd を省き、名前を切り詰める
        assert_eq!(line(36), " [↻] claude  [✓] claude");
        // 4. アイコンだけにする
        assert_eq!(line(16), " [↻]  [✓]");
    }

    #[test]
    fn control_characters_are_replaced() {
        let a = with_msg(agent(1, AgentState::Done), "line1\nline2\x1b[31m");
        assert_eq!(plain(&[a], 80), " [✓] claude: line1 line2 [31m");
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
        assert_eq!(plain(&[b, c, a], 80), " [⋯] a  [⋯] c  [⋯] b");
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
        assert_eq!(plain(&agents, 80), " [⋯] a  [!] b  [↻] c  [!] d  [✗] e");
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
            " [!] b  [!] d  [⋯] a  [↻] c  [✗] e"
        );
        // Blocked が解消されると元の位置に戻る
        let agents = named_agents([Idle, Working, Working, Blocked, Error]);
        assert_eq!(
            plain_with(&agents, 80, &blocked_first),
            " [!] d  [⋯] a  [↻] b  [↻] c  [✗] e"
        );
        let agents = named_agents([Idle, Working, Working, Done, Error]);
        assert_eq!(
            plain_with(&agents, 80, &blocked_first),
            " [⋯] a  [↻] b  [↻] c  [✓] d  [✗] e"
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
        assert_eq!(plain(&agents, 12), " [⋯]  [⋯]  ~");
        assert_eq!(plain_with(&agents, 12, &blocked_first), " [!]  [⋯]  ~");
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
        assert_eq!(plain_with(&agents, 80, &only_claude), " [⋯] claude");
        // 絞り込まなければ、app のないエージェントも表示する
        assert_eq!(plain(&agents, 80), " [⋯] claude  [⋯] codex  [⋯] #3");
    }

    #[test]
    fn msgs_are_shortened_evenly() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        // 全体：" [↻] claude: " (13) + 20 + "  " (2) + "[!] claude: " (12) + 5 = 52
        assert_eq!(
            plain(&[a.clone(), b.clone()], 52),
            " [↻] claude: abcdefghijklmnopqrst  [!] claude: short"
        );
        // 短い msg はそのまま、長い msg が余りを使う
        assert_eq!(
            plain(&[a, b], 44),
            " [↻] claude: abcdefghijk~  [!] claude: short"
        );
    }

    #[test]
    fn msgs_are_dropped_when_too_narrow() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        // msg に回せる幅が MIN_MSG_WIDTH 未満になる
        assert_eq!(plain(&[a, b], 36), " [↻] claude  [!] claude");
    }

    #[test]
    fn names_are_shortened_then_icons_only() {
        let mut a = agent(1, AgentState::Working);
        a.title = Some("long-project-name".to_owned());
        let b = agent(2, AgentState::Done);
        // 名前に回せる幅は 13：短い "claude" に 6、残り 7 を長い方へ
        assert_eq!(
            plain(&[a.clone(), b.clone()], 24),
            " [↻] long-p~  [✓] claude"
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
        // " [!] claude: " (13) + 16 = 29
        assert_eq!(plain(&a, 29), " [!] claude: 許可してください");
        // msg の幅 8：全角 3 文字（6）+ "~"、残り 1 桁には全角が入らない
        assert_eq!(plain(&a, 21), " [!] claude: 許可し~");
    }

    #[test]
    fn never_exceeds_width() {
        let mut agents = vec![
            with_msg(agent(1, AgentState::Working), "テストを実行しています"),
            with_msg(agent(2, AgentState::Blocked), "Allow Bash(cargo test)?"),
            agent(3, AgentState::Done),
        ];
        agents[0].progress = Some(100);
        agents[0].app = Some("claude-code".to_owned());
        agents[0].cwd = Some("/home/user/全角のディレクトリ".into());
        agents[2].title = Some("全角のタイトル".to_owned());
        agents[2].cwd = Some("/repo/main".into());
        for options in option_combinations() {
            for cols in 0..120 {
                let text = plain_with(&agents, cols, &options);
                assert!(text.width() <= cols, "{options:?} cols={cols}: {text:?}");
            }
        }
    }

    /// アイコン・cwd・Nerd Font モード（区切り）・強調・アイコンの後ろの空白の組み合わせ。
    /// アイコンの後ろの空白が効かない組み合わせでは、同じ設定が 2 回入る（重複は気にしない）
    fn option_combinations() -> Vec<ViewOptions> {
        let styles = [
            IconStyle::Symbol,
            IconStyle::Alphabet,
            IconStyle::Emoji,
            IconStyle::Nerd,
        ];
        let modes = [
            (false, Separator::Arrow),
            (true, Separator::Arrow),
            (true, Separator::Round),
            (true, Separator::Slash),
        ];
        let mut combinations = vec![];
        for style in styles {
            for show_cwd in [false, true] {
                for (nerd_font, separator) in modes {
                    for highlight_attention in [false, true] {
                        // アイコンの後ろの空白は Nerd Font のアイコンがあるときだけ効く
                        let uses_nerd_icon = nerd_font || style == IconStyle::Nerd;
                        for nerd_font_icon_padding in [false, uses_nerd_icon] {
                            combinations.push(ViewOptions {
                                show_cwd,
                                nerd_font,
                                separator,
                                highlight_attention,
                                nerd_font_icon_padding,
                                ..icons(style)
                            });
                        }
                    }
                }
            }
        }
        combinations
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
            if span.role == Role::Plain && span.text == options.separator_text() {
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
        // "[↻] claude: " (12) + msg 10 = 22。短い方はそのまま
        assert_eq!(
            plain_with(&[a, b], 80, &max_length(22)),
            " [↻] claude: abcdefghi~  [!] claude: short"
        );
    }

    #[test]
    fn max_length_drops_msg_then_truncates_name() {
        let mut a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        // msg に回せる幅が 4（MIN_MSG_WIDTH 未満）なので省く
        assert_eq!(plain_with(&[a.clone()], 80, &max_length(16)), " [↻] claude");
        // 収まる msg なら MIN_MSG_WIDTH 未満の幅でも表示する
        let short = with_msg(agent(1, AgentState::Working), "ok");
        assert_eq!(plain_with(&[short], 80, &max_length(16)), " [↻] claude: ok");

        a.title = Some("long-project-name".to_owned());
        assert_eq!(plain_with(&[a], 80, &max_length(16)), " [↻] long-projec~");
    }

    #[test]
    fn max_length_counts_icon_and_progress() {
        let mut a = with_msg(agent(1, AgentState::Working), "Running");
        a.title = Some("my-project".to_owned());
        a.progress = Some(100);
        assert_eq!(
            plain_with(&[a.clone()], 80, &max_length(MIN_MAX_LENGTH)),
            " [↻ 100%] my-pro~"
        );
        let emoji = ViewOptions {
            icons: IconStyle::Emoji,
            ..max_length(MIN_MAX_LENGTH)
        };
        // 絵文字はバッジの端がない分、名前に回せる幅が広い
        assert_eq!(plain_with(&[a.clone()], 80, &emoji), " ⏳ 100% my-proj~");
        // Nerd Font モードでは名前の後ろの app のアイコンと前の空白の 2 も数える
        a.app = Some("claude-code".to_owned());
        let nerd = ViewOptions {
            nerd_font: true,
            icons: IconStyle::Nerd,
            ..max_length(MIN_MAX_LENGTH)
        };
        assert_eq!(
            plain_with(&[a.clone()], 80, &nerd),
            " [\u{f013} 100%] my-p~ \u{ec82}"
        );
        // アイコンの後ろの空白も数える（名前は MIN_NAME_WIDTH まで縮む）
        let padded = ViewOptions {
            nerd_font_icon_padding: true,
            ..nerd
        };
        assert_eq!(
            plain_with(&[a], 80, &padded),
            " [\u{f013} 100%] my-~ \u{ec82} "
        );
    }

    #[test]
    fn max_length_is_combined_with_row_width() {
        let a = with_msg(agent(1, AgentState::Working), "abcdefghijklmnopqrst");
        let b = with_msg(agent(2, AgentState::Blocked), "short");
        let options = max_length(22);
        // 上限で切り詰めた幅（msg 10 と 5）を、行の幅に合わせてさらに均等に縮める
        assert_eq!(
            plain_with(&[a.clone(), b.clone()], 40, &options),
            " [↻] claude: abcdefg~  [!] claude: short"
        );
        assert_eq!(plain_with(&[a, b], 24, &options), " [↻] claude  [!] claude");
    }

    #[test]
    fn max_length_defaults_to_50_and_none_is_unlimited() {
        let msg = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let a = with_msg(agent(1, AgentState::Working), msg);
        // "[↻] claude: " (12) + msg 38 = 50
        assert_eq!(
            plain(std::slice::from_ref(&a), 80),
            " [↻] claude: abcdefghijklmnopqrstuvwxyzABCDEFGHIJK~"
        );
        let unlimited = ViewOptions {
            max_length: None,
            ..Default::default()
        };
        assert_eq!(
            plain_with(&[a], 80, &unlimited),
            format!(" [↻] claude: {msg}")
        );
    }

    #[test]
    fn max_length_leaves_short_agents_unchanged() {
        let a = with_msg(agent(1, AgentState::Done), "ok");
        assert_eq!(plain_with(&[a], 80, &max_length(40)), " [✓] claude: ok");
    }

    #[test]
    fn min_max_length_fits_widest_head_and_min_name() {
        let mut a = agent(1, AgentState::Blocked);
        a.app = Some("claude-code".to_owned());
        a.progress = Some(u8::MAX);
        let widths: Vec<usize> = option_combinations()
            .iter()
            .map(|options| {
                let item = Item::new(&a, options);
                item.head_width() + item.name_overhead() + MIN_NAME_WIDTH
            })
            .collect();
        // 最も広いのは、Nerd Font モードで絵文字以外の状態アイコン（バッジの端あり）を選び、
        // アイコンの後ろに空白を置くとき
        assert_eq!(widths.iter().max(), Some(&MIN_MAX_LENGTH));
        let options = ViewOptions {
            nerd_font: true,
            nerd_font_icon_padding: true,
            ..icons(IconStyle::Nerd)
        };
        let item = Item::new(&a, &options);
        assert!(item.app_icon.is_some());
        assert_eq!(
            item.head_width() + item.name_overhead() + MIN_NAME_WIDTH,
            MIN_MAX_LENGTH
        );
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

        assert_eq!(line(42), " [↻] claude (feature-branch): Running tests");
        // msg を切り詰める（MIN_MSG_WIDTH まで）
        assert_eq!(line(37), " [↻] claude (feature-branch): Running~");
        // msg を省き、cwd を切り詰める（MIN_CWD_WIDTH まで）
        assert_eq!(line(32), " [↻] claude (feature-branch)");
        assert_eq!(line(22), " [↻] claude (feature-~)");
        assert_eq!(line(17), " [↻] claude (fea~)");
        // cwd も省き、名前を切り詰める
        assert_eq!(line(16), " [↻] claude");
        let mut long = a.clone();
        long.title = Some("long-project-name".to_owned());
        assert_eq!(
            plain_with(&[long], 80, &max_length_with_cwd(16)),
            " [↻] long-projec~"
        );
    }

    #[test]
    fn max_length_with_cwd_is_combined_with_row_width() {
        let a = with_cwd(agent(1, AgentState::Working), "/repo/feature-branch");
        let b = with_cwd(agent(2, AgentState::Done), "/repo/main");
        let options = max_length_with_cwd(22);
        // 上限で切り詰めた cwd（9 と 4）を、行の幅に合わせてさらに均等に縮める
        assert_eq!(
            plain_with(&[a.clone(), b.clone()], 80, &options),
            " [↻] claude (feature-~)  [✓] claude (main)"
        );
        assert_eq!(
            plain_with(&[a, b], 40, &options),
            " [↻] claude (featur~)  [✓] claude (main)"
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
        agents[0].app = Some("claude-code".to_owned());
        agents[0].cwd = Some("/home/user/全角のディレクトリ".into());
        agents[2].title = Some("全角のタイトル".to_owned());
        agents[2].cwd = Some("/repo/main".into());
        for base in option_combinations() {
            for max in MIN_MAX_LENGTH..40 {
                let options = ViewOptions {
                    max_length: Some(max),
                    ..base.clone()
                };
                for cols in 0..100 {
                    let text = plain_with(&agents, cols, &options);
                    let case = format!("{options:?} cols={cols}");
                    assert!(text.width() <= cols, "{case}");
                    for width in agent_widths(&agents, cols, &options) {
                        assert!(width <= max, "{case}: {text:?}");
                    }
                }
            }
        }
    }

    fn nerd_font(separator: Separator) -> ViewOptions {
        ViewOptions {
            nerd_font: true,
            separator,
            ..icons(IconStyle::Nerd)
        }
    }

    #[test]
    fn nerd_font_shows_app_icon_and_separator() {
        let mut a = with_msg(agent(1, AgentState::Working), "Running tests");
        a.app = Some("claude-code".to_owned());
        let mut b = agent(2, AgentState::Done);
        b.app = Some("codex".to_owned());
        let agents = [a, b];
        // app のアイコンは名前の後ろ。対応するアイコンのない app（codex）には付けない
        assert_eq!(
            plain_with(&agents, 80, &nerd_font(Separator::Arrow)),
            " [\u{f013}] claude-code \u{ec82}: Running tests \u{e0b1} [\u{f00c}] codex"
        );
        assert_eq!(
            plain_with(&agents, 80, &nerd_font(Separator::Round)),
            " [\u{f013}] claude-code \u{ec82}: Running tests \u{e0b5} [\u{f00c}] codex"
        );
        assert_eq!(
            plain_with(&agents, 80, &nerd_font(Separator::Slash)),
            " [\u{f013}] claude-code \u{ec82}: Running tests \u{e0bb} [\u{f00c}] codex"
        );
        // cwd は app のアイコンの後ろ
        let with_dir = [with_cwd(agents[0].clone(), "/repo/main")];
        let options = ViewOptions {
            show_cwd: true,
            ..nerd_font(Separator::Arrow)
        };
        assert_eq!(
            plain_with(&with_dir, 80, &options),
            " [\u{f013}] claude-code \u{ec82} (main): Running tests"
        );
        // Nerd Font モードでなければ、app のアイコンも区切り文字も使わない
        let symbol_only = ViewOptions {
            separator: Separator::Slash,
            ..Default::default()
        };
        assert_eq!(
            plain_with(&agents, 80, &symbol_only),
            " [↻] claude-code: Running tests  [✓] codex"
        );
    }

    #[test]
    fn nerd_font_app_icon_ignores_show_app() {
        let mut a = agent(7, AgentState::Idle);
        a.app = Some("claude-code".to_owned());
        let options = ViewOptions {
            show_app: false,
            ..nerd_font(Separator::Arrow)
        };
        assert_eq!(plain_with(&[a], 80, &options), " [\u{f186}] #7 \u{ec82}");
    }

    #[test]
    fn nerd_font_can_be_combined_with_other_icon_styles() {
        let mut a = agent(1, AgentState::Done);
        a.app = Some("claude-code".to_owned());
        let options = ViewOptions {
            icons: IconStyle::Emoji,
            ..nerd_font(Separator::Arrow)
        };
        assert_eq!(plain_with(&[a], 80, &options), " ✅ claude-code \u{ec82}");
    }

    #[test]
    fn nerd_font_widths_are_counted_when_shortening() {
        let mut a = agent(1, AgentState::Working);
        a.app = Some("claude-code".to_owned());
        a.title = Some("long-project-name".to_owned());
        let b = agent(2, AgentState::Done);
        let agents = [a, b];
        let options = ViewOptions {
            max_length: None,
            ..nerd_font(Separator::Arrow)
        };
        // 固定部分：" " (1) + "[\u{f013}]" (3) + " \u{e0b1} " (3) + "[\u{f00c}]" (3) = 10
        // 名前に付く部分：" " + " \u{ec82}" (3) と " " (1) = 4
        // 名前に回せる幅は 26 - 10 - 4 = 12：短い "claude" に 6、残り 6 を長い方へ
        assert_eq!(
            plain_with(&agents, 26, &options),
            " [\u{f013}] long-~ \u{ec82} \u{e0b1} [\u{f00c}] claude"
        );
        // アイコンの後ろに空白を置くと、その分も数える
        // 固定部分は 12（状態アイコンの後ろの空白 2）、名前に付く部分は 5（app のアイコンの後ろの空白 1）
        let padded = ViewOptions {
            nerd_font_icon_padding: true,
            ..options.clone()
        };
        assert_eq!(
            plain_with(&agents, 29, &padded),
            " [\u{f013} ] long-~ \u{ec82}  \u{e0b1} [\u{f00c} ] claude"
        );
        // 状態アイコンだけにする。app のアイコンも名前と一緒に省く
        assert_eq!(
            plain_with(&agents, 17, &options),
            " [\u{f013}] \u{e0b1} [\u{f00c}]"
        );
    }

    #[test]
    fn nerd_font_icon_padding_adds_space_after_icons() {
        let mut a = with_msg(agent(1, AgentState::Blocked), "Allow?");
        a.app = Some("claude-code".to_owned());
        let mut b = with_cwd(agent(2, AgentState::Done), "/repo/main");
        b.app = Some("claude-code".to_owned());
        b.progress = Some(42);
        let agents = [a, b];
        let padded = ViewOptions {
            show_cwd: true,
            nerd_font_icon_padding: true,
            ..nerd_font(Separator::Arrow)
        };
        // 進捗があれば、その前の空白が状態アイコンの右隣になるので足さない
        assert_eq!(
            plain_with(&agents, 80, &padded),
            " [\u{f071} ] claude-code \u{ec82} : Allow? \u{e0b1} [\u{f00c} 42%] claude-code \u{ec82}  (main)"
        );
        // 強調しても幅は変わらない（状態アイコンと名前の間は空白 2 つ）
        let highlighted = ViewOptions {
            highlight_attention: true,
            ..padded.clone()
        };
        assert_eq!(
            plain_with(&agents, 80, &highlighted),
            " [\u{f071}  claude-code \u{ec82} : Allow?] \u{e0b1} [\u{f00c} 42%] claude-code \u{ec82}  (main)"
        );
        // Nerd Font のアイコンを使わなければ何も変わらない
        let symbol = ViewOptions {
            nerd_font_icon_padding: true,
            ..Default::default()
        };
        assert_eq!(plain_with(&agents, 80, &symbol), plain(&agents, 80));
    }

    #[test]
    fn nerd_font_icon_padding_puts_space_after_every_nerd_icon() {
        use AgentState::*;
        let states = [Idle, Working, Blocked, Done, Error];
        let icons: Vec<char> = states
            .iter()
            .map(|&s| IconStyle::Nerd.icon(s))
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

    /// 状態の色で塗った部分を、続いている範囲ごとにまとめたもの
    fn filled_runs(agents: &[AgentStatus], cols: usize, options: &ViewOptions) -> Vec<String> {
        let mut runs: Vec<String> = vec![];
        let mut prev = Fill::None;
        for span in layout(agents, cols, options) {
            if span.fill != Fill::None {
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

    #[test]
    fn badge_ends_depend_on_nerd_font_mode() {
        let a = [agent(1, AgentState::Working)];
        assert_eq!(badge_ends(&a, &ViewOptions::default()), [" ", " "]);
        assert_eq!(
            badge_ends(&a, &nerd_font(Separator::Arrow)),
            [LEFT_HALF_CIRCLE, RIGHT_HALF_CIRCLE]
        );
        // Nerd Font モードでなければ、状態アイコンが Nerd Font でも空白
        assert_eq!(badge_ends(&a, &icons(IconStyle::Nerd)), [" ", " "]);
        // 絵文字はバッジにしない
        assert!(badge_ends(&a, &icons(IconStyle::Emoji)).is_empty());
        assert_nerd_font_glyph(LEFT_HALF_CIRCLE);
        assert_nerd_font_glyph(RIGHT_HALF_CIRCLE);
    }

    #[test]
    fn badge_fills_icon_and_progress() {
        let mut a = with_msg(agent(1, AgentState::Working), "Running");
        a.progress = Some(42);
        let a = [a];
        assert_eq!(filled_runs(&a, 80, &ViewOptions::default()), [" ↻ 42% "]);
        // 半円は塗らずに前景色で描くので、塗るのはその内側だけ
        assert_eq!(
            filled_runs(&a, 80, &nerd_font(Separator::Arrow)),
            ["\u{f013} 42%"]
        );
        assert!(filled_runs(&a, 80, &icons(IconStyle::Emoji)).is_empty());
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
        let options = highlight(ViewOptions::default());
        let line = plain_with(&agents, 80, &options);
        assert_eq!(line, " [⋯] a  [! b: Allow Bash?]  [↻] c  [✓] d  [✗ e]");
        assert_eq!(
            filled_runs(&agents, 80, &options),
            [" ⋯ ", " ! b: Allow Bash? ", " ↻ ", " ✓ ", " ✗ e "]
        );
        // 端の文字を移すだけなので、強調しても幅は変わらない
        assert_eq!(line.width(), plain(&agents, 80).width());
        // 既定では強調しない
        assert_eq!(
            plain(&agents, 80),
            " [⋯] a  [!] b: Allow Bash?  [↻] c  [✓] d  [✗] e"
        );
    }

    #[test]
    fn highlight_attention_with_nerd_font_moves_half_circle_to_agent_end() {
        let mut a = with_msg(agent(1, AgentState::Blocked), "Allow?");
        a.app = Some("claude-code".to_owned());
        let a = [a];
        let options = highlight(nerd_font(Separator::Arrow));
        let raw: String = layout(&a, 80, &options)
            .into_iter()
            .map(|s| s.text)
            .collect();
        assert_eq!(
            raw,
            " \u{e0b6}\u{f071} claude-code \u{ec82}: Allow?\u{e0b4}"
        );
        assert_eq!(
            filled_runs(&a, 80, &options),
            ["\u{f071} claude-code \u{ec82}: Allow?"]
        );
    }

    #[test]
    fn highlight_attention_with_emoji_fills_without_ends() {
        let agents = [
            with_msg(agent(1, AgentState::Error), "failed"),
            agent(2, AgentState::Done),
        ];
        let options = highlight(icons(IconStyle::Emoji));
        assert_eq!(
            plain_with(&agents, 80, &options),
            " ❌ claude: failed  ✅ claude"
        );
        assert_eq!(filled_runs(&agents, 80, &options), ["❌ claude: failed"]);
    }

    #[test]
    fn clamp_fills_ellipsis_like_cut_span() {
        let fill = Fill::State(AgentState::Error);
        let spans = vec![
            Span::new("abc", Role::Msg).with_fill(fill),
            Span::new("de", Role::Plain),
        ];
        // 塗った部分の途中で切れたら、~ も塗る
        assert_eq!(
            clamp(spans.clone(), 3),
            [
                Span::new("ab", Role::Msg).with_fill(fill),
                Span::new("~", Role::Plain).with_fill(fill),
            ]
        );
        // 塗っていない部分で切れたら、~ は塗らない
        assert_eq!(
            clamp(spans, 4),
            [
                Span::new("abc", Role::Msg).with_fill(fill),
                Span::new("~", Role::Plain),
            ]
        );
    }

    #[test]
    fn fair_share_gives_leftover_to_longer_items() {
        assert_eq!(fair_share(&[3, 20, 20], 20), vec![3, 8, 9]);
        assert_eq!(fair_share(&[3, 4], 100), vec![3, 4]);
        assert_eq!(fair_share(&[], 10), Vec::<usize>::new());
    }
}
