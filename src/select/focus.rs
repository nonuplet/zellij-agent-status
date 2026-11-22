//! フォーカスの判定：このプラグインのタブがアクティブか、フォーカス中のペインはどれか。
//!
//! キー操作での選択（[`crate::select`]）に使う。
//! - パイプは全インスタンスに届くので、アクティブなタブにいるインスタンスだけが選択モードに入る。
//!   プラグインがタブごとに起動している場合に、複数のインスタンスが同時にキーを横取りしないようにするため
//! - 選択モード中に、ほかのペインやタブがクリックされるなどしてフォーカスが移ったら、選択モードを抜ける
//!
//! ## フォーカスはその場で問い合わせる
//!
//! フォーカス中のペインは、イベント（`PaneUpdate` / `TabUpdate`）から求めず、使うたびに `get_focused_pane_info` で
//! 問い合わせる（[`focused_now`]）。イベントから求めた値は、次の理由で古くなったりずれたりするため。
//! - バックグラウンドのタブにいるインスタンスには `TabUpdate` が届かない（Zellij 0.46 で、ログから確かめた）。
//!   アクティブなタブは、タブを離れた時点のまま古くなる
//! - タブの位置は、左のタブが閉じられたり動かされたりするとずれる。`PaneUpdate` と `TabUpdate` は届く順序が
//!   決まっていないので、一時的に別のタブの情報を引くことがある
//!
//! このプラグインのタブがアクティブかは、問い合わせたペインがこのプラグインのタブにあるか（[`Focus::owns`]）で決める。
//! `get_focused_pane_info` が返すタブの番号は、位置（`TabInfo.position`）か安定した id（`TabInfo.tab_id`）かが
//! はっきりしないので使わない。
//!
//! このプラグインのタブにあるペインは、`PaneUpdate` の中から自分のプラグイン id（`get_plugin_ids`）のペインを
//! 含むタブを探して求める。ペインの一覧はタブの位置に依存しないので、位置がずれても影響を受けない。

use zellij_tile::prelude::*;

#[derive(Debug, Default)]
pub struct Focus {
    /// このプラグインの id。テストなどで不明なら None
    plugin_id: Option<u32>,
    /// このプラグインのペインがあるタブの、すべてのペイン（このプラグイン自身を含む）
    own_panes: Vec<PaneId>,
}

impl Focus {
    pub fn new(plugin_id: u32) -> Self {
        Focus {
            plugin_id: Some(plugin_id),
            ..Default::default()
        }
    }

    /// `PaneUpdate` を受け取ったときに呼ぶ。ほかのイベントは無視する。
    pub fn update(&mut self, event: &Event) {
        let (Event::PaneUpdate(manifest), Some(plugin_id)) = (event, self.plugin_id) else {
            return;
        };
        self.own_panes = manifest
            .panes
            .values()
            .find(|panes| panes.iter().any(|p| p.is_plugin && p.id == plugin_id))
            .map(|panes| panes.iter().map(pane_id).collect())
            .unwrap_or_default();
    }

    /// `pane` がこのプラグインのタブにあるか。[`focused_now`] と合わせて、このプラグインのタブがアクティブかを判定する。
    /// まだ `PaneUpdate` が届いていなければ false
    pub fn owns(&self, pane: PaneId) -> bool {
        self.own_panes.contains(&pane)
    }
}

/// このプラグインのクライアントで、今フォーカスしているペイン（`get_focused_pane_info` で問い合わせる）。
/// 取得できなければ None
pub fn focused_now() -> Option<PaneId> {
    get_focused_pane_info()
        .inspect_err(|e| {
            eprintln!(
                "{}フォーカス中のペインを取得できません: {e}",
                crate::MESSAGE_PREFIX
            )
        })
        .ok()
        .map(|(_, pane)| pane)
}

/// ターミナルペインなら、その id
pub fn terminal_id(pane: PaneId) -> Option<u32> {
    match pane {
        PaneId::Terminal(id) => Some(id),
        PaneId::Plugin(_) => None,
    }
}

fn pane_id(pane: &PaneInfo) -> PaneId {
    if pane.is_plugin {
        PaneId::Plugin(pane.id)
    } else {
        PaneId::Terminal(pane.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u32, is_plugin: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin,
            ..Default::default()
        }
    }

    fn pane_update(tabs: Vec<(usize, Vec<PaneInfo>)>) -> Event {
        Event::PaneUpdate(PaneManifest {
            panes: tabs.into_iter().collect(),
        })
    }

    fn two_tabs() -> Event {
        pane_update(vec![
            (0, vec![pane(1, false), pane(7, true), pane(2, false)]),
            (1, vec![pane(3, false), pane(8, true)]),
        ])
    }

    #[test]
    fn owns_only_panes_of_own_tab() {
        let mut focus = Focus::new(8);
        // PaneUpdate が届くまでは、どのペインも自分のタブのものとみなさない
        assert!(!focus.owns(PaneId::Terminal(3)));

        focus.update(&two_tabs());
        assert!(focus.owns(PaneId::Terminal(3)));
        // 自分自身（ステータス行をクリックしてフォーカスしたとき）
        assert!(focus.owns(PaneId::Plugin(8)));
        assert!(!focus.owns(PaneId::Terminal(1)));
        assert!(!focus.owns(PaneId::Plugin(7)));

        // 自分のペインが見つからなければ、どのペインも自分のタブのものとみなさない
        let mut focus = Focus::new(99);
        focus.update(&two_tabs());
        assert!(!focus.owns(PaneId::Terminal(1)));
        assert!(!focus.owns(PaneId::Terminal(3)));
    }

    #[test]
    fn tab_position_does_not_matter() {
        // 左のタブが閉じられて、自分のタブの位置が 1 から 0 にずれても、同じペインを持つ
        let mut focus = Focus::new(8);
        focus.update(&two_tabs());
        focus.update(&pane_update(vec![(0, vec![pane(3, false), pane(8, true)])]));
        assert!(focus.owns(PaneId::Terminal(3)));

        // TabUpdate は見ない
        focus.update(&Event::TabUpdate(vec![TabInfo::default()]));
        assert!(focus.owns(PaneId::Terminal(3)));
    }

    #[test]
    fn terminal_id_of_pane() {
        assert_eq!(terminal_id(PaneId::Terminal(3)), Some(3));
        assert_eq!(terminal_id(PaneId::Plugin(3)), None);
    }
}
