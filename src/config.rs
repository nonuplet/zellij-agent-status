//! プラグイン設定（`load` に渡される configuration）の読み込み。
//! キーの一覧と意味は README の「設定」にまとめている。ここにはキーの名前の付け方と、読み込みの規則だけを書く。
//!
//! Zellij はレイアウトの plugin ブロックの子ノードを、`名前 → 最初の値の文字列` のフラットな
//! マップにして渡す（真偽値や数値も文字列になる）。そのため、設定はすべて文字列のキーと値で表す。
//!
//! ## キーの名前の付け方
//!
//! - 何に効くのかが名前だけで分かるようにする（`state_icon_style`、`max_agent_width` など）
//! - 真偽値は、`true` のときにどうなるかを名前にする（`blocked_first`、`highlight_attention` など）
//! - `_items` は、表示する要素の一覧に付ける。どこに表示するかを前に付ける（`badge_items`、`text_items`）
//! - `nerd_font_` は、Nerd Font の文字を使うアイコンにだけ効くものに付ける（`nerd_font` だけは既定値の切り替え）
//! - 値で Nerd Font を表すときも `nerd_font` と書く
//!
//! ## 読み込みの規則
//!
//! 真偽値は `true` / `false` だけを受け付ける。KDL の真偽値（`blocked_first false`）も
//! 文字列（`blocked_first "false"`）も、Zellij からは同じ文字列で届く。
//!
//! 一覧（`app_filter`、`*_items`）は、カンマか空白で区切る。
//!
//! エージェントの並びは `blocked_first` 以外では変えられない。1 エージェント分の中の要素の並びは `*_items` に書いた順で、
//! 切り詰め方（どの順に縮めるか）は並びによらず view（`src/view/fit.rs`）で固定している。
//!
//! `nerd_font` は `state_icon_style` / `badge_shape` / `badge_items` の既定値を切り替えるだけで、どれも指定すればそちらを優先する
//! （`badge_items` を指定したときは、`app_icon` を足さない）。
//! [`ViewOptions`] には切り替えた結果だけを入れ、`nerd_font` 自体は渡さない。
//!
//! 不正な値は、そのキーだけ既定値（`nerd_font` で変わるものは、切り替えた後の既定値）に戻して警告を返す。設定の誤りでプラグイン全体が動かなくなるのを避けるため。
//! 未知のキーは無視する（Zellij 側が予約キー以外を渡してくる可能性を排除できていないため、警告もしない）。

use std::collections::BTreeMap;

