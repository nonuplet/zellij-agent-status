//! 選択モード（[`crate::select`]）に入るキーの登録。
//!
//! Zellij の tab モードに、設定 `select_key` のキーを追加する。キーを押すと、キーバインドから
//! パイプ（`MessagePlugin`）で [`PIPE_NAME`] のメッセージが届き、プラグインが選択モードに入る。
//! パイプの送り先はプラグインの URL ではなく `name` だけで指定する。URL を書くと、設定値の違いなどで
//! 別のインスタンスが起動することがあるため。
//!
//! 登録は `reconfigure(.., false)` で、実行中のセッションの設定にだけ行い、ユーザの設定ファイルには書き込まない。
//! キーの動作の最後は、プリセットの基本のモード（`ModeInfo::base_mode`。default は `Normal`、unlock-first は `Locked`）に戻す。
//!
//! 登録するかどうかは、`ModeUpdate` で届く tab モードのキーバインドを見て決める（[`Keybind::step`]）。
//! - キーが空いていれば登録する
//! - すでにパイプ（`MessagePlugin`）になっていれば何もしない
//!   （登録した後の `ModeUpdate` や、ユーザが自分で `config.kdl` に書いた場合）
//! - ほかの動作に使われていれば登録せず、警告を出す
//!
//! パイプの送り先（[`PIPE_NAME`]）までは確かめられない。プラグインに届く `ModeUpdate` は protobuf を通るが、
//! zellij-utils 0.46 の変換では `KeybindPipe` の `name` などが捨てられ、すべて `name: None` で届くため
//! （`plugin_api/action.rs`）。そのため、ほかのプラグインへのパイプに使われているキーも登録せず、警告も出さない。
//!
//! 登録した後にキーが空いた（設定ファイルが読み直されて、登録が消えた）ときは、登録し直す。
//! プラグインがタブごとに起動している場合は、どのインスタンスも同じ内容で登録する。

use std::str::FromStr;

use zellij_tile::prelude::actions::Action;
use zellij_tile::prelude::*;

/// キーバインドから届くパイプのメッセージの名前
pub const PIPE_NAME: &str = "zellij-agent-status::select";

/// `select_key` の既定値
pub const DEFAULT_SELECT_KEY: &str = "a";

/// 選択モードに入るキー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectKey {
    key: KeyWithModifier,
    /// 設定に書かれた文字列。KDL の `bind` にはこちらを書く
    /// （`KeyWithModifier` の `Display` は `←` などになり、KDL のキーとして読めないため）
    text: String,
}

impl SelectKey {
    /// `a` や `Ctrl a` のような、Zellij の `bind` と同じ書き方のキー。解釈できなければ None
    pub fn parse(text: &str) -> Option<Self> {
        let key = KeyWithModifier::from_str(text).ok()?;
        Some(SelectKey {
            key,
            text: text.trim().to_owned(),
        })
    }
}

impl Default for SelectKey {
    fn default() -> Self {
        SelectKey::parse(DEFAULT_SELECT_KEY).expect("既定のキーは解釈できる")
    }
}

/// tab モードでのキーの使われ方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Binding {
    /// 使われていない
    Free,
    /// パイプになっている（このプラグインへのものとみなす。送り先は分からない）
    Pipe,
    /// ほかの動作に使われている
    Taken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Status {
    /// まだ登録していない
    #[default]
    Unregistered,
    /// 登録を要求し、`ModeUpdate` に反映されるのを待っている
    Requested,
    /// `ModeUpdate` で登録を確かめた
    Registered,
    /// ほかの動作に使われているので登録しなかった（警告を出した）
    Taken,
}

/// [`Keybind::step`] で行うこと
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    None,
    /// この KDL を `reconfigure` に渡す
    Register(String),
    /// 登録しなかったので、この警告を出す
    Warn(String),
}

