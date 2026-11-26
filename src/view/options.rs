//! 表示の設定。

use super::badge::BadgeShape;
use super::icon::StateIconStyle;
use crate::model::{AgentState, AgentStatus};

/// 表示の設定。`load` の configuration から作る（`src/config.rs`）。
/// 各設定のユーザ向けの説明は README の「設定」にあり、ここには実装上の意味だけを書く。
/// バッジ・アイコンに関わる設定は互いに依存するので、view では直接読まず、
/// [`BadgeStyle`](super::badge::BadgeStyle) で組み合わせを解決してから使う。
///
/// 設定の `nerd_font` は、[`ViewOptions::state_icon_style`] / [`ViewOptions::badge_shape`] / [`ViewOptions::badge_items`] の
/// 既定値を切り替えるだけなので、ここには持たない（切り替えは config で済ませる）。
/// [`ViewOptions::default`] は `nerd_font` が false のときの既定値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewOptions {
    /// 状態アイコンの種類
    pub state_icon_style: StateIconStyle,
    /// 表示する `app`。None なら絞り込まない
    pub app_filter: Option<Vec<String>>,
    /// `Blocked` のエージェントを先頭に並べるか（[`layout()`](super::layout::layout) の並べ替え）
    pub blocked_first: bool,
    /// 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。None なら上限なし。
    /// [`MIN_MAX_AGENT_WIDTH`](super::MIN_MAX_AGENT_WIDTH) 以上であること（config で検証する）。
    pub max_agent_width: Option<usize>,
    /// 状態バッジの中（状態アイコンと進捗の後ろ）に、この順に表示する要素。
    /// 絵文字アイコン（バッジにしない）では、状態アイコンと進捗の後ろにそのまま続ける。
    /// 同じ要素を 2 回含まないこと（config で検証する）
    pub badge_items: Vec<Element>,
    /// 状態バッジの外（バッジの後ろ、msg の前）に、この順に表示する要素。
    /// [`ViewOptions::badge_items`] にもある要素は表示しない（`src/view/item.rs` の `Item::new` を参照）
    pub text_items: Vec<Element>,
    /// バッジの端の形。絵文字アイコン（バッジにしない）では使わない
    pub badge_shape: BadgeShape,
    /// Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか。
    /// 設定にしている理由は [`app_icon`](super::icon::app_icon) を参照
    pub nerd_font_icon_padding: bool,
    /// 注意が必要な状態（[`AgentState::needs_attention`]）のエージェントを、1 つ分全体を状態の色で塗って強調するか
    pub highlight_attention: bool,
    /// どのエージェントで msg を表示するか
    pub msg_mode: MsgMode,
}

/// [`ViewOptions::badge_items`] / [`ViewOptions::text_items`] に並べる要素。
/// 値のない要素（`title` を送らないエージェントの [`Element::Title`] など）は表示しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// エージェントが送る `title`
    Title,
    /// ペインがあるタブの名前
    TabName,
    /// `app`（アプリの名前）
    AppName,
    /// `app` のアイコン（対応するものがあるときだけ。[`app_icon`](super::icon::app_icon) を参照）
    AppIcon,
    /// カレントディレクトリの末尾のディレクトリ名。括弧で囲む
    Cwd,
}

/// [`ViewOptions::badge_items`] の既定値（`nerd_font` が false のとき）。
/// `title` を送らない Claude Code でも、タブ名で複数のエージェントを見分けられるようにしている
const DEFAULT_BADGE_ITEMS: &[Element] = &[Element::Title, Element::TabName];

/// msg を表示するエージェントの範囲（[`ViewOptions::msg_mode`]）。
/// 注意が必要な状態の判定は [`AgentState::needs_attention`] で、[`ViewOptions::highlight_attention`] と同じ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MsgMode {
    /// どのエージェントでも表示しない
    None,
    /// 注意が必要な状態（`Blocked` / `Error`）のエージェントだけ表示する。
    /// エージェントが多いと行が msg で埋まり、操作待ちの理由などが切り詰められたり埋もれたりするため、これを既定にしている
    #[default]
    Attention,
    /// すべてのエージェントで表示する
    All,
}

impl MsgMode {
    /// 状態が `state` のエージェントで msg を表示するか
    pub(super) fn shows(self, state: AgentState) -> bool {
        match self {
            MsgMode::None => false,
            MsgMode::Attention => state.needs_attention(),
            MsgMode::All => true,
        }
    }
}

/// [`ViewOptions::max_agent_width`] の既定値
pub const DEFAULT_MAX_AGENT_WIDTH: usize = 50;

// max_agent_width などの既定値を指定するため、derive せずに書く
impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions {
            state_icon_style: StateIconStyle::default(),
            app_filter: None,
            blocked_first: false,
            max_agent_width: Some(DEFAULT_MAX_AGENT_WIDTH),
            badge_items: DEFAULT_BADGE_ITEMS.to_vec(),
            text_items: vec![],
            badge_shape: BadgeShape::default(),
            nerd_font_icon_padding: false,
            highlight_attention: false,
            msg_mode: MsgMode::default(),
        }
    }
}

impl ViewOptions {
    /// このエージェントを表示するか。表示するエージェントの条件はここに集める
    /// （[`layout()`](super::layout::layout) と、一覧が空かどうかを見る `main.rs` の両方が使う）。
    pub fn shows(&self, agent: &AgentStatus) -> bool {
        self.accepts_app(agent.app.as_deref())
    }

    /// `app` が絞り込みの条件に合うか。`app_filter` を指定しているときは、`app` のないレコードは表示しない。
    fn accepts_app(&self, app: Option<&str>) -> bool {
        match (&self.app_filter, app) {
            (None, _) => true,
            (Some(apps), Some(app)) => apps.iter().any(|a| a == app),
            (Some(_), None) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_app_without_filter() {
        let options = ViewOptions::default();
        assert!(options.accepts_app(Some("claude")));
        assert!(options.accepts_app(None));
    }

    #[test]
    fn accepts_app_with_filter() {
        let options = ViewOptions {
            app_filter: Some(vec!["claude".to_owned()]),
            ..Default::default()
        };
        assert!(options.accepts_app(Some("claude")));
        assert!(!options.accepts_app(Some("codex")));
        assert!(!options.accepts_app(Some("Claude")));
        assert!(!options.accepts_app(None));
    }
}
