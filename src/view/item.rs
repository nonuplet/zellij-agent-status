//! 1 エージェント分の表示内容：何を表示するかを決め、Span 列を組み立てる。
//!
//! 1 エージェント分は `アイコン 進捗 名前 (cwd): msg` の形（進捗・cwd・msg はあるときだけ）。
//! Nerd Font モードでは、名前の前に `app` のアイコンを付けて `<app> 名前 (cwd)` の形にする（対応するアイコンがあるときだけ）。
//! `app` のアイコンは名前と一体で扱い、名前を省くときは一緒に省く。
//! [`ViewOptions::nerd_font_icon_padding`] が true なら、Nerd Font のアイコン（状態アイコンと `app` のアイコン）の
//! 右隣に必ず空白を置く（`[<icon> ] <app>  名前: msg`。[`app_icon`] を参照）。
//! msg は [`ViewOptions::msg_mode`] で表示するエージェントにだけ持たせる。持たせないエージェントは
//! msg がないのと同じに扱うので、空いた幅は cwd や名前に回る。
//!
//! どこまでをバッジにするかは [`badge`](super::badge)、幅に合わせた縮め方は [`fit`](super::fit) を参照。

use unicode_width::UnicodeWidthStr;

use super::ViewOptions;
use super::badge::BadgeEnds;
use super::fit::{MIN_CWD_WIDTH, MIN_MSG_WIDTH, Widths};
use super::icon::{IconStyle, app_icon};
use super::span::{Fill, Role, Span};
use super::text::{clean, dir_name, truncate};
use crate::model::{AgentState, AgentStatus};

/// 名前の後ろに付ける cwd を囲む ` (` と `)` の幅
pub(super) const CWD_OVERHEAD: usize = 3;

/// 1 エージェント分の表示内容（切り詰める前）
pub(super) struct Item {
    state: AgentState,
    /// バッジの端の文字。絵文字アイコンなら None（バッジにしない）
    ends: Option<BadgeEnds>,
    /// エージェント 1 つ分全体をバッジにするか（[`ViewOptions::highlight_attention`]）
    highlight: bool,
    /// 名前（と `app` のアイコン・cwd）までをバッジにするか（[`ViewOptions::badge_name`]）。
    /// バッジにしない（[`Item::ends`] が None）なら false
    badge_name: bool,
    icon: &'static str,
    /// 状態アイコンが Nerd Font か
    nerd_icon: bool,
    /// Nerd Font のアイコンの右隣に空白を置くか（[`ViewOptions::nerd_font_icon_padding`]）
    icon_padding: bool,
    /// `42%` の形
    progress: Option<String>,
    name: String,
    /// 名前の前に付ける `app` のアイコン（Nerd Font モードで、対応するアイコンがあるときだけ）
    app_icon: Option<&'static str>,
    /// カレントディレクトリの末尾のディレクトリ名（`show_cwd` が false なら None）
    cwd: Option<String>,
    /// [`ViewOptions::msg_mode`] で表示しないエージェントでは None
    msg: Option<String>,
}

impl Item {
    /// 名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決める
    /// （`show_tab_name` が false ならタブ名を、`show_app_name` が false なら `app` を飛ばす）。
    /// Claude Code は `title` を送らないため、タブ名で複数のエージェントを見分けられるようにしている。
    /// 名前は必ずある前提で [`plan`](super::fit::plan) と [`Item::spans`] を書いているので、最後は `#<pane_id>` にする。
    /// `app` のアイコンは名前の候補とは別なので、`show_app_name` に関係なく付ける。
    pub(super) fn new(agent: &AgentStatus, options: &ViewOptions) -> Self {
        let tab_name = agent.tab_name.as_deref().filter(|_| options.show_tab_name);
        let app = agent.app.as_deref().filter(|_| options.show_app_name);
        let name = clean(agent.title.as_deref())
            .or_else(|| clean(tab_name))
            .or_else(|| clean(app))
            .unwrap_or_else(|| format!("#{}", agent.pane_id));
        let cwd = agent
            .cwd
            .as_deref()
            .filter(|_| options.show_cwd)
            .and_then(|cwd| clean(Some(&dir_name(cwd))));
        let ends = BadgeEnds::new(options);
        Item {
            state: agent.state,
            ends,
            highlight: options.highlight_attention && agent.state.needs_attention(),
            badge_name: options.badge_name && ends.is_some(),
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
            msg: clean(agent.msg.as_deref()).filter(|_| options.msg_mode.shows(agent.state)),
        }
    }

    /// アイコンと進捗、バッジの両端とその内側の空白の幅（常に表示する部分）。
    /// 名前を入れたり強調したりするとバッジの右端が後ろに移るが、幅は同じなのでここに含める。
    pub(super) fn head_width(&self) -> usize {
        self.icon.width()
            + self.progress.as_ref().map_or(0, |p| 1 + p.width())
            + usize::from(self.icon_pad())
            + self.ends.map_or(0, BadgeEnds::width)
    }