/// キーの登録の状態。
#[derive(Debug, Default)]
pub struct Keybind {
    /// None なら登録しない（`select_key ""`）
    key: Option<SelectKey>,
    /// 最後の `ModeUpdate` で見た、キーの使われ方と基本のモード
    observed: Option<(Binding, InputMode)>,
    status: Status,
}

impl Keybind {
    pub fn new(key: Option<SelectKey>) -> Self {
        Keybind {
            key,
            ..Default::default()
        }
    }

    /// `ModeUpdate` を受け取ったときに呼ぶ。キーの使われ方を記録する。
    /// キーバインドが空の `ModeInfo`（設定が届く前など）は、判定できないので記録しない。
    pub fn observe(&mut self, mode_info: &ModeInfo) {
        let Some(key) = &self.key else {
            return;
        };
        if mode_info.keybinds.is_empty() {
            return;
        }
        let base_mode = mode_info.base_mode.unwrap_or(InputMode::Normal);
        self.observed = Some((binding(&mode_info.keybinds, &key.key), base_mode));
    }

    /// 記録したキーの使われ方から、次に行うことを決める。
    /// permission（`Reconfigure`）が許可されているときだけ呼ぶ。
    pub fn step(&mut self) -> Step {
        let (Some(key), Some((binding, base_mode))) = (&self.key, self.observed) else {
            return Step::None;
        };
        match (self.status, binding) {
            (_, Binding::Pipe) => {
                self.status = Status::Registered;
                Step::None
            }
            // 要求した登録が反映されるまでは、登録し直さない
            (Status::Requested, Binding::Free) => Step::None,
            (_, Binding::Free) => {
                self.status = Status::Requested;
                Step::Register(config_kdl(key, base_mode))
            }
            (Status::Taken, Binding::Taken) => Step::None,
            (_, Binding::Taken) => {
                self.status = Status::Taken;
                Step::Warn(format!(
                    "select_key: `{}` は tab モードですでに使われているため登録しません。別のキーを指定するか、`\"\"` で無効にしてください",
                    key.text
                ))
            }
        }
    }
}

/// tab モードで `key` がどう使われているか
fn binding(keybinds: &KeybindsVec, key: &KeyWithModifier) -> Binding {
    let actions = keybinds
        .iter()
        .filter(|(mode, _)| *mode == InputMode::Tab)
        .flat_map(|(_, binds)| binds)
        .find(|(bound, _)| bound == key)
        .map(|(_, actions)| actions);
    match actions {
        None => Binding::Free,
        Some(actions) if actions.iter().any(is_pipe) => Binding::Pipe,
        Some(_) => Binding::Taken,
    }
}

/// パイプか。送り先（`name`）は `ModeUpdate` では分からないので見ない（モジュールの説明を参照）
fn is_pipe(action: &Action) -> bool {
    matches!(action, Action::KeybindPipe { .. })
}

/// `reconfigure` に渡す設定。README の「自分で `config.kdl` に書く場合」の例と同じ形にする。
fn config_kdl(key: &SelectKey, base_mode: InputMode) -> String {
    format!(
        "keybinds {{\n    tab {{\n        bind \"{key}\" {{\n            MessagePlugin {{ name \"{PIPE_NAME}\"; }}\n            SwitchToMode \"{base_mode:?}\";\n        }}\n    }}\n}}\n",
        key = kdl_escape(&key.text),
    )
}

