//! フォーカスの追跡：このプラグインのタブ、アクティブなタブ、フォーカス中のペイン。
//!
//! キー操作での選択（[`crate::select`]）に使う。
//! - パイプは全インスタンスに届くので、アクティブなタブにいるインスタンスだけが選択モードに入る
//!   （[`Focus::owns`]）。プラグインがタブごとに起動している場合に、複数のインスタンスが
//!   同時にキーを横取りしないようにするため
//! - 選択モード中に、ほかのペインやタブがクリックされるなどしてフォーカスが移ったら、選択モードを抜ける（[`Focus::focused`]）
//!
//! ## アクティブなタブにいるかの判定
//!
//! バックグラウンドのタブにいるインスタンスには `TabUpdate` が届かない（Zellij 0.46 で、ログから確かめた）。
//! そのため、イベントから求めたアクティブなタブは、タブを離れた時点のまま古くなる。
//! 選択モードに入るかどうかは、その場で `get_focused_pane_info` を呼んでフォーカス中のペインを問い合わせ（[`focused_now`]）、
//! そのペインがこのプラグインのタブにあるか（[`Focus::owns`]）で決める。
//! `get_focused_pane_info` が返すタブの番号は、位置（`TabInfo.position`）か安定した id（`TabInfo.tab_id`）かが
//! はっきりしないので使わない。
//!
//! このプラグインのタブにあるペインは、`PaneUpdate` の中から自分のプラグイン id（`get_plugin_ids`）のペインを
//! 含むタブを探して求める。

use std::collections::HashMap;

use zellij_tile::prelude::*;

/// 1 タブ内でフォーカスしているペイン。タイルとフローティングは別々にフォーカスを持つ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct TabFocus {
    tiled: Option<PaneId>,
    floating: Option<PaneId>,
}

#[derive(Debug, Default)]
pub struct Focus {
    /// このプラグインの id。テストなどで不明なら None
    plugin_id: Option<u32>,
    /// このプラグインのペインがあるタブの、すべてのペイン（このプラグイン自身を含む）
    own_panes: Vec<PaneId>,
    /// `PaneUpdate` から求めた、タブの位置 → フォーカスしているペイン
    panes: HashMap<usize, TabFocus>,
    /// `TabUpdate` から求めた、アクティブなタブの位置
    active_tab: Option<usize>,
    /// `TabUpdate` から求めた、フローティングのペインを表示しているタブの位置
    floating_visible: Vec<usize>,
}

impl Focus {
    pub fn new(plugin_id: u32) -> Self {
        Focus {
            plugin_id: Some(plugin_id),
            ..Default::default()
        }
    }

    /// `PaneUpdate` / `TabUpdate` を受け取ったときに呼ぶ。ほかのイベントは無視する。
    pub fn update(&mut self, event: &Event) {
        match event {
            Event::PaneUpdate(manifest) => {
                self.panes = manifest
                    .panes
                    .iter()
                    .map(|(&tab, panes)| (tab, tab_focus(panes)))
                    .collect();
                if let Some(plugin_id) = self.plugin_id {
                    self.own_panes = manifest
                        .panes
                        .values()
                        .find(|panes| panes.iter().any(|p| p.is_plugin && p.id == plugin_id))
                        .map(|panes| panes.iter().map(pane_id).collect())
                        .unwrap_or_default();
                }
            }
            Event::TabUpdate(tabs) => {
                self.active_tab = tabs.iter().find(|t| t.active).map(|t| t.position);
                self.floating_visible = tabs
                    .iter()
                    .filter(|t| t.are_floating_panes_visible)
                    .map(|t| t.position)
                    .collect();
            }
            _ => {}
        }
    }

    /// `pane` がこのプラグインのタブにあるか。[`focused_now`] と合わせて、このプラグインのタブがアクティブかを判定する。
    /// まだ `PaneUpdate` が届いていなければ false
    pub fn owns(&self, pane: PaneId) -> bool {
        self.own_panes.contains(&pane)
    }

