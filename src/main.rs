mod collect;
mod config;
mod model;
mod view;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use collect::{Collector, Permission};
use config::Config;
use model::AgentStatus;
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
        self.collector.load();
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

impl State {
    fn line(&self, cols: usize) -> String {
        match self.collector.permission() {
            // 許可ダイアログはこのペインに出る。ダイアログが出ている間はこの表示は見えない
            Permission::Pending => {
                return view::render_message(
                    "zellij-agent-status: permission の応答を待っています",
                    cols,
                    &self.theme,
                );
            }
            Permission::Denied => {
                return view::render_message(
                    "zellij-agent-status: ReadApplicationState が許可されていないため表示できません",
                    cols,
                    &self.theme,
                );
            }
            Permission::Granted => {}
        }

        let agents: Vec<AgentStatus> = self
            .collector
            .statuses()
            .iter()
            .filter(|s| self.config.accepts_app(s.app.as_deref()))
            .cloned()
            .collect();
        // 1 行しかないので、警告はエージェント一覧の表示を邪魔しない「空のとき」だけ出す
        if agents.is_empty()
            && let Some(warning) = self.warnings.first()
        {
            return view::render_message(
                &format!("zellij-agent-status: {warning}"),
                cols,
                &self.theme,
            );
        }
        view::render_line(&agents, cols, &self.theme, &self.config.view)
    }
}
