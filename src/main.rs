mod collect;
mod config;
mod model;
mod permission;
mod select;
mod view;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use collect::Collector;
use config::Config;
use model::{AgentState, AgentStatus};
use permission::Permission;
use select::focus::{self, Focus};
use select::{Effect, PIPE_NAME, SelectMode};
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
    select: SelectMode,
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
                    && run(self.select.on_focus(focus::focused_now));
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
        // 選べるエージェントは行の内容で決まるので、選び直しはここで行う。
        // Zellij の API は標準出力で命令を送るので、行の出力より前に済ませる
        run(self.select.keep_visible(
            &self.line.pane_ids(),
            needs_attention(&self.collector),
            focus::focused_now,
        ));
        print!("{}", self.line.paint(&self.theme, self.select.selected()));
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
        let left = clicked && run(self.select.leave());
        if let Mouse::LeftClick(_, col) = mouse
            && let Some(pane_id) = self.line.pane_at(col)
        {
            focus_terminal_pane(pane_id, false, false);
        }
        left
    }

    /// パイプを受け取ったら選択モードに入る。
    /// パイプは全インスタンスに届くので、アクティブなタブにいるインスタンスだけが入る。
    /// 再描画が必要なときに true を返す。
    fn enter(&mut self) -> bool {
        if self.select.is_active() || self.permission != Permission::Granted {
            return false;
        }
        // フォーカスはその場で問い合わせる（イベントから求めると古くなることがある。select::focus を参照）
        let Some(focused) = focus::focused_now().filter(|&pane| self.focus.owns(pane)) else {
            return false;
        };
        run(self.select.enter(
            focused,
            &self.line.pane_ids(),
            needs_attention(&self.collector),
        ))
    }

    /// 選択モード中に横取りしたキーを処理する。再描画が必要なときに true を返す。
    fn key(&mut self, key: &KeyWithModifier) -> bool {
        run(self.select.key(key, &self.line.pane_ids(), || {
            focus::focused_now().is_some_and(|pane| self.focus.owns(pane))
        }))
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

/// 選択モードの遷移の結果に従って、Zellij の API を呼ぶ。再描画が必要なときに true を返す。
/// キーの横取りを始めるのもやめるのも、ここだけで行う。
fn run(effect: Effect) -> bool {
    match effect {
        Effect::None => return false,
        Effect::Redraw => {}
        Effect::Enter => intercept_key_presses(),
        Effect::Leave => clear_key_presses_intercepts(),
        Effect::Confirm(pane_id) => {
            clear_key_presses_intercepts();
            focus_terminal_pane(pane_id, false, false);
        }
    }
    true
}

/// ペインのエージェントが、注意が必要な状態か（[`AgentState::needs_attention`]）の判定
fn needs_attention(collector: &Collector) -> impl Fn(u32) -> bool + '_ {
    |pane_id| {
        collector
            .state(pane_id)
            .is_some_and(AgentState::needs_attention)
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
