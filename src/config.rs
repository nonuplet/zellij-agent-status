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
//! }
//! ```
//!
//! | キー | 既定値 | 内容 |
//! |---|---|---|
//! | `icon_style` | `symbol` | アイコンを記号（`symbol`）、アルファベット 1 文字（`alphabet`）、絵文字（`emoji`）のどれにするか |
//! | `apps` | （空） | 表示する `app` の一覧。カンマか空白で区切る。空なら絞り込まない |
//! | `show_app` | `true` | `app` を名前の候補にするか。名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決まり、`false` なら `app` を飛ばす |
//! | `blocked_first` | `false` | 操作待ち（`Blocked`）のエージェントを先頭に並べるか。`false` なら (タブ, ペイン) の順のまま |
//! | `max_length` | `30` | 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。[`MIN_MAX_LENGTH`] 以上の整数。`0` なら上限なし |
//!
//! 真偽値は `true` / `false` だけを受け付ける。KDL の真偽値（`show_app false`）も
//! 文字列（`show_app "false"`）も、Zellij からは同じ文字列で届く。
//!
//! 表示の並びは `blocked_first` 以外では変えられない。切り詰め方（どの順に縮めるか）は view（`src/view/layout.rs`）で固定している。
//!
//! 不正な値は、そのキーだけ既定値に戻して警告を返す。設定の誤りでプラグイン全体が動かなくなるのを避けるため。
//! 未知のキーは無視する（Zellij 側が予約キー以外を渡してくる可能性を排除できていないため、警告もしない）。

use std::collections::BTreeMap;

use crate::view::{DEFAULT_MAX_LENGTH, IconStyle, MIN_MAX_LENGTH, ViewOptions};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub view: ViewOptions,
}

impl Config {
    /// configuration から設定を作る。不正な値はそのキーだけ既定値にし、警告メッセージとして返す。
    pub fn from_configuration(configuration: &BTreeMap<String, String>) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();

        if let Some(value) = configuration.get("icon_style") {
            match parse_icon_style(value) {
                Some(icons) => config.view.icons = icons,
                None => warnings.push(format!(
                    "icon_style: `{value}` は使えません。`symbol`・`alphabet`・`emoji` のどれかを指定してください（既定値を使います）"
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
        let cases = [("40", Some(40)), ("12", Some(MIN_MAX_LENGTH)), ("0", None)];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("max_length", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.max_length, expected, "{value}");
        }
    }

    #[test]
    fn invalid_max_length_falls_back_to_default_with_warning() {
        for value in ["abc", "1", "11", "-1", "40.5", " 40", ""] {
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
