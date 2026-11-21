//! プラグイン設定（`load` に渡される configuration）の読み込み。
//!
//! Zellij はレイアウトの plugin ブロックの子ノードを、`名前 → 最初の値の文字列` のフラットな
//! マップにして渡す（真偽値や数値も文字列になる）。そのため、設定はすべて文字列のキーと値で表す。
//!
//! ```kdl
//! plugin location="file:zellij-agent-status.wasm" {
//!     icon_style "alphabet"
//!     apps "claude-code, codex"
//!     show_app "false"
//!     blocked_first "true"
//!     max_length 40
//!     show_cwd "true"
//!     nerd_font "true"
//!     badge_shape "round"
//!     nerd_font_icon_padding "true"
//!     highlight_attention "true"
//! }
//! ```
//!
//! | キー | 既定値 | 内容 |
//! |---|---|---|
//! | `icon_style` | `symbol`（`nerd_font` が true なら `nerd`） | アイコンを記号（`symbol`）、アルファベット 1 文字（`alphabet`）、絵文字（`emoji`）、Nerd Font（`nerd`）のどれにするか |
//! | `apps` | （空） | 表示する `app` の一覧。カンマか空白で区切る。空なら絞り込まない |
//! | `show_app` | `true` | `app` を名前の候補にするか。名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決まり、`false` なら `app` を飛ばす |
//! | `blocked_first` | `false` | 操作待ち（`Blocked`）のエージェントを先頭に並べるか。`false` なら (タブ, ペイン) の順のまま |
//! | `max_length` | `50` | 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。[`MIN_MAX_LENGTH`] 以上の整数。`0` なら上限なし |
//! | `show_cwd` | `false` | 名前の後ろに、ペインのカレントディレクトリの末尾のディレクトリ名を `名前 (dir)` の形で表示するか |
//! | `nerd_font` | `false` | Nerd Font モード。バッジの端を `badge_shape` の形にし、`app` のアイコンを付け、`icon_style` の既定値を `nerd` にする |
//! | `badge_shape` | `slash` | Nerd Font モードのバッジの端の形（`slash` / `arrow` / `round`）。`nerd_font` が false なら使わない |
//! | `nerd_font_icon_padding` | `false` | Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか。アイコンの右が切れたり隣と重なったりするフォント向け |
//! | `highlight_attention` | `false` | `Blocked` / `Error` のエージェントを、1 つ分全体を状態の色で塗って強調するか |
//!
//! `nerd_font` と `icon_style` を両方指定したときは、状態アイコンは `icon_style` に従う
//! （Nerd Font モードで状態アイコンだけ絵文字にする、などができる）。
//!
//! 真偽値は `true` / `false` だけを受け付ける。KDL の真偽値（`show_app false`）も
//! 文字列（`show_app "false"`）も、Zellij からは同じ文字列で届く。
//!
//! 表示の並びは `blocked_first` 以外では変えられない。切り詰め方（どの順に縮めるか）は view（`src/view/layout.rs`）で固定している。
//!
//! 不正な値は、そのキーだけ既定値に戻して警告を返す。設定の誤りでプラグイン全体が動かなくなるのを避けるため。
//! 未知のキーは無視する（Zellij 側が予約キー以外を渡してくる可能性を排除できていないため、警告もしない）。

use std::collections::BTreeMap;

use crate::view::{BadgeShape, DEFAULT_MAX_LENGTH, IconStyle, MIN_MAX_LENGTH, ViewOptions};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub view: ViewOptions,
}

impl Config {
    /// configuration から設定を作る。不正な値はそのキーだけ既定値にし、警告メッセージとして返す。
    pub fn from_configuration(configuration: &BTreeMap<String, String>) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();

