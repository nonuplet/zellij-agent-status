mod collect;
mod config;
mod model;
mod permission;
mod view;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use collect::Collector;
use config::Config;
use model::AgentStatus;
use permission::Permission;
use view::{Line, Theme};

/// 表示するメッセージとログ（`eprintln!`）の先頭に付ける、どのプラグインからのものかを示す名前
const MESSAGE_PREFIX: &str = "zellij-agent-status: ";

#[derive(Default)]
struct State {
    permission: Permission,
    collector: Collector,
    config: Config,
    /// 設定の警告。エージェントがいないときに表示する
    warnings: Vec<String>,
    theme: Theme,
    /// 最後に描画した行。クリックされた列のエージェントを引くのに使う
    line: Line,
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

        // 許可が出るまで selectable のままにする処理は Permission が持つ
        Permission::request();
        Collector::subscribe();
        subscribe(&[EventType::ModeUpdate, EventType::Mouse]);
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
            Event::Mouse(mouse) => {
                self.click(*mouse);
                false
            }
            _ => self.collector.update(&event),
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 {
            return;
        }
        self.line = self.line(cols);
        print!("{}", self.line.paint(&self.theme));
    }
}

impl State {
    /// 描画した行のエージェントを左クリックしたら、そのペインにフォーカスする。
    /// `focus_terminal_pane` は、ペインが別のタブにあればそのタブにも切り替える。
    /// 描画するのは 1 行目だけなので、2 行目以降のクリックは無視する。
    fn click(&self, mouse: Mouse) {
        let Mouse::LeftClick(0, col) = mouse else {
            return;
        };
        if let Some(pane_id) = self.line.pane_at(col) {
            focus_terminal_pane(pane_id, false, false);
        }
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
        Permission::Denied => Some("permission が許可されていないため表示できません"),
        Permission::Granted => None,
    }
}
