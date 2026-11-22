//! 描画の中間表現：[`layout`](mod@super::layout) が作り、[`paint`](mod@super::paint) が色を付ける Span。

use unicode_width::UnicodeWidthStr;

use crate::model::AgentState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// 余白や区切り
    Plain,
    /// 名前の前の `app` のアイコン（Nerd Font モード）
    App,
    /// 状態アイコンと進捗。状態の色で塗る。
    State(AgentState),
    /// バッジの端の文字（[`BadgeEnds`](super::badge::BadgeEnds)）。状態の色で塗る（太字にしない）。
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
    /// どのエージェントの部分か（エージェントのペインの id）。クリックしたときにフォーカスするペインになる。
    /// 行頭の余白や区切りなど、どのエージェントにも属さない部分は None
    pub(super) pane_id: Option<u32>,
}

impl Span {
    pub(super) fn new(text: impl Into<String>, role: Role) -> Self {
        Span {
            text: text.into(),
            role,
            fill: Fill::None,
            pane_id: None,
        }
    }

    pub(super) fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = fill;
        self
    }

    pub(super) fn with_pane(mut self, pane_id: Option<u32>) -> Self {
        self.pane_id = pane_id;
        self
    }
}

/// Span 列の表示幅
pub(super) fn width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.text.width()).sum()
}