    /// 状態アイコンの後ろに空白を付けるか（Nerd Font のアイコンで、[`Item::icon_padding`] が true のとき）。
    /// 進捗があればその前の空白が右隣になるので、付けない。
    /// 強調して名前が続くときも付ける（幅を状態によらず同じにするため。名前との間は空白 2 つになる）。
    fn icon_pad(&self) -> bool {
        self.icon_padding && self.nerd_icon && self.progress.is_none()
    }

    /// 名前に付く部分（前の空白と、前の `app` のアイコンとその前の空白、
    /// [`Item::icon_padding`] ならアイコンの後ろの空白も）の幅。名前と一緒に表示し、一緒に省く
    pub(super) fn name_overhead(&self) -> usize {
        1 + self
            .app_icon
            .map_or(0, |i| 1 + i.width() + usize::from(self.icon_padding))
    }

    /// 名前（と cwd）と msg の間の区切り。
    /// 名前までをバッジにする（強調しない）ときは、バッジの右端が境目になるので空白 1 つにする。
    /// それ以外（名前と msg が同じ背景で続く）は `: ` にする。強調すると幅が 1 広がる。
    pub(super) fn msg_separator(&self) -> &'static str {
        if self.badge_name && !self.highlight {
            " "
        } else {
            ": "
        }
    }

    /// 各部分の幅の上限（名前は常に Some）。`max_length` を指定していれば、1 エージェント分がその幅に収まるようにする。
    /// msg を切り詰め、[`MIN_MSG_WIDTH`] を下回るなら msg を省いて cwd を切り詰め、
    /// [`MIN_CWD_WIDTH`] を下回るなら cwd も省いて名前を切り詰める。
    pub(super) fn widths(&self, max_length: Option<usize>) -> Widths {
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
        // 名前（と cwd・msg）に使える幅。名前の前の空白と app のアイコンを除く
        let room = max_length.saturating_sub(self.head_width() + self.name_overhead());
        let cwd_total = cwd.map_or(0, |c| c + CWD_OVERHEAD);
        let fits = |width: usize, room: usize, min: usize| width <= room || room >= min;

        if let Some(msg) = msg
            && let Some(msg_room) =
                room.checked_sub(name + cwd_total + self.msg_separator().width())
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
    /// 1 エージェント分を `アイコン 進捗` / ` <app> 名前 (cwd)` / `: msg` の 3 つに分け、
    /// 先頭から続けてバッジに入れる（塗る）。バッジの右端（と内側の空白。[`BadgeEnds::inner_pad`]）は、
    /// 最初に入れない部分の手前に置く。
    /// - アイコンと進捗：端の文字があるか、強調するときに入れる
    /// - 名前（と `app` のアイコン・cwd）：[`Item::badge_name`] か、強調するときに入れる
    /// - msg：強調するときだけ入れる
    ///
    /// 名前を省いたときは名前の部分が空になるので、右端はアイコンと進捗の直後に来る。
    /// 絵文字アイコン（端の文字なし）は端の内側の空白も付けないが、強調するときは全体を塗る。
    pub(super) fn spans(&self, widths: Widths) -> Vec<Span> {
        let state = Fill::State(self.state);
        let (left, mut right) = self
            .ends
            .map_or_else(Default::default, |e| e.spans(self.state));

        let mut head = vec![Span::new(self.icon, Role::State(self.state))];
        if let Some(progress) = &self.progress {
            head.push(Span::new(format!(" {progress}"), Role::State(self.state)));
        }
        if self.icon_pad() {
            head.push(Span::new(" ", Role::State(self.state)));
        }
        let mut label = vec![];
        let mut msg = vec![];
        if let Some(width) = widths.name {
            // app のアイコンは名前の前に置く（右隣に必ず名前の前の空白が来る。src/view/badge.rs の説明を参照）
            if let Some(icon) = self.app_icon {
                label.push(Span::new(" ", Role::Plain));
                label.push(Span::new(icon, Role::App));
                if self.icon_padding {
                    label.push(Span::new(" ", Role::Plain));
                }
            }
            label.push(Span::new(" ", Role::Plain));
            label.push(Span::new(truncate(&self.name, width), Role::Name));
            if let (Some(text), Some(width)) = (&self.cwd, widths.cwd) {
                label.push(Span::new(" (", Role::Plain));
                label.push(Span::new(truncate(text, width), Role::Cwd));
                label.push(Span::new(")", Role::Plain));
            }
            if let (Some(text), Some(width)) = (&self.msg, widths.msg) {
                msg.push(Span::new(self.msg_separator(), Role::Plain));
                msg.push(Span::new(truncate(text, width), Role::Msg));
            }
        }

        let parts = [
            (head, self.ends.is_some() || self.highlight),
            (label, self.badge_name || self.highlight),
            (msg, self.highlight),
        ];
        let mut spans = left;
        for (part, in_badge) in parts {
            if in_badge {
                spans.extend(part.into_iter().map(|s| s.with_fill(state)));
            } else {
                // バッジに入れる部分は先頭から続いているので、最初に入れない部分の手前で閉じる
                // （append で right は空になるので、閉じるのは 1 回だけ）
                spans.append(&mut right);
                spans.extend(part);
            }
        }
        spans.append(&mut right);
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::MIN_MAX_LENGTH;
    use crate::view::fit::MIN_NAME_WIDTH;
    use crate::view::test_support::{agent, icons, option_combinations};

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
}
