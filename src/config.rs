//! プラグイン設定（`load` に渡される configuration）の読み込み。
//! キーの一覧と意味は README の「設定」にまとめている。ここには読み込みの規則だけを書く。
//!
//! Zellij はレイアウトの plugin ブロックの子ノードを、`名前 → 最初の値の文字列` のフラットな
//! マップにして渡す（真偽値や数値も文字列になる）。そのため、設定はすべて文字列のキーと値で表す。
//!
//! 真偽値は `true` / `false` だけを受け付ける。KDL の真偽値（`show_app_name false`）も
//! 文字列（`show_app_name "false"`）も、Zellij からは同じ文字列で届く。
//!
//! 表示の並びは `blocked_first` 以外では変えられない。切り詰め方（どの順に縮めるか）は view（`src/view/fit.rs`）で固定している。
//!
//! 不正な値は、そのキーだけ既定値に戻して警告を返す。設定の誤りでプラグイン全体が動かなくなるのを避けるため。
//! 未知のキーは無視する（Zellij 側が予約キー以外を渡してくる可能性を排除できていないため、警告もしない）。

use std::collections::BTreeMap;

use crate::view::{
    BadgeShape, DEFAULT_MAX_LENGTH, IconStyle, MIN_MAX_LENGTH, MsgMode, ViewOptions,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub view: ViewOptions,
}

impl Config {
    /// configuration から設定を作る。不正な値はそのキーだけ既定値にし、警告メッセージとして返す。
    /// 警告は読んだ順に並ぶ（`main.rs` は先頭の 1 つだけを表示する）。
    pub fn from_configuration(configuration: &BTreeMap<String, String>) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let view = &mut config.view;
        let mut r = Reader {
            configuration,
            warnings: Vec::new(),
        };

        // icon_style の既定値が変わるので、icon_style より先に読む
        r.read("nerd_font", &mut view.nerd_font, parse_bool);
        if view.nerd_font {
            view.icons = IconStyle::Nerd;
        }
        r.read("icon_style", &mut view.icons, choice(ICON_STYLES));
        r.read("badge_shape", &mut view.badge_shape, choice(BADGE_SHAPES));
        r.read("apps", &mut view.apps, |value| Ok(parse_apps(value)));
        r.read("show_app_name", &mut view.show_app_name, parse_bool);
        r.read("show_tab_name", &mut view.show_tab_name, parse_bool);
        r.read("blocked_first", &mut view.blocked_first, parse_bool);
        r.read("max_length", &mut view.max_length, parse_max_length);
        r.read("show_cwd", &mut view.show_cwd, parse_bool);
        r.read(
            "highlight_attention",
            &mut view.highlight_attention,
            parse_bool,
        );
        r.read(
            "nerd_font_icon_padding",
            &mut view.nerd_font_icon_padding,
            parse_bool,
        );
        r.read("badge_name", &mut view.badge_name, parse_bool);
        r.read("msg_mode", &mut view.msg_mode, choice(MSG_MODES));

        (config, r.warnings)
    }
}

/// configuration から 1 キーずつ読み、不正な値の警告を積む。
struct Reader<'a> {
    configuration: &'a BTreeMap<String, String>,
    warnings: Vec<String>,
}

impl Reader<'_> {
    /// `key` の値があれば `parse` して `target` に入れる。キーがなければ何もしない。
    /// 不正なら `target` は既定値のままにし、`parse` が返した案内（何を指定すべきか）を付けて警告を積む。
    fn read<T>(&mut self, key: &str, target: &mut T, parse: impl Fn(&str) -> Result<T, String>) {
        let Some(value) = self.configuration.get(key) else {
            return;
        };
        match parse(value) {
            Ok(parsed) => *target = parsed,
            Err(guidance) => self
                .warnings
                .push(format!("{key}: `{value}` は使えません。{guidance}")),
        }
    }
}

