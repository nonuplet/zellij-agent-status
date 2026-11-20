//! プラグイン設定（`load` に渡される configuration）の読み込み。
//!
//! Zellij はレイアウトの plugin ブロックの子ノードを、`名前 → 最初の値の文字列` のフラットな
//! マップにして渡す（真偽値や数値も文字列になる）。そのため、設定はすべて文字列のキーと値で表す。
//!
//! ```kdl
//! plugin location="file:zellij-agent-status.wasm" {
//!     icon_style "alphabet"
//!     apps "claude, codex"
//! }
//! ```
//!
//! | キー | 既定値 | 内容 |
//! |---|---|---|
//! | `icon_style` | `symbol` | アイコンを記号（`symbol`）にするか、アルファベット 1 文字（`alphabet`）にするか |
//! | `apps` | （空） | 表示する `app` の一覧。カンマか空白で区切る。空なら絞り込まない |
//!
//! 表示の並びや切り詰めは view.rs で固定しているので、設定では変えられない。
//!
//! 不正な値は、そのキーだけ既定値に戻して警告を返す。設定の誤りでプラグイン全体が動かなくなるのを避けるため。
//! 未知のキーは無視する（Zellij 側が予約キー以外を渡してくる可能性を排除できていないため、警告もしない）。

use std::collections::BTreeMap;

use crate::view::{IconStyle, ViewOptions};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub view: ViewOptions,
    /// 表示する `app`。None なら絞り込まない
    pub apps: Option<Vec<String>>,
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
                    "icon_style: `{value}` は使えません。`symbol` か `alphabet` を指定してください（既定値を使います）"
                )),
            }
        }

        if let Some(value) = configuration.get("apps") {
            config.apps = parse_apps(value);
        }

        (config, warnings)
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

fn parse_icon_style(value: &str) -> Option<IconStyle> {
    match value {
        "symbol" => Some(IconStyle::Symbol),
        "alphabet" => Some(IconStyle::Alphabet),
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
        assert_eq!(config.apps, None);
    }

    #[test]
    fn icon_style() {
        let cases = [
            ("symbol", IconStyle::Symbol),
            ("alphabet", IconStyle::Alphabet),
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
            ("icon_style", "emoji"),
            ("apps", "claude"),
        ]));
        assert_eq!(config.view.icons, IconStyle::Symbol);
        // 他のキーは影響を受けない
        assert_eq!(config.apps, Some(vec!["claude".to_string()]));
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
    fn accepts_app_without_filter() {
        let config = Config::default();
        assert!(config.accepts_app(Some("claude")));
        assert!(config.accepts_app(None));
    }

    #[test]
    fn accepts_app_with_filter() {
        let (config, _) = Config::from_configuration(&configuration(&[("apps", "claude")]));
        assert!(config.accepts_app(Some("claude")));
        assert!(!config.accepts_app(Some("codex")));
        assert!(!config.accepts_app(Some("Claude")));
        assert!(!config.accepts_app(None));
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (config, warnings) = Config::from_configuration(&configuration(&[("foo", "bar")]));
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
    }
}
