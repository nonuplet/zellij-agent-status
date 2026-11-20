mod collect;
mod config;
mod model;
mod view;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use collect::{Collector, Permission};
use config::Config;
use view::Theme;

#[derive(Default)]
struct State {
    collector: Collector,
    config: Config,
    /// 設定の警告。エージェントがいないときに表示する
    warnings: Vec<String>,
    theme: Theme,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        let (config, warnings) = Config::from_configuration(&configuration);
        for warning in &warnings {
            eprintln!("zellij-agent-status: {warning}");
        }
        self.config = config;
        self.warnings = warnings;

        // permission の要求と、許可が出るまで selectable のままにする処理は Collector が持つ
        self.collector.load(self.config.view.show_cwd);
        subscribe(&[EventType::ModeUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        match &event {
            Event::ModeUpdate(mode_info) => {
                let theme = Theme::from_styling(&mode_info.style.colors);
                let changed = theme != self.theme;
                self.theme = theme;
                changed
            }
            _ => self.collector.update(&event),
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 {
            return;
        }
        print!("{}", self.line(cols));
    }
}

/// メッセージの先頭に付ける、どのプラグインからの表示かを示す名前
const MESSAGE_PREFIX: &str = "zellij-agent-status: ";

impl State {
    fn line(&self, cols: usize) -> String {
        if let Some(text) = permission_message(self.collector.permission()) {
            return self.message(text, cols);
        }

        let options = &self.config.view;
        // 1 行しかないので、警告はエージェント一覧の表示を邪魔しない「空のとき」だけ出す
        let empty = !self.collector.statuses().any(|s| options.shows(s));
        if empty && let Some(warning) = self.warnings.first() {
            return self.message(warning, cols);
        }
        view::render_line(self.collector.statuses(), cols, &self.theme, options)
    }

    fn message(&self, text: &str, cols: usize) -> String {
        view::render_message(&format!("{MESSAGE_PREFIX}{text}"), cols, &self.theme)
    }
}

/// permission が得られていないときに、エージェント一覧の代わりに出すメッセージ。
fn permission_message(permission: Permission) -> Option<&'static str> {
    match permission {
        // 許可ダイアログはこのペインに出る。ダイアログが出ている間はこの表示は見えない
        Permission::Pending => Some("permission の応答を待っています"),
        Permission::Denied => Some("ReadApplicationState が許可されていないため表示できません"),
        Permission::Granted => None,
    }
}
