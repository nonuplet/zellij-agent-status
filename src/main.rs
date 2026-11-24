mod collect;
mod config;
mod model;
mod permission;
mod select;
mod view;

use std::collections::{BTreeMap, HashSet};
use zellij_tile::prelude::*;

use collect::Collector;
use config::Config;
use model::AgentStatus;
use permission::Permission;
use select::focus::{self, Focus};
use select::{KeyAction, PIPE_NAME};
use view::{Line, Theme};

/// 表示するメッセージとログ（`eprintln!`）の先頭に付ける、どのプラグインからのものかを示す名前
const MESSAGE_PREFIX: &str = "zellij-agent-status: ";

/// ここ（`main.rs`）で直接処理するイベント。ほかのイベントは、使うモジュールがそれぞれの `EVENTS` に持つ
const EVENTS: &[EventType] = &[
    EventType::ModeUpdate,
    EventType::Mouse,
    EventType::InterceptedKeyPress,
];

#[derive(Default)]
struct State {
    permission: Permission,
    collector: Collector,
    config: Config,
    /// 設定の警告。エージェントがいないときに表示する
    warnings: Vec<String>,
    theme: Theme,
    /// 最後に描画した行。クリックされた列のエージェントや、キー操作で選べるエージェントを引くのに使う
    line: Line,
    focus: Focus,
    /// 選択モード中なら Some
    selection: Option<Selection>,
}

/// 選択モードの状態
struct Selection {
    /// 選んでいるエージェントのペインの id
    pane_id: u32,
    /// 選択モードに入ったときにフォーカスしていたペイン（[`focus::focused_now`]）。
    /// ほかのペインやタブがクリックされるなどして変わったら、選択モードを抜ける
    focused: PaneId,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        let (config, warnings) = Config::from_configuration(&configuration);
        for warning in &warnings {
            eprintln!("{MESSAGE_PREFIX}{warning}");
        }
        self.config = config;
        self.warnings = warnings;
        self.focus = Focus::new(get_plugin_ids().plugin_id);

        // 許可が出るまで selectable のままにする処理は Permission が持つ
        Permission::request();
        // 同じイベントを複数のモジュールが使っていてもよい（重複は subscribe がまとめる）
        subscribe(&[Permission::EVENTS, Collector::EVENTS, Focus::EVENTS, EVENTS].concat());
    }

    fn update(&mut self, event: Event) -> bool {
        match &event {
            Event::PermissionRequestResult(status) => self.permission.update(*status),
            Event::ModeUpdate(mode_info) => {
                let theme = Theme::from_styling(&mode_info.style.colors);
                let changed = theme != self.theme;
                self.theme = theme;
                changed
            }
            Event::Mouse(mouse) => self.click(*mouse),
            Event::InterceptedKeyPress(key) => self.key(key),
            _ => {
                self.focus.update(&event);
                let changed = self.collector.update(&event);
                // フォーカスの移動は PaneUpdate / TabUpdate で届く（購読は Focus::EVENTS）
                let moved = matches!(event, Event::PaneUpdate(_) | Event::TabUpdate(_))
                    && self.leave_on_focus_change();
                moved || changed
            }
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        pipe_message.name == PIPE_NAME && self.enter()
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 {
            return;
        }
        self.line = self.line(cols);
        self.keep_selection_visible();
        let selected = self.selection.as_ref().map(|s| s.pane_id);
        print!("{}", self.line.paint(&self.theme, selected));
    }
}

impl State {
    /// 描画した行のエージェントを左クリックしたら、そのペインにフォーカスする。
    /// `focus_terminal_pane` は、ペインが別のタブにあればそのタブにも切り替える。
    /// 行は見ない（Zellij の tab-bar と同じ）。ペインの枠などで行の番号がずれても、クリックを取りこぼさないようにするため。
    /// 選択モード中なら、どのボタンで、どこをクリックしても選択モードを抜ける（フォーカスするのは左クリックだけ）。
    /// 再描画が必要なときに true を返す。
    fn click(&mut self, mouse: Mouse) -> bool {
        let clicked = matches!(
            mouse,
            Mouse::LeftClick(..) | Mouse::RightClick(..) | Mouse::DoubleClick(..)
        );
        let left = clicked && self.leave();
        if let Mouse::LeftClick(_, col) = mouse
            && let Some(pane_id) = self.line.pane_at(col)
        {
            focus_terminal_pane(pane_id, false, false);
        }
        left
    }

