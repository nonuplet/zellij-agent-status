//! 1 エージェント分の表示内容：何を表示するかを決め、Span 列を組み立てる。
//!
//! 1 エージェント分は `アイコン 進捗 名前 (cwd): msg` の形（進捗・cwd・msg はあるときだけ）。
//! Nerd Font モードでは、名前の前に `app` のアイコンを付けて `<app> 名前 (cwd)` の形にする（対応するアイコンがあるときだけ）。
//! `app` のアイコンは名前と一体で扱い、名前を省くときは一緒に省く。
//! [`ViewOptions::nerd_font_icon_padding`] が true なら、Nerd Font のアイコン（状態アイコンと `app` のアイコン）の
//! 右隣に必ず空白を置く（`[<icon> ] <app>  名前: msg`。[`app_icon`](super::icon::app_icon) を参照）。
//! msg は [`ViewOptions::msg_mode`] で表示するエージェントにだけ持たせる。持たせないエージェントは
//! msg がないのと同じに扱うので、空いた幅は cwd や名前に回る。
//!
//! 幅によらない固定の部分（アイコン・進捗・バッジの端・名前の前の部分・区切り）は [`Item::new`] で Span にしておき、
//! 幅の計算（[`fit`](super::fit)）でもその Span の幅を使う。組み立てと幅の計算で、同じ文字列を別々に数えないようにするため。
//!
//! どこまでをバッジにするかは [`badge`](super::badge)、幅に合わせた縮め方は [`fit`](super::fit) を参照。

use unicode_width::UnicodeWidthStr;

use super::ViewOptions;
use super::badge::{BadgeExtent, BadgeStyle};
use super::span::{Fill, Role, Span, width};
use super::text::{clean, dir_name, truncate};
use crate::model::{AgentState, AgentStatus};

/// 名前の後ろの cwd を囲む文字列
const CWD_OPEN: &str = " (";
const CWD_CLOSE: &str = ")";

/// 各部分を表示する幅の上限。None なら表示しない。
/// cwd と msg は名前に続けて表示するので、名前が None なら表示しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Widths {
    pub(super) name: Option<usize>,
    pub(super) cwd: Option<usize>,
    pub(super) msg: Option<usize>,
}

/// 1 エージェント分の表示内容（切り詰める前）
pub(super) struct Item {
    state: AgentState,
    /// バッジに入れる範囲
    extent: BadgeExtent,
    /// バッジの左端と右端（内側の空白を含む）。バッジにしないなら空
    left: Vec<Span>,
    right: Vec<Span>,
    /// 状態アイコンと進捗（とアイコンの後ろの空白）
    head: Vec<Span>,
    /// 名前の前に付ける部分（前の空白と `app` のアイコン）。名前と一緒に表示し、一緒に省く
    name_prefix: Vec<Span>,
    name: String,
    /// カレントディレクトリの末尾のディレクトリ名（`show_cwd` が false なら None）
    cwd: Option<String>,
    /// [`ViewOptions::msg_mode`] で表示しないエージェントでは None
    msg: Option<String>,
    /// 名前（と cwd）と msg の間の区切り（[`BadgeExtent::msg_separator`]）
    msg_separator: Span,
}

impl Item {
    /// 名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決める
    /// （`show_tab_name` が false ならタブ名を、`show_app_name` が false なら `app` を飛ばす）。
    /// Claude Code は `title` を送らないため、タブ名で複数のエージェントを見分けられるようにしている。
    /// cwd と msg は名前に続けて表示するので、名前は必ずある（最後は `#<pane_id>`）ようにしている。
    /// `app` のアイコンは名前の候補とは別なので、`show_app_name` に関係なく付ける。
    pub(super) fn new(agent: &AgentStatus, options: &ViewOptions, style: &BadgeStyle) -> Self {
        let tab_name = agent.tab_name.as_deref().filter(|_| options.show_tab_name);
        let app = agent.app.as_deref().filter(|_| options.show_app_name);
        let name = agent
            .title
            .as_deref()
            .and_then(clean)
            .or_else(|| tab_name.and_then(clean))
            .or_else(|| app.and_then(clean))
            .unwrap_or_else(|| format!("#{}", agent.pane_id));
        let cwd = agent
            .cwd
            .as_deref()
            .filter(|_| options.show_cwd)
            .and_then(|cwd| clean(&dir_name(cwd)));

        let state = Role::State(agent.state);
        let mut head = vec![Span::new(style.icon(agent.state), state)];
        if let Some(progress) = agent.progress {
            head.push(Span::new(format!(" {progress}%"), state));
        }
        if style.pads_state_icon(agent.progress.is_some()) {
            head.push(Span::new(" ", state));
        }

        // app のアイコンは名前の前に置く（右隣に必ず名前の前の空白が来る。src/view/badge.rs の説明を参照）
        let mut name_prefix = vec![];
        if let Some(icon) = style.app_icon(agent.app.as_deref()) {
            name_prefix.push(Span::new(" ", Role::Plain));
            name_prefix.push(Span::new(icon, Role::App));
            if style.pads_app_icon() {
                name_prefix.push(Span::new(" ", Role::Plain));
            }
        }
        name_prefix.push(Span::new(" ", Role::Plain));

        let (left, right) = style
            .ends()
            .map_or_else(Default::default, |e| e.spans(agent.state));
        let extent = style.extent(agent.state);
        Item {
            state: agent.state,
            extent,
            left,
            right,
            head,
            name_prefix,
            name,
            cwd,
            msg: agent
                .msg
                .as_deref()
                .and_then(clean)
                .filter(|_| options.msg_mode.shows(agent.state)),
            msg_separator: Span::new(extent.msg_separator(), Role::Plain),
        }
    }

