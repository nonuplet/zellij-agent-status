//! 1 エージェント分の表示内容：何を表示するかを決め、Span 列を組み立てる。
//!
//! 1 エージェント分は `[アイコン 進捗 badge_items] text_items msg` の形（進捗と msg はあるときだけ）。
//! 要素（[`Element`]）は書いた順に並べ、値のない要素は前の空白や括弧も含めて省く。要素の間は空白 1 つ。
//! msg の前の区切りは、バッジの右端の直後なら空白 1 つ、それ以外は `: ` になる（[`BadgeExtent::msg_separator`]）。
//! [`ViewOptions::nerd_font_icon_padding`] が true なら、Nerd Font のアイコンの右隣に必ず空白を置く
//! （`[<icon>  <app>  work] msg`。理由は [`app_icon`] を参照）。
//! msg は [`ViewOptions::msg_mode`] で表示するエージェントにだけ持たせる。持たせないエージェントは
//! msg がないのと同じに扱うので、空いた幅はほかの要素に回る。
//!
//! 幅によらない固定の部分（アイコン・進捗・バッジの端・要素の前後の空白や括弧・区切り）は [`Item::new`] で決めておき、
//! 幅の計算（[`fit`](super::fit)）でもその幅を使う。組み立てと幅の計算で、同じ文字列を別々に数えないようにするため。
//!
//! どこまでをバッジにするかは [`badge`](super::badge)、幅に合わせた縮め方は [`fit`](super::fit) を参照。

use zellij_tile::graphemes::display_width;

use super::ViewOptions;
use super::badge::{BadgeExtent, BadgeStyle};
use super::icon::app_icon;
use super::options::Element;
use super::span::{Role, Span, width};
use super::text::{clean, dir_name, truncate};
use crate::model::{AgentState, AgentStatus};

/// 幅の割り当て（[`fit`](super::fit)）で、各エージェントの同じ Slot をまとめて縮める単位。
/// 1 エージェントに同じ Slot の要素は 1 つまで。並びは縮める順（[`Slot::ALL`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Slot {
    Msg,
    Cwd,
    AppName,
    TabName,
    Title,
    /// 各エージェントで最後に残る要素（[`Item::new`] を参照）。元の要素に関係なく、ほかの要素より後に縮める
    Last,
}

impl Slot {
    /// 縮める順
    pub(super) const ALL: [Slot; 6] = [
        Slot::Msg,
        Slot::Cwd,
        Slot::AppName,
        Slot::TabName,
        Slot::Title,
        Slot::Last,
    ];
}

/// 各 Slot を表示する幅の上限。None なら表示しない。
/// [`Slot::Last`] が None なら状態アイコン（と進捗）だけにする（`app` のアイコンも省く）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Widths([Option<usize>; Slot::ALL.len()]);

impl Widths {
    pub(super) fn get(&self, slot: Slot) -> Option<usize> {
        self.0[slot as usize]
    }

    pub(super) fn set(&mut self, slot: Slot, width: Option<usize>) {
        self.0[slot as usize] = width;
    }
}

/// 要素 1 つ分。前後の文字（前の空白、cwd の括弧、`app` のアイコンの後ろの空白）は、要素と一緒に表示し、一緒に省く
#[derive(Debug, Clone)]
struct Part {
    /// 縮める単位。None なら `app` のアイコン（切り詰めず、[`Slot::Last`] と一緒に表示し、一緒に省く）
    slot: Option<Slot>,
    before: &'static str,
    text: String,
    role: Role,
    after: &'static str,
}

impl Part {
    /// 前に空白 1 つを付けるだけの要素
    fn plain(slot: Slot, text: String) -> Self {
        Part {
            slot: Some(slot),
            before: " ",
            text,
            role: Role::Name,
            after: "",
        }
    }

    /// 縮めても変わらない部分の幅。`app` のアイコンは切り詰めないので、アイコン自体も含める
    fn fixed_width(&self) -> usize {
        let around = display_width(self.before) + display_width(self.after);
        match self.slot {
            Some(_) => around,
            None => around + display_width(&self.text),
        }
    }

    fn spans(&self, width: usize) -> impl Iterator<Item = Span> {
        [
            Span::new(self.before, Role::Plain),
            Span::new(truncate(&self.text, width), self.role),
            Span::new(self.after, Role::Plain),
        ]
        .into_iter()
        .filter(|s| !s.text.is_empty())
    }
}