    /// パイプを受け取ったら選択モードに入る。
    /// パイプは全インスタンスに届くので、アクティブなタブにいるインスタンスだけが入る。
    /// 表示中のエージェントがなければ入らない。
    fn enter(&mut self) -> bool {
        if self.selection.is_some() || self.permission != Permission::Granted {
            return false;
        }
        // フォーカスはその場で問い合わせる（イベントから求めると古くなることがある。select::focus を参照）
        let Some(focused) = focus::focused_now() else {
            return false;
        };
        if !self.focus.owns(focused) {
            return false;
        }
        let Some(pane_id) = self.initial_selection(focus::terminal_id(focused)) else {
            return false;
        };
        intercept_key_presses();
        self.selection = Some(Selection { pane_id, focused });
        true
    }

    /// 選択モードを抜ける。横取りも必ずやめる。選択モード中だったときに true を返す
    fn leave(&mut self) -> bool {
        let left = self.selection.take().is_some();
        if left {
            clear_key_presses_intercepts();
        }
        left
    }

    /// 選択モード中に横取りしたキーを処理する。再描画が必要なときに true を返す。
    fn key(&mut self, key: &KeyWithModifier) -> bool {
        if self.selection.is_none() {
            // 選択モードでないのに届いたら、横取りが残っているので念のためやめる
            clear_key_presses_intercepts();
            return false;
        }
        // 選択中にタブが切り替わっても、バックグラウンドになったインスタンスには TabUpdate が届かず気づけない。
        // キーが届いた時点で確かめ、自分のタブがアクティブでなくなっていたら、そのキーは捨てて選択モードを抜ける
        if !focus::focused_now().is_some_and(|pane| self.focus.owns(pane)) {
            return self.leave();
        }
        let Some(selection) = &mut self.selection else {
            return false;
        };
        match KeyAction::from_key(key) {
            None => false,
            Some(action @ (KeyAction::Prev | KeyAction::Next)) => {
                let pane_id = select::step(&self.line.pane_ids(), selection.pane_id, action);
                let changed = pane_id != selection.pane_id;
                selection.pane_id = pane_id;
                changed
            }
            Some(KeyAction::Confirm) => {
                let pane_id = selection.pane_id;
                self.leave();
                focus_terminal_pane(pane_id, false, false);
                true
            }
            Some(KeyAction::Cancel) => self.leave(),
        }
    }

    /// 選択モード中にフォーカスが移ったら（ほかのペインやタブがクリックされたときなど）、選択モードを抜ける。
    /// 入ったときと同じく、フォーカスはその場で問い合わせて比べる。問い合わせに失敗したときも抜ける
    fn leave_on_focus_change(&mut self) -> bool {
        let Some(selection) = &self.selection else {
            return false;
        };
        if focus::focused_now() == Some(selection.focused) {
            return false;
        }
        self.leave()
    }

    /// 選んでいたエージェントが行から消えたら、最初に選ぶときと同じ順で選び直す。
    /// 表示中のエージェントがなくなったら、選択モードを抜ける。
    fn keep_selection_visible(&mut self) {
        let Some(selection) = &self.selection else {
            return;
        };
        if self.line.pane_ids().contains(&selection.pane_id) {
            return;
        }
        let focused = focus::focused_now().and_then(focus::terminal_id);
        match self.initial_selection(focused) {
            Some(pane_id) => {
                if let Some(selection) = &mut self.selection {
                    selection.pane_id = pane_id;
                }
            }
            None => {
                self.leave();
            }
        }
    }

    /// 選択モードで最初に選ぶエージェント（[`select::initial`]）。`focused` はフォーカス中のターミナルペイン
    fn initial_selection(&self, focused: Option<u32>) -> Option<u32> {
        let attention: HashSet<u32> = self
            .collector
            .statuses()
            .filter(|s| s.state.needs_attention())
            .map(|s| s.pane_id)
            .collect();
        select::initial(
            &self.line.pane_ids(),
            |pane_id| attention.contains(&pane_id),
            focused,
        )
    }

    fn line(&self, cols: usize) -> Line {
        if let Some(text) = permission_message(self.permission) {
            return self.message(text, cols);
        }

        let options = &self.config.view;
        let statuses: Vec<AgentStatus> = self.collector.statuses().collect();
        // 1 行しかないので、警告はエージェント一覧の表示を邪魔しない「空のとき」だけ出す
        let empty = !statuses.iter().any(|s| options.shows(s));
        if empty && let Some(warning) = self.warnings.first() {
            return self.message(warning, cols);
        }
        view::render_line(&statuses, cols, options)
    }

    fn message(&self, text: &str, cols: usize) -> Line {
        view::render_message(&format!("{MESSAGE_PREFIX}{text}"), cols)
    }
}

/// permission が得られていないときに、エージェント一覧の代わりに出すメッセージ。
fn permission_message(permission: Permission) -> Option<&'static str> {
    match permission {
        // 許可ダイアログはこのペインに出る。ダイアログが出ている間はこの表示は見えない
        Permission::Pending => Some("permission の応答を待っています"),
        Permission::Denied => Some("必要な permission が許可されていないため表示できません"),
        Permission::Granted => None,
    }
}