/// 警告文の案内のうち、既定値に戻す設定で共通の形。`expected` は指定できる値の説明
fn guidance(expected: &str) -> String {
    format!("{expected}を指定してください（既定値を使います）")
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(guidance("`true` か `false` ")),
    }
}

const ICON_STYLES: &[(&str, IconStyle)] = &[
    ("symbol", IconStyle::Symbol),
    ("alphabet", IconStyle::Alphabet),
    ("emoji", IconStyle::Emoji),
    ("nerd", IconStyle::Nerd),
];

const BADGE_SHAPES: &[(&str, BadgeShape)] = &[
    ("slash", BadgeShape::Slash),
    ("arrow", BadgeShape::Arrow),
    ("round", BadgeShape::Round),
];

const MSG_MODES: &[(&str, MsgMode)] = &[
    ("none", MsgMode::None),
    ("attention", MsgMode::Attention),
    ("all", MsgMode::All),
];

/// 列挙型の設定の parse。受け付ける値と、警告文の「どれか」の一覧を、どちらも `table` から作る
/// （別々に書くと、片方だけ直したときに案内と実際に受け付ける値がずれるため）。
fn choice<T: Copy>(table: &'static [(&'static str, T)]) -> impl Fn(&str) -> Result<T, String> {
    move |value| {
        table
            .iter()
            .find(|(name, _)| *name == value)
            .map(|&(_, parsed)| parsed)
            .ok_or_else(|| {
                let names: Vec<String> =
                    table.iter().map(|(name, _)| format!("`{name}`")).collect();
                guidance(&format!("{} のどれか", names.join("・")))
            })
    }
}