        // icon_style の既定値が変わるので、icon_style より先に読む
        if let Some(value) = configuration.get("nerd_font") {
            match parse_bool(value) {
                Some(nerd_font) => config.view.nerd_font = nerd_font,
                None => warnings.push(format!(
                    "nerd_font: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }
        if config.view.nerd_font {
            config.view.icons = IconStyle::Nerd;
        }

        if let Some(value) = configuration.get("icon_style") {
            match parse_icon_style(value) {
                Some(icons) => config.view.icons = icons,
                None => warnings.push(format!(
                    "icon_style: `{value}` は使えません。`symbol`・`alphabet`・`emoji`・`nerd` のどれかを指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("badge_shape") {
            match parse_badge_shape(value) {
                Some(shape) => config.view.badge_shape = shape,
                None => warnings.push(format!(
                    "badge_shape: `{value}` は使えません。`slash`・`arrow`・`round` のどれかを指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("apps") {
            config.view.apps = parse_apps(value);
        }

        if let Some(value) = configuration.get("show_app") {
            match parse_bool(value) {
                Some(show_app) => config.view.show_app = show_app,
                None => warnings.push(format!(
                    "show_app: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("blocked_first") {
            match parse_bool(value) {
                Some(blocked_first) => config.view.blocked_first = blocked_first,
                None => warnings.push(format!(
                    "blocked_first: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("max_length") {
            match parse_max_length(value) {
                Some(max_length) => config.view.max_length = max_length,
                None => warnings.push(format!(
                    "max_length: `{value}` は使えません。0（上限なし）か {MIN_MAX_LENGTH} 以上の整数を指定してください（既定値 {DEFAULT_MAX_LENGTH} を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("show_cwd") {
            match parse_bool(value) {
                Some(show_cwd) => config.view.show_cwd = show_cwd,
                None => warnings.push(format!(
                    "show_cwd: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("highlight_attention") {
            match parse_bool(value) {
                Some(highlight) => config.view.highlight_attention = highlight,
                None => warnings.push(format!(
                    "highlight_attention: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("nerd_font_icon_padding") {
            match parse_bool(value) {
                Some(padding) => config.view.nerd_font_icon_padding = padding,
                None => warnings.push(format!(
                    "nerd_font_icon_padding: `{value}` は使えません。`true` か `false` を指定してください（既定値を使います）"
                )),
            }
        }

        (config, warnings)
    }
}

/// 0 は上限なし（`Some(None)`）。不正な値は None。
/// 1 以上 [`MIN_MAX_LENGTH`] 未満は、アイコンと進捗だけで上限を超えることがあるので受け付けない。
fn parse_max_length(value: &str) -> Option<Option<usize>> {
    match value.parse().ok()? {
        0 => Some(None),
        max_length if max_length >= MIN_MAX_LENGTH => Some(Some(max_length)),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_icon_style(value: &str) -> Option<IconStyle> {
    match value {
        "symbol" => Some(IconStyle::Symbol),
        "alphabet" => Some(IconStyle::Alphabet),
        "emoji" => Some(IconStyle::Emoji),
        "nerd" => Some(IconStyle::Nerd),
        _ => None,
    }
}

fn parse_badge_shape(value: &str) -> Option<BadgeShape> {
    match value {
        "slash" => Some(BadgeShape::Slash),
        "arrow" => Some(BadgeShape::Arrow),
        "round" => Some(BadgeShape::Round),
        _ => None,
    }
}

/// カンマか空白で区切った一覧。空なら None（絞り込まない）。
fn parse_apps(value: &str) -> Option<Vec<String>> {
    let apps: Vec<String> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (!apps.is_empty()).then_some(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn empty_configuration_gives_defaults() {
        let (config, warnings) = Config::from_configuration(&BTreeMap::new());
        assert!(warnings.is_empty());
        assert_eq!(config.view.icons, IconStyle::Symbol);
        assert_eq!(config.view.apps, None);
        assert!(config.view.show_app);
        assert!(!config.view.blocked_first);
        assert_eq!(config.view.max_length, Some(DEFAULT_MAX_LENGTH));
        assert!(!config.view.show_cwd);
        assert!(!config.view.nerd_font);
        assert_eq!(config.view.badge_shape, BadgeShape::Slash);
        assert!(!config.view.highlight_attention);
        assert!(!config.view.nerd_font_icon_padding);
    }

    #[test]
    fn nerd_font_switches_default_icon_style() {
        let (config, warnings) =
            Config::from_configuration(&configuration(&[("nerd_font", "true")]));
        assert!(warnings.is_empty());
        assert!(config.view.nerd_font);
        assert_eq!(config.view.icons, IconStyle::Nerd);

        let (config, warnings) =
            Config::from_configuration(&configuration(&[("nerd_font", "false")]));
        assert!(warnings.is_empty());
        assert!(!config.view.nerd_font);
        assert_eq!(config.view.icons, IconStyle::Symbol);
    }

    #[test]
    fn explicit_icon_style_wins_over_nerd_font() {
        let (config, warnings) = Config::from_configuration(&configuration(&[
            ("nerd_font", "true"),
            ("icon_style", "emoji"),
        ]));
        assert!(warnings.is_empty());
        assert!(config.view.nerd_font);
        assert_eq!(config.view.icons, IconStyle::Emoji);

        // nerd_font なしで状態アイコンだけ Nerd Font にもできる
        let (config, warnings) =
            Config::from_configuration(&configuration(&[("icon_style", "nerd")]));
        assert!(warnings.is_empty());
        assert!(!config.view.nerd_font);
        assert_eq!(config.view.icons, IconStyle::Nerd);
    }

    #[test]
    fn invalid_icon_style_falls_back_to_nerd_in_nerd_font_mode() {
        let (config, warnings) = Config::from_configuration(&configuration(&[
            ("nerd_font", "true"),
            ("icon_style", "emoticon"),
        ]));
        assert_eq!(config.view.icons, IconStyle::Nerd);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("icon_style:"));
    }

    #[test]
    fn invalid_nerd_font_falls_back_to_default_with_warning() {
        for value in ["yes", "True", "1", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("nerd_font", value),
                ("show_app", "false"),
            ]));
            assert!(!config.view.nerd_font, "{value}");
            assert_eq!(config.view.icons, IconStyle::Symbol, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("nerd_font:"), "{value}");
        }
    }

    #[test]
    fn badge_shape() {
        let cases = [
            ("slash", BadgeShape::Slash),
            ("arrow", BadgeShape::Arrow),
            ("round", BadgeShape::Round),
        ];
        for (value, expected) in cases {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("nerd_font", "true"),
                ("badge_shape", value),
            ]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.badge_shape, expected, "{value}");
        }
    }

    #[test]
    fn invalid_badge_shape_falls_back_to_default_with_warning() {
        for value in ["Arrow", "space", ">", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("badge_shape", value),
                ("nerd_font", "true"),
            ]));
            assert_eq!(config.view.badge_shape, BadgeShape::Slash, "{value}");
            // 他のキーは影響を受けない
            assert!(config.view.nerd_font, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("badge_shape:"), "{value}");
        }
    }

    #[test]
    fn blocked_first() {
        for (value, expected) in [("true", true), ("false", false)] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("blocked_first", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.blocked_first, expected, "{value}");
        }
    }

    #[test]
    fn show_cwd() {
        for (value, expected) in [("true", true), ("false", false)] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("show_cwd", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.show_cwd, expected, "{value}");
        }
    }

    #[test]
    fn invalid_blocked_first_falls_back_to_default_with_warning() {
        for value in ["yes", "True", "1", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("blocked_first", value),
                ("show_app", "false"),
            ]));
            assert!(!config.view.blocked_first, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("blocked_first:"), "{value}");
        }
    }

    #[test]
    fn max_length() {
        let cases = [("40", Some(40)), ("17", Some(MIN_MAX_LENGTH)), ("0", None)];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("max_length", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.max_length, expected, "{value}");
        }
    }

    #[test]
    fn invalid_max_length_falls_back_to_default_with_warning() {
        for value in ["abc", "1", "16", "-1", "40.5", " 40", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("max_length", value),
                ("apps", "claude"),
            ]));
            assert_eq!(config.view.max_length, Some(DEFAULT_MAX_LENGTH), "{value}");
            // 他のキーは影響を受けない
            assert_eq!(
                config.view.apps,
                Some(vec!["claude".to_string()]),
                "{value}"
            );
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("max_length:"), "{value}");
        }
    }

    #[test]
    fn invalid_show_cwd_falls_back_to_default_with_warning() {
        for value in ["yes", "True", "1", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("show_cwd", value),
                ("show_app", "false"),
            ]));
            assert!(!config.view.show_cwd, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("show_cwd:"), "{value}");
        }
    }

    #[test]
    fn nerd_font_icon_padding() {
        for (value, expected) in [("true", true), ("false", false)] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("nerd_font_icon_padding", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.nerd_font_icon_padding, expected, "{value}");
        }
    }

    #[test]
    fn invalid_nerd_font_icon_padding_falls_back_to_default_with_warning() {
        for value in ["yes", "True", "1", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("nerd_font_icon_padding", value),
                ("show_app", "false"),
            ]));
            assert!(!config.view.nerd_font_icon_padding, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(
                warnings[0].starts_with("nerd_font_icon_padding:"),
                "{value}"
            );
        }
    }

    #[test]
    fn highlight_attention() {
        for (value, expected) in [("true", true), ("false", false)] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("highlight_attention", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.highlight_attention, expected, "{value}");
        }
    }

    #[test]
    fn invalid_highlight_attention_falls_back_to_default_with_warning() {
        for value in ["yes", "True", "1", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("highlight_attention", value),
                ("show_app", "false"),
            ]));
            assert!(!config.view.highlight_attention, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("highlight_attention:"), "{value}");
        }
    }

    #[test]
    fn show_app() {
        for (value, expected) in [("true", true), ("false", false)] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("show_app", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.show_app, expected, "{value}");
        }
    }

    #[test]
    fn invalid_show_app_falls_back_to_default_with_warning() {
        for value in ["no", "False", "0", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("show_app", value),
                ("apps", "claude"),
            ]));
            assert!(config.view.show_app, "{value}");
            // 他のキーは影響を受けない
            assert_eq!(
                config.view.apps,
                Some(vec!["claude".to_string()]),
                "{value}"
            );
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("show_app:"), "{value}");
        }
    }

    #[test]
    fn icon_style() {
        let cases = [
            ("symbol", IconStyle::Symbol),
            ("alphabet", IconStyle::Alphabet),
            ("emoji", IconStyle::Emoji),
            ("nerd", IconStyle::Nerd),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("icon_style", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.icons, expected, "{value}");
        }
    }

    #[test]
    fn unknown_icon_style_falls_back_to_default_with_warning() {
        let (config, warnings) = Config::from_configuration(&configuration(&[
            ("icon_style", "emoticon"),
            ("apps", "claude"),
        ]));
        assert_eq!(config.view.icons, IconStyle::Symbol);
        // 他のキーは影響を受けない
        assert_eq!(config.view.apps, Some(vec!["claude".to_string()]));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("icon_style:"));
    }

    #[test]
    fn apps_accepts_commas_and_spaces() {
        assert_eq!(
            parse_apps(" claude, codex  gemini,,"),
            Some(vec![
                "claude".to_string(),
                "codex".to_string(),
                "gemini".to_string()
            ])
        );
        assert_eq!(parse_apps(""), None);
        assert_eq!(parse_apps(" , "), None);
    }

    #[test]
    fn apps_is_passed_to_view_options() {
        let (config, warnings) = Config::from_configuration(&configuration(&[("apps", "claude")]));
        assert!(warnings.is_empty());
        assert_eq!(config.view.apps, Some(vec!["claude".to_string()]));
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (config, warnings) = Config::from_configuration(&configuration(&[("foo", "bar")]));
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
    }
}