use crate::view::{
    BadgeShape, DEFAULT_MAX_AGENT_WIDTH, Element, MIN_MAX_AGENT_WIDTH, MsgMode, StateIconStyle,
    ViewOptions,
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

        // ほかのキーの既定値が変わるので、最初に読む
        let mut nerd_font = false;
        r.read("nerd_font", &mut nerd_font, parse_bool);
        if nerd_font {
            view.state_icon_style = StateIconStyle::NerdFont;
            view.badge_shape = BadgeShape::Slash;
            view.badge_items.insert(0, Element::AppIcon);
        }
        r.read(
            "state_icon_style",
            &mut view.state_icon_style,
            choice(STATE_ICON_STYLES),
        );
        r.read("badge_shape", &mut view.badge_shape, choice(BADGE_SHAPES));
        r.read("badge_items", &mut view.badge_items, parse_items);
        r.read("text_items", &mut view.text_items, parse_items);
        r.read("app_filter", &mut view.app_filter, |value| {
            Ok(parse_app_filter(value))
        });
        r.read("blocked_first", &mut view.blocked_first, parse_bool);
        r.read(
            "max_agent_width",
            &mut view.max_agent_width,
            parse_max_agent_width,
        );
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

const STATE_ICON_STYLES: &[(&str, StateIconStyle)] = &[
    ("symbol", StateIconStyle::Symbol),
    ("alphabet", StateIconStyle::Alphabet),
    ("emoji", StateIconStyle::Emoji),
    ("nerd_font", StateIconStyle::NerdFont),
];

const BADGE_SHAPES: &[(&str, BadgeShape)] = &[
    ("square", BadgeShape::Square),
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
/// 1 以上 [`MIN_MAX_AGENT_WIDTH`] 未満は、アイコンと進捗だけで上限を超えることがあるので受け付けない。
fn parse_max_agent_width(value: &str) -> Result<Option<usize>, String> {
    match value.parse() {
        Ok(0) => Ok(None),
        Ok(max_agent_width) if max_agent_width >= MIN_MAX_AGENT_WIDTH => Ok(Some(max_agent_width)),
        _ => Err(format!(
            "0（上限なし）か {MIN_MAX_AGENT_WIDTH} 以上の整数を指定してください（既定値 {DEFAULT_MAX_AGENT_WIDTH} を使います）"
        )),
    }
}

/// カンマか空白で区切った一覧。区切りが続いたり前後にあったりしても、空の項目は作らない
fn split_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
}

/// 空なら None（絞り込まない）。
fn parse_app_filter(value: &str) -> Option<Vec<String>> {
    let apps: Vec<String> = split_list(value).map(str::to_string).collect();
    (!apps.is_empty()).then_some(apps)
}

const ELEMENTS: &[(&str, Element)] = &[
    ("title", Element::Title),
    ("tab_name", Element::TabName),
    ("app_name", Element::AppName),
    ("app_icon", Element::AppIcon),
    ("cwd", Element::Cwd),
];

/// 要素の一覧。空なら空の一覧にする（`app_filter` と違い、既定値には戻さない。
/// 表示できる要素がなければ `#<pane_id>` を表示するのは view で決める）。知らない要素名か、重複があれば不正。
/// 警告文の案内は、受け付ける値と同じく [`ELEMENTS`] から作る。
fn parse_items(value: &str) -> Result<Vec<Element>, String> {
    let invalid = || {
        let names: Vec<String> = ELEMENTS
            .iter()
            .map(|(name, _)| format!("`{name}`"))
            .collect();
        guidance(&format!(
            "{} から、重複しないように選んでカンマか空白で区切ったもの",
            names.join("・")
        ))
    };
    let mut items = vec![];
    for name in split_list(value) {
        let &(_, element) = ELEMENTS
            .iter()
            .find(|(n, _)| *n == name)
            .ok_or_else(invalid)?;
        if items.contains(&element) {
            return Err(invalid());
        }
        items.push(element);
    }
    Ok(items)
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

    /// 真偽値の設定と、そのフィールド（`nerd_font` は対応するフィールドがないので、別のテストで確かめる）
    const BOOL_KEYS: &[(&str, Field)] = &[
        ("blocked_first", |view| view.blocked_first),
        ("highlight_attention", |view| view.highlight_attention),
        ("nerd_font_icon_padding", |view| view.nerd_font_icon_padding),
    ];

    #[test]
    fn empty_configuration_gives_defaults() {
        let (config, warnings) = Config::from_configuration(&BTreeMap::new());
        assert!(warnings.is_empty());
        assert_eq!(config.view.state_icon_style, StateIconStyle::Symbol);
        assert_eq!(config.view.app_filter, None);
        assert!(!config.view.blocked_first);
        assert_eq!(config.view.max_agent_width, Some(DEFAULT_MAX_AGENT_WIDTH));
        assert_eq!(config.view.badge_items, [Element::Title, Element::TabName]);
        assert_eq!(config.view.text_items, []);
        assert_eq!(config.view.badge_shape, BadgeShape::Square);
        assert!(!config.view.highlight_attention);
        assert!(!config.view.nerd_font_icon_padding);
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
                let (config, warnings) = Config::from_configuration(&configuration(&[
                    (key, value),
                    ("app_filter", "claude"),
                ]));
                assert_eq!(field(&config.view), field(&default), "{key} {value}");
                // 他のキーは影響を受けない
                assert_eq!(
                    config.view.app_filter,
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
                "blocked_first",
                "yes",
                "blocked_first: `yes` は使えません。`true` か `false` を指定してください（既定値を使います）",
            ),
            (
                "badge_items",
                "title,app",
                "badge_items: `title,app` は使えません。`title`・`tab_name`・`app_name`・`app_icon`・`cwd` から、重複しないように選んでカンマか空白で区切ったものを指定してください（既定値を使います）",
            ),
            (
                "state_icon_style",
                "emoticon",
                "state_icon_style: `emoticon` は使えません。`symbol`・`alphabet`・`emoji`・`nerd_font` のどれかを指定してください（既定値を使います）",
            ),
            (
                "badge_shape",
                ">",
                "badge_shape: `>` は使えません。`square`・`slash`・`arrow`・`round` のどれかを指定してください（既定値を使います）",
            ),
            (
                "msg_mode",
                "blocked",
                "msg_mode: `blocked` は使えません。`none`・`attention`・`all` のどれかを指定してください（既定値を使います）",
            ),
            (
                "max_agent_width",
                "17",
                "max_agent_width: `17` は使えません。0（上限なし）か 20 以上の整数を指定してください（既定値 50 を使います）",
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
            ("max_agent_width", "x"),
            ("text_items", "x"),
            ("badge_items", "x"),
            ("badge_shape", "x"),
            ("state_icon_style", "x"),
            ("nerd_font", "x"),
        ]));
        let keys: Vec<&str> = warnings
            .iter()
            .map(|w| w.split(':').next().unwrap())
            .collect();
        assert_eq!(
            keys,
            [
                "nerd_font",
                "state_icon_style",
                "badge_shape",
                "badge_items",
                "text_items",
                "max_agent_width",
                "msg_mode"
            ]
        );
    }

    /// `nerd_font` が true のときの既定値
    fn nerd_font_defaults() -> ViewOptions {
        ViewOptions {
            state_icon_style: StateIconStyle::NerdFont,
            badge_shape: BadgeShape::Slash,
            badge_items: vec![Element::AppIcon, Element::Title, Element::TabName],
            ..Default::default()
        }
    }

    #[test]
    fn nerd_font_only_switches_defaults() {
        let cases = [
            ("true", nerd_font_defaults()),
            ("false", ViewOptions::default()),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("nerd_font", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view, expected, "{value}");
        }
    }

    #[test]
    fn invalid_nerd_font_keeps_defaults_with_warning() {
        for value in ["yes", "True", "1", "no", "False", "0", ""] {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("nerd_font", value)]));
            assert_eq!(config.view, ViewOptions::default(), "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("nerd_font:"), "{value}");
        }
    }

    #[test]
    fn explicit_values_win_over_nerd_font() {
        for (nerd_font, defaults) in [
            ("true", nerd_font_defaults()),
            ("false", ViewOptions::default()),
        ] {
            let read = |key, value| {
                let (config, warnings) = Config::from_configuration(&configuration(&[
                    ("nerd_font", nerd_font),
                    (key, value),
                ]));
                assert!(warnings.is_empty(), "{nerd_font} {key} {value}");
                config.view
            };
            // 指定したものだけが変わり、ほかの 2 つは nerd_font に応じた既定値のまま
            for &(value, state_icon_style) in STATE_ICON_STYLES {
                let expected = ViewOptions {
                    state_icon_style,
                    ..defaults.clone()
                };
                assert_eq!(
                    read("state_icon_style", value),
                    expected,
                    "{nerd_font} {value}"
                );
            }
            for &(value, badge_shape) in BADGE_SHAPES {
                let expected = ViewOptions {
                    badge_shape,
                    ..defaults.clone()
                };
                assert_eq!(read("badge_shape", value), expected, "{nerd_font} {value}");
            }
            // badge_items を指定したら、nerd_font でも app_icon を足さない
            for (value, badge_items) in [
                ("title", vec![Element::Title]),
                ("app_icon cwd", vec![Element::AppIcon, Element::Cwd]),
                ("", vec![]),
            ] {
                let expected = ViewOptions {
                    badge_items,
                    ..defaults.clone()
                };
                assert_eq!(read("badge_items", value), expected, "{nerd_font} {value}");
            }
        }
    }

    #[test]
    fn invalid_values_fall_back_to_defaults_for_nerd_font() {
        for (nerd_font, defaults) in [
            ("true", nerd_font_defaults()),
            ("false", ViewOptions::default()),
        ] {
            for key in ["state_icon_style", "badge_shape", "badge_items"] {
                let (config, warnings) = Config::from_configuration(&configuration(&[
                    ("nerd_font", nerd_font),
                    (key, "x"),
                ]));
                assert_eq!(config.view, defaults, "{nerd_font} {key}");
                assert_eq!(warnings.len(), 1, "{nerd_font} {key}");
                assert!(
                    warnings[0].starts_with(&format!("{key}:")),
                    "{nerd_font} {key}"
                );
            }
        }
    }

    #[test]
    fn state_icon_style() {
        let cases = [
            ("symbol", StateIconStyle::Symbol),
            ("alphabet", StateIconStyle::Alphabet),
            ("emoji", StateIconStyle::Emoji),
            ("nerd_font", StateIconStyle::NerdFont),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("state_icon_style", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.state_icon_style, expected, "{value}");
        }
    }

    #[test]
    fn unknown_state_icon_style_falls_back_to_default_with_warning() {
        let (config, warnings) = Config::from_configuration(&configuration(&[
            ("state_icon_style", "emoticon"),
            ("app_filter", "claude"),
        ]));
        assert_eq!(config.view.state_icon_style, StateIconStyle::Symbol);
        // 他のキーは影響を受けない
        assert_eq!(config.view.app_filter, Some(vec!["claude".to_string()]));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("state_icon_style:"));
    }

    #[test]
    fn badge_shape() {
        let cases = [
            ("square", BadgeShape::Square),
            ("slash", BadgeShape::Slash),
            ("arrow", BadgeShape::Arrow),
            ("round", BadgeShape::Round),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("badge_shape", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.badge_shape, expected, "{value}");
        }
    }

    #[test]
    fn invalid_badge_shape_falls_back_to_default_with_warning() {
        for value in ["Arrow", "none", ">", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("badge_shape", value),
                ("app_filter", "claude"),
            ]));
            assert_eq!(config.view.badge_shape, BadgeShape::Square, "{value}");
            // 他のキーは影響を受けない
            assert_eq!(
                config.view.app_filter,
                Some(vec!["claude".to_string()]),
                "{value}"
            );
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("badge_shape:"), "{value}");
        }
    }

    #[test]
    fn max_agent_width() {
        let cases = [
            ("40", Some(40)),
            ("20", Some(MIN_MAX_AGENT_WIDTH)),
            ("0", None),
        ];
        for (value, expected) in cases {
            let (config, warnings) =
                Config::from_configuration(&configuration(&[("max_agent_width", value)]));
            assert!(warnings.is_empty(), "{value}");
            assert_eq!(config.view.max_agent_width, expected, "{value}");
        }
    }

    #[test]
    fn invalid_max_agent_width_falls_back_to_default_with_warning() {
        for value in ["abc", "1", "19", "-1", "40.5", " 40", ""] {
            let (config, warnings) = Config::from_configuration(&configuration(&[
                ("max_agent_width", value),
                ("app_filter", "claude"),
            ]));
            assert_eq!(
                config.view.max_agent_width,
                Some(DEFAULT_MAX_AGENT_WIDTH),
                "{value}"
            );
            // 他のキーは影響を受けない
            assert_eq!(
                config.view.app_filter,
                Some(vec!["claude".to_string()]),
                "{value}"
            );
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("max_agent_width:"), "{value}");
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
                ("blocked_first", "true"),
            ]));
            assert_eq!(config.view.msg_mode, MsgMode::Attention, "{value}");
            // 他のキーは影響を受けない
            assert!(config.view.blocked_first, "{value}");
            assert_eq!(warnings.len(), 1, "{value}");
            assert!(warnings[0].starts_with("msg_mode:"), "{value}");
        }
    }

    #[test]
    fn app_filter_accepts_commas_and_spaces() {
        assert_eq!(
            parse_app_filter(" claude, codex  gemini,,"),
            Some(vec![
                "claude".to_string(),
                "codex".to_string(),
                "gemini".to_string()
            ])
        );
        assert_eq!(parse_app_filter(""), None);
        assert_eq!(parse_app_filter(" , "), None);
    }

    #[test]
    fn app_filter_is_passed_to_view_options() {
        let (config, warnings) =
            Config::from_configuration(&configuration(&[("app_filter", "claude")]));
        assert!(warnings.is_empty());
        assert_eq!(config.view.app_filter, Some(vec!["claude".to_string()]));
    }

    #[test]
    fn items_keep_written_order() {
        use Element::*;
        let cases = [
            ("title", vec![Title]),
            (
                "cwd app_name,tab_name, app_icon  title",
                vec![Cwd, AppName, TabName, AppIcon, Title],
            ),
            // 空なら空の一覧（app_filter と違い、既定値には戻さない）
            ("", vec![]),
            (" , ", vec![]),
        ];
        for key in ["badge_items", "text_items"] {
            for (value, expected) in &cases {
                let (config, warnings) =
                    Config::from_configuration(&configuration(&[(key, value)]));
                assert!(warnings.is_empty(), "{key} {value}");
                let items = match key {
                    "badge_items" => &config.view.badge_items,
                    _ => &config.view.text_items,
                };
                assert_eq!(items, expected, "{key} {value}");
            }
        }
    }

    #[test]
    fn invalid_items_fall_back_to_default_with_warning() {
        // 知らない要素名、大文字、1 つのキーの中での重複
        for value in [
            "app",
            "Title",
            "title msg",
            "title,title",
            "cwd tab_name cwd",
        ] {
            for key in ["badge_items", "text_items"] {
                let (config, warnings) = Config::from_configuration(&configuration(&[
                    (key, value),
                    ("app_filter", "claude"),
                ]));
                assert_eq!(
                    config.view,
                    ViewOptions {
                        app_filter: Some(vec!["claude".to_string()]),
                        ..Default::default()
                    },
                    "{key} {value}"
                );
                assert_eq!(warnings.len(), 1, "{key} {value}");
                assert!(warnings[0].starts_with(&format!("{key}:")), "{key} {value}");
            }
        }
    }

    #[test]
    fn same_item_in_both_keys_is_accepted() {
        // 既定の badge_items と重なることもあるので、警告しない（表示は badge_items だけ。view で決める）
        let (config, warnings) = Config::from_configuration(&configuration(&[
            ("badge_items", "title cwd"),
            ("text_items", "cwd tab_name"),
        ]));
        assert!(warnings.is_empty());
        assert_eq!(config.view.badge_items, [Element::Title, Element::Cwd]);
        assert_eq!(config.view.text_items, [Element::Cwd, Element::TabName]);
        let (_, warnings) =
            Config::from_configuration(&configuration(&[("text_items", "tab_name")]));
        assert!(warnings.is_empty());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (config, warnings) = Config::from_configuration(&configuration(&[("foo", "bar")]));
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
    }
}
