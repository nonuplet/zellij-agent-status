//! 表示：エージェントの一覧と幅から、1 行分の文字列（ANSI エスケープ付き）を作る。
//!
//! 副作用のない関数だけで構成し、`render` からは [`render_line`] の結果を出力するだけにする。
//! 表示するエージェントの絞り込みと並び順も、表示の都合としてここで決める（Collector は順序を持たない）。
//!
//! - [`layout`](mod@layout)：どのエージェントを、どの順で、どの文字列をどの役割で並べるか（幅に合わせた切り詰めを含む）
//! - [`paint`](mod@paint)：色付け
//! - [`theme`] / [`icon`]：色・アイコンの定義

mod icon;
mod layout;
mod paint;
mod theme;

pub use icon::IconStyle;
pub use layout::{BadgeShape, MIN_MAX_LENGTH};
pub use theme::Theme;

use crate::model::{AgentState, AgentStatus};
use layout::{PAD, Role, Span, clamp, clean, layout};
use paint::paint;

/// 表示の設定。`load` の configuration から作る（`src/config.rs`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewOptions {
    pub icons: IconStyle,
    /// 表示する `app`。None なら絞り込まない
    pub apps: Option<Vec<String>>,
    /// `app` を名前の候補にするか。false なら名前は `title` → タブ名 → `#<pane_id>` の順で決める。
    /// 絞り込み（[`ViewOptions::shows`]）には影響しない。
    pub show_app: bool,
    /// `Blocked` のエージェントを先頭に並べるか（[`layout()`] の並べ替え）
    pub blocked_first: bool,
    /// 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。None なら上限なし。
    /// [`MIN_MAX_LENGTH`] 以上であること（config で検証する）。
    pub max_length: Option<usize>,
    /// 名前の後ろにカレントディレクトリ（末尾のディレクトリ名）を表示するか
    pub show_cwd: bool,
    /// Nerd Font モード。バッジの端を [`ViewOptions::badge_shape`] の形にし、名前の前に
    /// `app` のアイコンを付ける。状態アイコン（[`ViewOptions::icons`]）は config 側で切り替える。
    pub nerd_font: bool,
    /// Nerd Font モードのバッジの端の形。`nerd_font` が false なら使わない
    pub badge_shape: BadgeShape,
    /// Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか。
    /// グリフが 1 桁からはみ出すかどうかはフォントと端末の組み合わせで変わり、プログラムからは判別できないため設定にしている。
    pub nerd_font_icon_padding: bool,
    /// 注意が必要な状態（`Blocked` / `Error`）のエージェントを、1 つ分全体を状態の色で塗って強調するか
    pub highlight_attention: bool,
    /// 状態バッジの中に、名前（と `app` のアイコン・cwd）も入れるか。絵文字アイコン（バッジにしない）では使わない。
    /// アイコンがバッジの端で切れたり重なったりしないよう、既定で入れる（`src/view/layout.rs` を参照）
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
    fn shows(self, state: AgentState) -> bool {
        match self {
            MsgMode::None => false,
            MsgMode::Attention => state.needs_attention(),
            MsgMode::All => true,
        }
    }
}

/// [`ViewOptions::max_length`] の既定値
pub const DEFAULT_MAX_LENGTH: usize = 50;

// show_app などの既定値を指定するため、derive せずに書く
impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions {
            icons: IconStyle::default(),
            apps: None,
            show_app: true,
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
    /// （[`layout()`] と、一覧が空かどうかを見る `main.rs` の両方が使う）。
    pub fn shows(&self, agent: &AgentStatus) -> bool {
        self.accepts_app(agent.app.as_deref())
    }

    /// `app` が絞り込みの条件に合うか。`apps` を指定しているときは、`app` のないレコードは表示しない。
    pub fn accepts_app(&self, app: Option<&str>) -> bool {
        match (&self.apps, app) {
            (None, _) => true,
            (Some(apps), Some(app)) => apps.iter().any(|a| a == app),
            (Some(_), None) => false,
        }
    }
}

/// 幅 `cols` の 1 行を作る。末尾は背景色の空白で埋める。
/// `agents` の順序は問わない（並び順は [`layout()`] で決める）。
pub fn render_line<'a>(
    agents: impl IntoIterator<Item = &'a AgentStatus>,
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

#[cfg(test)]
pub(crate) mod tests {
    use super::theme::Color;
    use super::*;
    use unicode_width::UnicodeWidthStr;

