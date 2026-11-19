use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct State;

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // ステータスバー用なのでフォーカスできないようにする
        set_selectable(false);
    }

    fn update(&mut self, _event: Event) -> bool {
        false // 再描画不要
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        let text = " Claude: Hello, world!";
        // 幅を超えないよう切り詰める
        let line: String = text.chars().take(cols).collect();
        print!("{}", line);
    }
}