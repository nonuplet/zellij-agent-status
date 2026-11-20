mod collect;
mod model;
mod probe;
mod view;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    probe: probe::Probe,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // T0 検証中は permission ダイアログに応答できるよう、set_selectable(false) にしない
        self.probe = probe::Probe::new();
        request_permission(&[PermissionType::ReadApplicationState]);
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::PaneAttentionChanged,
            EventType::GlobalAttentionUpdate,
            EventType::Timer,
        ]);
        set_timeout(probe::TICK_SECS);
    }

    fn update(&mut self, event: Event) -> bool {
        self.probe.on_event(&event)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.probe.render(rows, cols);
    }
}