/// 1 エージェント分の表示内容（切り詰める前）
pub(super) struct Item {
    pane_id: u32,
    state: AgentState,
    /// バッジに入れる範囲
    extent: BadgeExtent,
    /// バッジの左端と右端（内側の空白を含む）。バッジにしないなら空
    left: Vec<Span>,
    right: Vec<Span>,
    /// 状態アイコンと進捗（とアイコンの後ろの空白）
    head: Vec<Span>,
    /// [`ViewOptions::badge_items`] の要素のうち、値のあるもの
    badge: Vec<Part>,
    /// [`ViewOptions::text_items`] の要素のうち、値があり、`badge_items` にないもの
    text: Vec<Part>,
    /// [`ViewOptions::msg_mode`] で表示しないエージェントでは None
    msg: Option<String>,
    /// msg の前の区切り（[`BadgeExtent::msg_separator`]）
    msg_separator: Span,
}

impl Item {
    /// 要素を `badge_items`、`text_items` の順に並べる。両方にある要素は `badge_items` だけに表示する
    /// （既定の `badge_items` と重なることもあるので、config では警告しない）。
    ///
    /// エージェントを見分けられるよう、`title` / `tab_name` / `app_name` / `cwd` のうち値のある要素から、
    /// 縮める順で最も後ろのもの（`title` → `tab_name` → `app_name` → `cwd` の順に優先）を [`Slot::Last`] にし、
    /// ほかの要素を省いても残す。どれもなければ、保険として `#<pane_id>` を `badge_items` の末尾
    /// （`badge_items` が空なら `text_items` の末尾）に置く。
    /// `app` のアイコンはそれだけでは見分けられず、msg は出たり消えたりするので、判定に含めない。
    pub(super) fn new(agent: &AgentStatus, options: &ViewOptions, style: &BadgeStyle) -> Self {
        let part = |element: Element| -> Option<Part> {
            match element {
                Element::Title => agent
                    .title
                    .as_deref()
                    .and_then(clean)
                    .map(|text| Part::plain(Slot::Title, text)),
                Element::TabName => agent
                    .tab_name
                    .as_deref()
                    .and_then(clean)
                    .map(|text| Part::plain(Slot::TabName, text)),
                Element::AppName => agent
                    .app
                    .as_deref()
                    .and_then(clean)
                    .map(|text| Part::plain(Slot::AppName, text)),
                Element::AppIcon => agent.app.as_deref().and_then(app_icon).map(|icon| Part {
                    slot: None,
                    before: " ",
                    text: icon.to_owned(),
                    role: Role::App,
                    after: if style.pads_app_icon() { " " } else { "" },
                }),
                Element::Cwd => agent
                    .cwd
                    .as_deref()
                    .and_then(|cwd| clean(&dir_name(cwd)))
                    .map(|text| Part {
                        slot: Some(Slot::Cwd),
                        before: " (",
                        text,
                        role: Role::Cwd,
                        after: ")",
                    }),
            }
        };
        let mut badge: Vec<Part> = options
            .badge_items
            .iter()
            .filter_map(|&e| part(e))
            .collect();
        let mut text: Vec<Part> = options
            .text_items
            .iter()
            .filter(|e| !options.badge_items.contains(e))
            .filter_map(|&e| part(e))
            .collect();
        let last = badge
            .iter_mut()
            .chain(text.iter_mut())
            .filter(|p| p.slot.is_some())
            .max_by_key(|p| p.slot);
        match last {
            Some(part) => part.slot = Some(Slot::Last),
            None => {
                let pane = Part::plain(Slot::Last, format!("#{}", agent.pane_id));
                if options.badge_items.is_empty() {
                    text.push(pane);
                } else {
                    badge.push(pane);
                }
            }
        }

        let state = Role::State(agent.state);
        let mut head = vec![Span::new(style.icon(agent.state), state)];
        if let Some(progress) = agent.progress {
            head.push(Span::new(format!(" {progress}%"), state));
        }
        if style.pads_state_icon(agent.progress.is_some()) {
            head.push(Span::new(" ", state));
        }

        let (left, right) = style
            .shape()
            .map_or_else(Default::default, |s| s.spans(agent.state));
        let extent = style.extent(agent.state);
        // msg を表示するのは、ほかの要素をどれも省いていないときだけ（fit を参照）なので、
        // text_items の要素があるかどうかは、幅によらずここで決まる
        let msg_separator = Span::new(extent.msg_separator(!text.is_empty()), Role::Plain);
        Item {
            pane_id: agent.pane_id,
            state: agent.state,
            extent,
            left,
            right,
            head,
            badge,
            text,
            msg: agent
                .msg
                .as_deref()
                .and_then(clean)
                .filter(|_| options.msg_mode.shows(agent.state)),
            msg_separator,
        }
    }

