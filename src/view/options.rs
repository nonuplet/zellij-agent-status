//! 表示の設定。

use super::badge::BadgeShape;
use super::icon::IconStyle;
use crate::model::{AgentState, AgentStatus};

/// 表示の設定。`load` の configuration から作る（`src/config.rs`）。
/// 各設定のユーザ向けの説明は README の「設定」にあり、ここには実装上の意味だけを書く。
/// バッジ・アイコンに関わる設定は互いに依存するので、view では直接読まず、
/// [`BadgeStyle`](super::badge::BadgeStyle) で組み合わせを解決してから使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewOptions {
    /// 状態アイコンの種類。`nerd_font` による既定値の切り替えは config で済ませてある
    pub icons: IconStyle,
    /// 表示する `app`。None なら絞り込まない
    pub apps: Option<Vec<String>>,
    /// `app` を名前の候補にするか（名前の決まり方は `src/view/item.rs` の `Item::new` を参照）。
    /// 絞り込み（[`ViewOptions::shows`]）には影響しない。
    pub show_app_name: bool,
    /// タブ名を名前の候補にするか。並び順と絞り込みには影響しない。
    pub show_tab_name: bool,
    /// `Blocked` のエージェントを先頭に並べるか（[`layout()`](super::layout::layout) の並べ替え）
    pub blocked_first: bool,
    /// 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。None なら上限なし。
    /// [`MIN_MAX_LENGTH`](super::MIN_MAX_LENGTH) 以上であること（config で検証する）。
    pub max_length: Option<usize>,
    /// 名前の後ろにカレントディレクトリ（末尾のディレクトリ名）を表示するか
    pub show_cwd: bool,
    /// Nerd Font モード。バッジの端を [`ViewOptions::badge_shape`] の形にし、名前の前に
    /// `app` のアイコンを付ける。状態アイコン（[`ViewOptions::icons`]）は config 側で切り替える。
    pub nerd_font: bool,
    /// Nerd Font モードのバッジの端の形。`nerd_font` が false なら使わない
    pub badge_shape: BadgeShape,
    /// Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか。
    /// グリフが 1 桁からはみ出すかどうかはプログラムからは判別できないため、設定にしている。
    pub nerd_font_icon_padding: bool,
    /// 注意が必要な状態（[`AgentState::needs_attention`]）のエージェントを、1 つ分全体を状態の色で塗って強調するか
    pub highlight_attention: bool,
    /// 状態バッジの中に、名前（と `app` のアイコン・cwd）も入れるか。絵文字アイコン（バッジにしない）では使わない。
    /// 既定で入れる理由は `src/view/badge.rs` を参照
    pub badge_name: bool,
    /// どのエージェントで msg を表示するか
    pub msg_mode: MsgMode,
}

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

/// [`ViewOptions::max_length`] の既定値
pub const DEFAULT_MAX_LENGTH: usize = 50;

// show_app_name などの既定値を指定するため、derive せずに書く
impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions {
            icons: IconStyle::default(),
            apps: None,
            show_app_name: true,
            show_tab_name: true,
            blocked_first: false,
            max_length: Some(DEFAULT_MAX_LENGTH),
            show_cwd: false,
            nerd_font: false,
            badge_shape: BadgeShape::default(),
            nerd_font_icon_padding: false,
            highlight_attention: false,
            badge_name: true,
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

    /// `app` が絞り込みの条件に合うか。`apps` を指定しているときは、`app` のないレコードは表示しない。
    fn accepts_app(&self, app: Option<&str>) -> bool {
        match (&self.apps, app) {
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
            apps: Some(vec!["claude".to_owned()]),
            ..Default::default()
        };
        assert!(options.accepts_app(Some("claude")));
        assert!(!options.accepts_app(Some("codex")));
        assert!(!options.accepts_app(Some("Claude")));
        assert!(!options.accepts_app(None));
    }
}