    /// イベントから求めた、アクティブなタブと、そのタブでフォーカスしているペイン。
    /// フローティングのペインを表示していれば、フローティングのほうのフォーカスを使う。
    /// バックグラウンドのタブにいるインスタンスでは古くなる（モジュールの説明を参照）
    pub fn focused(&self) -> (Option<usize>, Option<PaneId>) {
        let pane = self.active_tab.and_then(|tab| {
            let focus = self.panes.get(&tab)?;
            if self.floating_visible.contains(&tab) {
                focus.floating
            } else {
                focus.tiled
            }
        });
        (self.active_tab, pane)
    }

    /// フォーカスしているターミナルペインの id
    pub fn focused_terminal(&self) -> Option<u32> {
        match self.focused().1 {
            Some(PaneId::Terminal(id)) => Some(id),
            _ => None,
        }
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

fn pane_id(pane: &PaneInfo) -> PaneId {
    if pane.is_plugin {
        PaneId::Plugin(pane.id)
    } else {
        PaneId::Terminal(pane.id)
    }
}

fn tab_focus(panes: &[PaneInfo]) -> TabFocus {
    let focused = |floating: bool| {
        panes
            .iter()
            .find(|p| p.is_focused && p.is_floating == floating && !p.is_suppressed)
            .map(pane_id)
    };
    TabFocus {
        tiled: focused(false),
        floating: focused(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u32, is_plugin: bool, is_focused: bool, is_floating: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin,
            is_focused,
            is_floating,
            ..Default::default()
        }
    }

    fn pane_update(tabs: Vec<(usize, Vec<PaneInfo>)>) -> Event {
        Event::PaneUpdate(PaneManifest {
            panes: tabs.into_iter().collect(),
        })
    }

    fn tab_update(active: usize, floating_visible: &[usize], count: usize) -> Event {
        Event::TabUpdate(
            (0..count)
                .map(|position| TabInfo {
                    position,
                    active: position == active,
                    are_floating_panes_visible: floating_visible.contains(&position),
                    ..Default::default()
                })
                .collect(),
        )
    }

    fn two_tabs() -> Event {
        pane_update(vec![
            (
                0,
                vec![
                    pane(1, false, true, false),
                    pane(7, true, false, false),
                    pane(2, false, true, true),
                ],
            ),
            (
                1,
                vec![pane(3, false, true, false), pane(8, true, false, false)],
            ),
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

        // TabUpdate（アクティブなタブ）には左右されない。
        // バックグラウンドのタブには TabUpdate が届かず、古くなるため
        focus.update(&tab_update(0, &[], 2));
        assert!(focus.owns(PaneId::Terminal(3)));
        assert!(!focus.owns(PaneId::Terminal(1)));

        // 自分のペインが見つからなければ、どのペインも自分のタブのものとみなさない
        let mut focus = Focus::new(99);
        focus.update(&two_tabs());
        assert!(!focus.owns(PaneId::Terminal(1)));
        assert!(!focus.owns(PaneId::Terminal(3)));
    }

    #[test]
    fn focused_pane_follows_active_tab_and_floating_layer() {
        let mut focus = Focus::new(8);
        assert_eq!(focus.focused(), (None, None));

        focus.update(&two_tabs());
        focus.update(&tab_update(0, &[], 2));
        assert_eq!(focus.focused(), (Some(0), Some(PaneId::Terminal(1))));
        assert_eq!(focus.focused_terminal(), Some(1));

        // フローティングを表示していれば、フローティングのフォーカス
        focus.update(&tab_update(0, &[0], 2));
        assert_eq!(focus.focused_terminal(), Some(2));

        focus.update(&tab_update(1, &[0], 2));
        assert_eq!(focus.focused(), (Some(1), Some(PaneId::Terminal(3))));
    }

    #[test]
    fn focused_plugin_pane_is_not_a_terminal() {
        let mut focus = Focus::new(8);
        focus.update(&pane_update(vec![(0, vec![pane(5, true, true, false)])]));
        focus.update(&tab_update(0, &[], 1));
        assert_eq!(focus.focused(), (Some(0), Some(PaneId::Plugin(5))));
        assert_eq!(focus.focused_terminal(), None);
    }
}