    fn parts(&self) -> impl Iterator<Item = &Part> {
        self.badge.iter().chain(&self.text)
    }

    /// 各 Slot を切り詰めずに表示するときの幅
    pub(super) fn widths(&self) -> Widths {
        let mut widths = Widths::default();
        for part in self.parts() {
            if let Some(slot) = part.slot {
                widths.set(slot, Some(display_width(&part.text)));
            }
        }
        widths.set(Slot::Msg, self.msg.as_deref().map(display_width));
        widths
    }

    /// アイコンと進捗、バッジの両端とその内側の空白の幅（常に表示する部分）。
    /// 要素を入れたり強調したりするとバッジの右端が後ろに移るが、幅は同じなのでここに含める。
    pub(super) fn head_width(&self) -> usize {
        width(&self.left) + width(&self.head) + width(&self.right)
    }

    /// `slot` に付く部分（要素の前後の空白や括弧、msg の前の区切り）の幅。`slot` の要素と一緒に表示し、一緒に省く。
    /// [`Slot::Last`] には、一緒に表示する `app` のアイコンの幅も含める。
    pub(super) fn overhead(&self, slot: Slot) -> usize {
        match slot {
            Slot::Msg => display_width(&self.msg_separator.text),
            Slot::Last => self
                .parts()
                .filter(|p| matches!(p.slot, None | Some(Slot::Last)))
                .map(Part::fixed_width)
                .sum(),
            _ => self
                .parts()
                .filter(|p| p.slot == Some(slot))
                .map(Part::fixed_width)
                .sum(),
        }
    }