    pub(crate) fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
        AgentStatus {
            pane_id,
            tab: 0,
            tab_name: None,
            cwd: None,
            state,
            kind: None,
            progress: None,
            app: Some("claude".to_owned()),
            title: None,
            msg: None,
            updated_ms: 0,
        }
    }

    pub(crate) fn with_msg(mut a: AgentStatus, msg: &str) -> AgentStatus {
        a.msg = Some(msg.to_owned());
        a
    }

    pub(crate) fn with_cwd(mut a: AgentStatus, cwd: &str) -> AgentStatus {
        a.cwd = Some(cwd.into());
        a
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

    fn test_theme() -> Theme {
        Theme {
            fg: Color::Indexed(7),
            bg: Color::Rgb(1, 2, 3),
            idle: Color::Indexed(8),
            working: Color::Indexed(6),
            blocked: Color::Indexed(3),
            done: Color::Indexed(2),
            error: Color::Indexed(1),
        }
    }

    #[test]
    fn paint_colors_spans_and_fills_width() {
        let theme = test_theme();
        let a = [agent(1, AgentState::Error)];
        let line = render_line(&a, 20, &theme, &ViewOptions::default());
        // バッジは状態の色の背景に、行の背景色の文字（反転）。端の空白は太字にしない
        assert!(
            line.contains("\x1b[0;38;2;1;2;3;48;5;1m \x1b[0;1;38;2;1;2;3;48;5;1m✗"),
            "{line:?}"
        );
        // 名前もバッジの中なので反転させる（太字のまま）。その後ろが右端の空白
        assert!(
            line.contains("\x1b[0;1;38;2;1;2;3;48;5;1mclaude\x1b[0;38;2;1;2;3;48;5;1m "),
            "{line:?}"
        );
        assert!(line.ends_with("\x1b[0m"));
        let visible = strip_ansi(&line);
        assert_eq!(visible.width(), 20);
        assert_eq!(visible.trim_end(), "  ✗ claude");

        // 名前をバッジに入れなければ、アイコンの直後が右端の空白で、名前は前景色のまま
        let without = ViewOptions {
            badge_name: false,
            ..Default::default()
        };
        let line = render_line(&a, 20, &theme, &without);
        assert!(
            line.contains("\x1b[0;1;38;2;1;2;3;48;5;1m✗\x1b[0;38;2;1;2;3;48;5;1m "),
            "{line:?}"
        );
        assert!(
            line.contains("\x1b[0;1;38;5;7;48;2;1;2;3mclaude"),
            "{line:?}"
        );
        assert_eq!(strip_ansi(&line).trim_end(), "  ✗  claude");

        // Nerd Font モードの app のアイコンは太字にしない。バッジの中では反転させ、外では前景色
        let mut a = agent(1, AgentState::Error);
        a.app = Some("claude-code".to_owned());
        let nerd = ViewOptions {
            nerd_font: true,
            ..Default::default()
        };
        let line = render_line(&[a.clone()], 20, &theme, &nerd);
        assert!(
            line.contains("\x1b[0;38;2;1;2;3;48;5;1m\u{ec82}"),
            "{line:?}"
        );
        let line_without = render_line(
            &[a],
            20,
            &theme,
            &ViewOptions {
                badge_name: false,
                ..nerd.clone()
            },
        );
        assert!(
            line_without.contains("\x1b[0;38;5;7;48;2;1;2;3m\u{ec82}"),
            "{line_without:?}"
        );
        // バッジの端の文字（既定の slash）は、行の背景の上に状態の色の前景色で描き、太字にしない
        assert!(
            line.contains("\x1b[0;38;5;1;48;2;1;2;3m\u{e0ba}"),
            "{line:?}"
        );
        assert!(
            line.contains("\x1b[0;38;5;1;48;2;1;2;3m\u{e0bc}"),
            "{line:?}"
        );
    }

    #[test]
    fn paint_inverts_highlighted_agent_and_skips_emoji_badge() {
        let theme = test_theme();
        let a = [with_msg(agent(1, AgentState::Blocked), "Allow?")];
        let highlight = ViewOptions {
            highlight_attention: true,
            ..Default::default()
        };
        let line = render_line(&a, 30, &theme, &highlight);
        // 名前・msg も状態の色の背景に、行の背景色の文字で描く
        assert!(
            line.contains("\x1b[0;1;38;2;1;2;3;48;5;3mclaude"),
            "{line:?}"
        );
        assert!(line.contains("\x1b[0;38;2;1;2;3;48;5;3mAllow?"), "{line:?}");

        // 絵文字アイコンはバッジにしない（今までどおり前景色だけ）
        let emoji = ViewOptions {
            icons: IconStyle::Emoji,
            ..Default::default()
        };
        let line = render_line(&a, 30, &theme, &emoji);
        assert!(line.contains("\x1b[0;1;38;5;3;48;2;1;2;3m❗"), "{line:?}");
        assert!(!line.contains("48;5;3"), "{line:?}");
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
