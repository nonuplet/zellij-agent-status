//! 表示：エージェントの一覧と幅から、1 行分の文字列（ANSI エスケープ付き）を作る。
//!
//! 副作用のない関数だけで構成し、`render` からは [`render_line`] の結果を出力するだけにする。
//! 表示するエージェントの絞り込みと並び順も、表示の都合としてここで決める（Collector は順序を持たない）。
//!
//! - [`layout`](mod@layout)：どのエージェントを、どの順で、どの文字列をどの役割で並べるか（幅に合わせた切り詰めを含む）
//! - [`paint`](mod@paint)：色付け
//! - [`theme`] / [`icon`]：色とアイコンの定義

mod icon;
mod layout;
mod paint;
mod theme;

pub use icon::IconStyle;
pub use theme::Theme;

use crate::model::AgentStatus;
use layout::{PAD, Role, Span, clamp, clean, layout};
use paint::paint;

/// 表示の設定。`load` の configuration から作る（`src/config.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewOptions {
    pub icons: IconStyle,
    /// 表示する `app`。None なら絞り込まない
    pub apps: Option<Vec<String>>,
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
    use crate::model::AgentState;
    use unicode_width::UnicodeWidthStr;

    pub(crate) fn agent(pane_id: u32, state: AgentState) -> AgentStatus {
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

    pub(crate) fn with_msg(mut a: AgentStatus, msg: &str) -> AgentStatus {
        a.msg = Some(msg.to_owned());
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