/// KDL の文字列（`"..."`）の中に書けるようにする。キーが `"` や `\` 1 文字のときに要る
fn kdl_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> KeyWithModifier {
        KeyWithModifier::from_str(text).unwrap()
    }

    fn pipe(name: &str) -> Action {
        Action::KeybindPipe {
            name: Some(name.to_owned()),
            payload: None,
            args: None,
            plugin: None,
            plugin_id: None,
            configuration: None,
            launch_new: false,
            skip_cache: false,
            floating: None,
            in_place: None,
            cwd: None,
            pane_title: None,
        }
    }

    fn mode_info(tab: Vec<(&str, Vec<Action>)>, base_mode: Option<InputMode>) -> ModeInfo {
        let tab = tab.into_iter().map(|(k, a)| (key(k), a)).collect();
        ModeInfo {
            base_mode,
            keybinds: vec![
                (InputMode::Normal, vec![(key("Ctrl t"), vec![Action::Quit])]),
                (InputMode::Tab, tab),
            ],
            ..Default::default()
        }
    }

    fn keybind(text: &str) -> Keybind {
        Keybind::new(SelectKey::parse(text))
    }

    #[test]
    fn parse_accepts_zellij_key_syntax() {
        assert_eq!(SelectKey::parse("a").unwrap().key, key("a"));
        assert_eq!(SelectKey::parse(" Ctrl a ").unwrap().text, "Ctrl a");
        assert_eq!(
            SelectKey::parse("Alt Shift x").unwrap().key,
            key("Alt Shift x")
        );
        for text in ["", " ", "ctrl+a", "foo", "Hyper a"] {
            assert_eq!(SelectKey::parse(text), None, "{text:?}");
        }
        assert_eq!(SelectKey::default().text, DEFAULT_SELECT_KEY);
    }

    #[test]
    fn registers_free_key_once_until_confirmed() {
        let mut keybind = keybind("a");
        // ModeUpdate が届くまでは何もしない
        assert_eq!(keybind.step(), Step::None);

        keybind.observe(&mode_info(vec![("n", vec![Action::Quit])], None));
        let Step::Register(kdl) = keybind.step() else {
            panic!("登録する");
        };
        assert!(kdl.contains("bind \"a\""), "{kdl}");
        // 反映を待つ間は登録し直さない
        assert_eq!(keybind.step(), Step::None);
        keybind.observe(&mode_info(vec![("n", vec![Action::Quit])], None));
        assert_eq!(keybind.step(), Step::None);

        // 反映された
        keybind.observe(&mode_info(vec![("a", vec![pipe(PIPE_NAME)])], None));
        assert_eq!(keybind.step(), Step::None);

        // 設定ファイルが読み直されて消えたら、登録し直す
        keybind.observe(&mode_info(vec![], None));
        assert!(matches!(keybind.step(), Step::Register(_)));
    }

    #[test]
    fn existing_pipe_is_not_registered_again() {
        // ユーザが自分で config.kdl に書いた場合
        let mut keybind = keybind("a");
        keybind.observe(&mode_info(vec![("a", vec![pipe(PIPE_NAME)])], None));
        assert_eq!(keybind.step(), Step::None);

        // 送り先は分からないので、ほかのプラグインへのパイプでも登録せず、警告も出さない
        let mut keybind = self::keybind("a");
        keybind.observe(&mode_info(vec![("a", vec![pipe("other")])], None));
        assert_eq!(keybind.step(), Step::None);
    }

    /// プラグインに届くのと同じく、protobuf を通した Action
    fn through_protobuf(action: Action) -> Action {
        use zellij_utils::plugin_api::action::ProtobufAction;
        let protobuf = ProtobufAction::try_from(action).unwrap();
        Action::try_from(protobuf).unwrap()
    }

    #[test]
    fn registered_key_is_recognized_after_protobuf_conversion() {
        // 登録した KDL を Zellij が読み、protobuf を通して ModeUpdate で届いた形
        let mut keybind = keybind("a");
        keybind.observe(&mode_info(vec![], None));
        let Step::Register(kdl) = keybind.step() else {
            panic!("登録する");
        };
        let config = zellij_utils::input::config::Config::from_kdl(&kdl, None).unwrap();
        let actions: Vec<Action> = config
            .keybinds
            .get_actions_for_key_in_mode(&InputMode::Tab, &key("a"))
            .unwrap()
            .iter()
            .cloned()
            .map(through_protobuf)
            .collect();
        // name は捨てられて届く
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, Action::KeybindPipe { name: None, .. })),
            "{actions:?}"
        );

        // 登録したキーとして扱い、警告を出さない
        keybind.observe(&mode_info(vec![("a", actions.clone())], None));
        assert_eq!(keybind.step(), Step::None);
        keybind.observe(&mode_info(vec![("a", actions)], None));
        assert_eq!(keybind.step(), Step::None);
    }

    #[test]
    fn taken_key_warns_once() {
        let mut keybind = keybind("n");
        keybind.observe(&mode_info(vec![("n", vec![Action::Quit])], None));
        let Step::Warn(warning) = keybind.step() else {
            panic!("警告する");
        };
        assert!(warning.starts_with("select_key: `n`"), "{warning}");
        assert_eq!(keybind.step(), Step::None);

        // 空いたら登録する
        keybind.observe(&mode_info(vec![], None));
        assert!(matches!(keybind.step(), Step::Register(_)));
    }

    #[test]
    fn only_tab_mode_is_checked() {
        // Normal モードの Ctrl t は関係ない
        let mut keybind = keybind("Ctrl t");
        keybind.observe(&mode_info(vec![], None));
        assert!(matches!(keybind.step(), Step::Register(_)));
    }

    #[test]
    fn uppercase_key_matches_shift() {
        let mut keybind = keybind("A");
        keybind.observe(&mode_info(vec![("Shift a", vec![Action::Quit])], None));
        assert!(matches!(keybind.step(), Step::Warn(_)));
    }

    #[test]
    fn disabled_or_unknown_keybinds_do_nothing() {
        let mut keybind = Keybind::new(None);
        keybind.observe(&mode_info(vec![], None));
        assert_eq!(keybind.step(), Step::None);

        // キーバインドが届いていない ModeInfo では判定しない
        let mut keybind = self::keybind("a");
        keybind.observe(&ModeInfo::default());
        assert_eq!(keybind.step(), Step::None);
    }

    #[test]
    fn kdl_returns_to_base_mode() {
        let kdl = |base_mode| {
            let mut keybind = keybind("a");
            keybind.observe(&mode_info(vec![], base_mode));
            match keybind.step() {
                Step::Register(kdl) => kdl,
                step => panic!("{step:?}"),
            }
        };
        assert_eq!(
            kdl(None),
            r#"keybinds {
    tab {
        bind "a" {
            MessagePlugin { name "zellij-agent-status::select"; }
            SwitchToMode "Normal";
        }
    }
}
"#
        );
        assert!(kdl(Some(InputMode::Normal)).contains("SwitchToMode \"Normal\";"));
        assert!(kdl(Some(InputMode::Locked)).contains("SwitchToMode \"Locked\";"));
    }

    #[test]
    fn kdl_parses_as_zellij_config() {
        // 生成した KDL を Zellij の設定として読めるか（キーとパイプの名前が正しく読めるか）
        for text in ["a", "Ctrl a", "\"", "\\"] {
            let select_key = SelectKey::parse(text).unwrap();
            let kdl = config_kdl(&select_key, InputMode::Locked);
            let config = zellij_utils::input::config::Config::from_kdl(&kdl, None)
                .unwrap_or_else(|e| panic!("{text:?}: {e:?}\n{kdl}"));
            let actions = config
                .keybinds
                .get_actions_for_key_in_mode(&InputMode::Tab, &select_key.key)
                .unwrap_or_else(|| panic!("{text:?}\n{kdl}"));
            assert!(
                actions.iter().any(
                    |a| matches!(a, Action::KeybindPipe { name: Some(name), .. } if name == PIPE_NAME)
                ),
                "{text:?}: {actions:?}"
            );
            assert!(
                actions.contains(&Action::SwitchToMode {
                    input_mode: InputMode::Locked
                }),
                "{text:?}: {actions:?}"
            );
        }
    }
}