/// 0 は上限なし（`Ok(None)`）。
/// 1 以上 [`MIN_MAX_LENGTH`] 未満は、アイコンと進捗だけで上限を超えることがあるので受け付けない。
fn parse_max_length(value: &str) -> Result<Option<usize>, String> {
    match value.parse() {
        Ok(0) => Ok(None),
        Ok(max_length) if max_length >= MIN_MAX_LENGTH => Ok(Some(max_length)),
        _ => Err(format!(
            "0（上限なし）か {MIN_MAX_LENGTH} 以上の整数を指定してください（既定値 {DEFAULT_MAX_LENGTH} を使います）"
        )),
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

    type Field = fn(&ViewOptions) -> bool;

    /// 真偽値の設定と、そのフィールド
    const BOOL_KEYS: &[(&str, Field)] = &[
        ("nerd_font", |view| view.nerd_font),
        ("show_app_name", |view| view.show_app_name),
        ("show_tab_name", |view| view.show_tab_name),
        ("blocked_first", |view| view.blocked_first),
        ("show_cwd", |view| view.show_cwd),
        ("highlight_attention", |view| view.highlight_attention),
        ("nerd_font_icon_padding", |view| view.nerd_font_icon_padding),
        ("badge_name", |view| view.badge_name),
    ];

    #[test]
    fn empty_configuration_gives_defaults() {
        let (config, warnings) = Config::from_configuration(&BTreeMap::new());
        assert!(warnings.is_empty());
        assert_eq!(config.view.icons, IconStyle::Symbol);
        assert_eq!(config.view.apps, None);
        assert!(config.view.show_app_name);
        assert!(config.view.show_tab_name);
        assert!(!config.view.blocked_first);
        assert_eq!(config.view.max_length, Some(DEFAULT_MAX_LENGTH));
        assert!(!config.view.show_cwd);
        assert!(!config.view.nerd_font);
        assert_eq!(config.view.badge_shape, BadgeShape::Slash);
        assert!(!config.view.highlight_attention);
        assert!(!config.view.nerd_font_icon_padding);
        assert!(config.view.badge_name);
        assert_eq!(config.view.msg_mode, MsgMode::Attention);
    }

    #[test]
    fn bool_keys() {
        for &(key, field) in BOOL_KEYS {
            for (value, expected) in [("true", true), ("false", false)] {
                let (config, warnings) =
                    Config::from_configuration(&configuration(&[(key, value)]));
                assert!(warnings.is_empty(), "{key} {value}");
                assert_eq!(field(&config.view), expected, "{key} {value}");
            }
        }
    }

    #[test]
    fn invalid_bool_falls_back_to_default_with_warning() {
        let default = ViewOptions::default();
        for &(key, field) in BOOL_KEYS {
            for value in ["yes", "True", "1", "no", "False", "0", ""] {
                let (config, warnings) =
                    Config::from_configuration(&configuration(&[(key, value), ("apps", "claude")]));
                assert_eq!(field(&config.view), field(&default), "{key} {value}");
                // 他のキーは影響を受けない
                assert_eq!(
                    config.view.apps,
                    Some(vec!["claude".to_string()]),
                    "{key} {value}"
                );
                assert_eq!(warnings.len(), 1, "{key} {value}");
                assert!(warnings[0].starts_with(&format!("{key}:")), "{key} {value}");
            }
        }
    }

    #[test]
    fn warning_messages() {
        let cases = [
            (
                "show_cwd",
                "yes",
                "show_cwd: `yes` は使えません。`true` か `false` を指定してください（既定値を使います）",
            ),
            (
                "icon_style",
                "emoticon",
                "icon_style: `emoticon` は使えません。`symbol`・`alphabet`・`emoji`・`nerd` のどれかを指定してください（既定値を使います）",
            ),
            (
                "badge_shape",
                ">",
                "badge_shape: `>` は使えません。`slash`・`arrow`・`round` のどれかを指定してください（既定値を使います）",
            ),
            (
                "msg_mode",
                "blocked",
                "msg_mode: `blocked` は使えません。`none`・`attention`・`all` のどれかを指定してください（既定値を使います）",
            ),
            (
                "max_length",
                "17",
                "max_length: `17` は使えません。0（上限なし）か 18 以上の整数を指定してください（既定値 50 を使います）",
            ),
        ];
        for (key, value, expected) in cases {
            let (_, warnings) = Config::from_configuration(&configuration(&[(key, value)]));
            assert_eq!(warnings, [expected], "{key} {value}");
        }
    }

    #[test]
    fn warnings_keep_reading_order() {
        // main.rs は先頭の警告だけを表示するので、順番も変えない
        let (_, warnings) = Config::from_configuration(&configuration(&[
            ("msg_mode", "x"),
            ("max_length", "x"),
            ("icon_style", "x"),
            ("nerd_font", "x"),
        ]));
        let keys: Vec<&str> = warnings
            .iter()
            .map(|w| w.split(':').next().unwrap())
            .collect();
        assert_eq!(keys, ["nerd_font", "icon_style", "max_length", "msg_mode"]);
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
    fn invalid_nerd_font_keeps_default_icon_style() {
        for value in ["yes", "True", "1", ""] {
            let (config, _) = Config::from_configuration(&configuration(&[("nerd_font", value)]));
            assert_eq!(config.view.icons, IconStyle::Symbol, "{value}");
        }
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
    fn max_length() {
        let cases = [("40", Some(40)), ("18", Some(MIN_MAX_LENGTH)), ("0", None)];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("max_length", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.max_length, expected, "{value}");
        }
    }

    #[test]
    fn invalid_max_length_falls_back_to_default_with_warning() {
        for value in ["abc", "1", "17", "-1", "40.5", " 40", ""] {
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
    fn msg_mode() {
        let cases = [
            ("none", MsgMode::None),
            ("attention", MsgMode::Attention),
            ("all", MsgMode::All),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("msg_mode", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.msg_mode, expected, "{value}");
        }
    }

    #[test]
    fn invalid_msg_mode_falls_back_to_default_with_warning() {
        for value in ["All", "blocked", "true", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("msg_mode", value),
                ("show_app_name", "false"),
            ]));
            assert_eq!(config.view.msg_mode, MsgMode::Attention, "{value}");
            // 他のキーは影響を受けない
            assert!(!config.view.show_app_name, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("msg_mode:"), "{value}");
        }
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