    /// 各部分を切り詰めずに表示するときの幅
    pub(super) fn widths(&self) -> Widths {
        Widths {
            name: Some(self.name.width()),
            cwd: self.cwd.as_ref().map(|c| c.width()),
            msg: self.msg.as_ref().map(|m| m.width()),
        }
    }

    /// アイコンと進捗、バッジの両端とその内側の空白の幅（常に表示する部分）。
    /// 名前を入れたり強調したりするとバッジの右端が後ろに移るが、幅は同じなのでここに含める。
    pub(super) fn head_width(&self) -> usize {
        width(&self.left) + width(&self.head) + width(&self.right)
    }

    /// 名前に付く部分（前の空白と `app` のアイコン）の幅。名前と一緒に表示し、一緒に省く
    pub(super) fn name_overhead(&self) -> usize {
        width(&self.name_prefix)
    }

    /// cwd に付く部分（前後の括弧）の幅。
    /// どのエージェントでも同じだが、名前・msg に付く部分と同じ形（`fn(&Item) -> usize`）で
    /// [`fit`](super::fit) に渡すため、メソッドにしている。
    pub(super) fn cwd_overhead(&self) -> usize {
        CWD_OPEN.width() + CWD_CLOSE.width()
    }

    /// msg に付く部分（前の区切り）の幅
    pub(super) fn msg_overhead(&self) -> usize {
        self.msg_separator.text.width()
    }

    /// `app` のアイコン・cwd・msg は名前を表示するときだけ表示する。
    ///
    /// 1 エージェント分を `アイコン 進捗` / ` <app> 名前 (cwd)` / `: msg` の 3 つの部分に分け、
    /// 先頭から [`Item::extent`] の部分までをバッジに入れる（塗る）。バッジの右端（と内側の空白）は、
    /// バッジに入れた部分の直後に置く。
    /// 名前を省いたときは名前の部分が空になるので、右端はアイコンと進捗の直後に来る。
    pub(super) fn spans(&self, widths: Widths) -> Vec<Span> {
        let mut label = vec![];
        let mut msg = vec![];
        if let Some(width) = widths.name {
            label.extend_from_slice(&self.name_prefix);
            label.push(Span::new(truncate(&self.name, width), Role::Name));
            if let (Some(text), Some(width)) = (&self.cwd, widths.cwd) {
                label.push(Span::new(CWD_OPEN, Role::Plain));
                label.push(Span::new(truncate(text, width), Role::Cwd));
                label.push(Span::new(CWD_CLOSE, Role::Plain));
            }
            if let (Some(text), Some(width)) = (&self.msg, widths.msg) {
                msg.push(self.msg_separator.clone());
                msg.push(Span::new(truncate(text, width), Role::Msg));
            }
        }

        let fill = Fill::State(self.state);
        let mut parts = [self.head.clone(), label, msg].into_iter();
        let mut spans = self.left.clone();
        spans.extend(
            parts
                .by_ref()
                .take(self.extent.parts())
                .flatten()
                .map(|s| s.with_fill(fill)),
        );
        spans.extend_from_slice(&self.right);
        spans.extend(parts.flatten());
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::MIN_MAX_LENGTH;
    use crate::view::fit::MIN_NAME_WIDTH;
    use crate::view::icon::IconStyle;
    use crate::view::test_support::{agent, icons, option_combinations};

    fn item(agent: &AgentStatus, options: &ViewOptions) -> Item {
        Item::new(agent, options, &BadgeStyle::new(options))
    }

    #[test]
    fn min_max_length_fits_widest_head_and_min_name() {
        let mut a = agent(1, AgentState::Blocked);
        a.app = Some("claude-code".to_owned());
        a.progress = Some(u8::MAX);
        let widths: Vec<usize> = option_combinations()
            .iter()
            .map(|options| {
                let item = item(&a, options);
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
        let item = item(&a, &options);
        assert!(item.name_prefix.iter().any(|s| s.role == Role::App));
        assert_eq!(
            item.head_width() + item.name_overhead() + MIN_NAME_WIDTH,
            MIN_MAX_LENGTH
        );
    }
}