    /// 3 つの部分（[`BadgeExtent`] を参照）に分け、先頭から [`Item::extent`] の部分までをバッジに入れる（塗る）。
    /// バッジの右端（と内側の空白）は、バッジに入れた部分の直後に置く。
    /// 要素を省いたときはその部分が空になるので、右端は前の部分の直後に来る。
    /// バッジにしないときは、選んでいるときだけ塗る部分に印を付ける（[`BadgeExtent::fill`]。端の文字はない）。
    ///
    /// どの Span にもこのエージェントのペインを持たせ、1 エージェント分のどこをクリックしてもフォーカスできるようにする。
    pub(super) fn spans(&self, widths: Widths) -> Vec<Span> {
        let shown = |part: &Part| match part.slot {
            Some(slot) => widths.get(slot),
            None => widths.get(Slot::Last).map(|_| display_width(&part.text)),
        };
        let label = |parts: &[Part]| -> Vec<Span> {
            parts
                .iter()
                .filter_map(|p| shown(p).map(|width| p.spans(width)))
                .flatten()
                .collect()
        };
        let mut head = self.head.clone();
        head.extend(label(&self.badge));
        let mut msg = vec![];
        if let (Some(text), Some(width)) = (&self.msg, widths.get(Slot::Msg)) {
            msg.push(self.msg_separator.clone());
            msg.push(Span::new(truncate(text, width), Role::Msg));
        }

        let (filled, fill) = self.extent.fill(self.state);
        let mut parts = [head, label(&self.text), msg].into_iter();
        let mut spans = self.left.clone();
        spans.extend(
            parts
                .by_ref()
                .take(filled)
                .flatten()
                .map(|s| s.with_fill(fill)),
        );
        spans.extend_from_slice(&self.right);
        spans.extend(parts.flatten());
        spans
            .into_iter()
            .map(|s| s.with_pane(Some(self.pane_id)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::MIN_MAX_AGENT_WIDTH;
    use crate::view::badge::BadgeShape;
    use crate::view::fit::MIN_NAME_WIDTH;
    use crate::view::icon::StateIconStyle;
    use crate::view::test_support::{agent, option_combinations, state_icons};

    fn item(agent: &AgentStatus, options: &ViewOptions) -> Item {
        Item::new(agent, options, &BadgeStyle::new(options))
    }

    /// 最後に残る要素を [`MIN_NAME_WIDTH`] まで縮めたときの幅
    fn min_width(item: &Item) -> usize {
        item.head_width() + item.overhead(Slot::Last) + MIN_NAME_WIDTH
    }

    #[test]
    fn min_max_agent_width_fits_widest_head_and_min_last_element() {
        // 最後に残る要素が cwd になるエージェントと、ほかの要素になるエージェント
        let mut only_cwd = agent(1, AgentState::Blocked);
        only_cwd.title = None;
        only_cwd.app = Some("claude-code".to_owned());
        only_cwd.cwd = Some("/repo/main".into());
        only_cwd.progress = Some(u8::MAX);
        let mut titled = only_cwd.clone();
        titled.title = Some("title".to_owned());
        let widths: Vec<usize> = option_combinations()
            .iter()
            .flat_map(|options| [&only_cwd, &titled].map(|a| min_width(&item(a, options))))
            .collect();
        // 最も広いのは、絵文字以外の状態アイコン（バッジの端あり）で、バッジの端を Nerd Font の文字にし、
        // app のアイコンとその後ろの空白を付け、cwd（括弧の分だけ広い）が最後に残るとき
        assert_eq!(widths.iter().max(), Some(&MIN_MAX_AGENT_WIDTH));
        let options = ViewOptions {
            badge_shape: BadgeShape::Slash,
            badge_items: vec![Element::AppIcon, Element::Cwd],
            nerd_font_icon_padding: true,
            ..state_icons(StateIconStyle::NerdFont)
        };
        let item = item(&only_cwd, &options);
        assert!(item.badge.iter().any(|p| p.role == Role::App));
        assert!(
            item.badge
                .iter()
                .any(|p| p.role == Role::Cwd && p.slot == Some(Slot::Last))
        );
        assert_eq!(min_width(&item), MIN_MAX_AGENT_WIDTH);
    }

    fn slots(item: &Item) -> (Vec<Option<Slot>>, Vec<Option<Slot>>) {
        let slots = |parts: &[Part]| parts.iter().map(|p| p.slot).collect();
        (slots(&item.badge), slots(&item.text))
    }

    #[test]
    fn last_element_is_chosen_by_shrinking_order() {
        use Element::*;
        let mut a = agent(1, AgentState::Idle);
        a.tab_name = Some("work".to_owned());
        a.cwd = Some("/repo".into());
        let options = |badge_items: Vec<Element>, text_items: Vec<Element>| ViewOptions {
            badge_items,
            text_items,
            ..ViewOptions::default()
        };
        // 並び順やバッジの中か外かに関係なく、title → tab_name → app_name → cwd の順に優先する
        assert_eq!(
            slots(&item(
                &a,
                &options(vec![Cwd, AppName], vec![TabName, Title])
            )),
            (
                vec![Some(Slot::Cwd), Some(Slot::AppName)],
                vec![Some(Slot::TabName), Some(Slot::Last)]
            )
        );
        a.title = None;
        assert_eq!(
            slots(&item(
                &a,
                &options(vec![Cwd, AppName], vec![TabName, Title])
            )),
            (
                vec![Some(Slot::Cwd), Some(Slot::AppName)],
                vec![Some(Slot::Last)]
            )
        );
        // app のアイコンは最後に残る要素にしない
        a.app = Some("claude-code".to_owned());
        assert_eq!(
            slots(&item(&a, &options(vec![AppIcon, Cwd], vec![]))),
            (vec![None, Some(Slot::Last)], vec![])
        );
        // どれもなければ #<pane_id>
        a.cwd = None;
        assert_eq!(
            slots(&item(&a, &options(vec![AppIcon, Cwd], vec![]))),
            (vec![None, Some(Slot::Last)], vec![])
        );
        assert_eq!(
            item(&a, &options(vec![AppIcon, Cwd], vec![])).badge[1].text,
            "#1"
        );
    }
}
