//! 選択モードの状態と遷移。
//!
//! Zellij の API は呼ばない。判断の材料（行の並びやフォーカスなど）を受け取り、すべきことを [`Effect`] で返す。
//! API の呼び出しは、返した [`Effect`] に従って `main.rs` が行う。
//!
//! フォーカスなど Zellij への問い合わせが要る材料は、値ではなくクロージャで受け取り、選択モード中に必要なときだけ呼ぶ。
//! `PaneUpdate` などは選択モードでなくても頻繁に届くので、そのたびに問い合わせないようにするため。

use zellij_tile::prelude::*;

use super::focus::terminal_id;
use super::{KeyAction, initial, step};

/// 選択モード
#[derive(Debug, Default)]
pub struct SelectMode {
    /// 選択モード中なら Some
    selection: Option<Selection>,
}

/// 選択モード中の状態
#[derive(Debug)]
struct Selection {
    /// 選んでいるエージェントのペインの id
    pane_id: u32,
    /// 選択モードに入ったときにフォーカスしていたペイン（[`focused_now`](super::focus::focused_now)）。
    /// ほかのペインやタブがクリックされるなどして変わったら、選択モードを抜ける
    focused: PaneId,
}

/// 遷移の結果、呼び出し側がすること。[`Effect::None`] 以外では再描画も必要
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// 何もしない
    None,
    /// 再描画だけする（選択が動いた）
    Redraw,
    /// キーの横取りを始める（選択モードに入った）
    Enter,
    /// キーの横取りをやめる（選択モードを抜けた）
    Leave,
    /// キーの横取りをやめ、このペインにフォーカスする（選んで選択モードを抜けた）
    Confirm(u32),
}

impl SelectMode {
    pub fn is_active(&self) -> bool {
        self.selection.is_some()
    }

    /// 選んでいるエージェントのペインの id。選択モードでなければ None
    pub fn selected(&self) -> Option<u32> {
        self.selection.as_ref().map(|s| s.pane_id)
    }

    /// 選択モードに入る。`focused` は今フォーカスしているペイン、`order` は表示順のペインの id。
    /// 最初に選ぶエージェントは [`initial`] で決める。表示中のエージェントがなければ入らない。
    /// すでに選択モード中なら何もしない。
    pub fn enter(
        &mut self,
        focused: PaneId,
        order: &[u32],
        needs_attention: impl Fn(u32) -> bool,
    ) -> Effect {
        if self.is_active() {
            return Effect::None;
        }
        let Some(pane_id) = initial(order, needs_attention, terminal_id(focused)) else {
            return Effect::None;
        };
        self.selection = Some(Selection { pane_id, focused });
        Effect::Enter
    }

    /// 選択モードを抜ける。選択モードでなければ何もしない
    pub fn leave(&mut self) -> Effect {
        match self.selection.take() {
            Some(_) => Effect::Leave,
            None => Effect::None,
        }
    }

    /// 横取りしたキーを処理する。`tab_is_active` は、このプラグインのタブがアクティブか。
    pub fn key(
        &mut self,
        key: &KeyWithModifier,
        order: &[u32],
        tab_is_active: impl FnOnce() -> bool,
    ) -> Effect {
        let Some(selection) = &mut self.selection else {
            // 選択モードでないのに届いたら、横取りが残っているので念のためやめる
            return Effect::Leave;
        };
        // 選択中にタブが切り替わっても、バックグラウンドになったインスタンスには TabUpdate が届かず気づけない。
        // キーが届いた時点で確かめ、自分のタブがアクティブでなくなっていたら、そのキーは捨てて選択モードを抜ける
        if !tab_is_active() {
            return self.leave();
        }
        match KeyAction::from_key(key) {
            None => Effect::None,
            Some(KeyAction::Move(to)) => {
                let pane_id = step(order, selection.pane_id, to);
                if pane_id == selection.pane_id {
                    return Effect::None;
                }
                selection.pane_id = pane_id;
                Effect::Redraw
            }
            Some(KeyAction::Confirm) => {
                let pane_id = selection.pane_id;
                self.selection = None;
                Effect::Confirm(pane_id)
            }
            Some(KeyAction::Cancel) => self.leave(),
        }
    }

    /// フォーカスが移ったかもしれないときに呼ぶ。選択モード中にフォーカスが移ったら
    /// （ほかのペインやタブがクリックされたときなど）、選択モードを抜ける。
    /// `focused` は今フォーカスしているペイン。問い合わせに失敗した（None の）ときも抜ける
    pub fn on_focus(&mut self, focused: impl FnOnce() -> Option<PaneId>) -> Effect {
        let Some(selection) = &self.selection else {
            return Effect::None;
        };
        if focused() == Some(selection.focused) {
            return Effect::None;
        }
        self.leave()
    }

    /// 行の内容が変わったときに呼ぶ。選んでいたエージェントが行から消えたら、最初に選ぶときと同じ順で選び直す。
    /// 表示中のエージェントがなくなったら、選択モードを抜ける。
    pub fn keep_visible(
        &mut self,
        order: &[u32],
        needs_attention: impl Fn(u32) -> bool,
        focused: impl FnOnce() -> Option<PaneId>,
    ) -> Effect {
        let Some(selection) = &mut self.selection else {
            return Effect::None;
        };
        if order.contains(&selection.pane_id) {
            return Effect::None;
        }
        match initial(order, needs_attention, focused().and_then(terminal_id)) {
            Some(pane_id) => {
                selection.pane_id = pane_id;
                Effect::Redraw
            }
            None => self.leave(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    const ORDER: [u32; 3] = [3, 1, 4];
    /// 選択モードに入ったときにフォーカスしていたペイン
    const FOCUSED: PaneId = PaneId::Terminal(1);

    fn calm(_: u32) -> bool {
        false
    }

    /// フォーカス中のペイン 1 を選んでいる選択モード
    fn active() -> SelectMode {
        let mut mode = SelectMode::default();
        assert_eq!(mode.enter(FOCUSED, &ORDER, calm), Effect::Enter);
        assert_eq!(mode.selected(), Some(1));
        mode
    }

    fn press(mode: &mut SelectMode, order: &[u32], key: &str) -> Effect {
        mode.key(&KeyWithModifier::from_str(key).unwrap(), order, || true)
    }

    #[test]
    fn enter_selects_initial_agent() {
        let mut mode = SelectMode::default();
        assert!(!mode.is_active());
        assert_eq!(mode.selected(), None);
        assert_eq!(mode.enter(FOCUSED, &ORDER, |id| id == 4), Effect::Enter);
        assert!(mode.is_active());
        assert_eq!(mode.selected(), Some(4));

        // フォーカス中のペインがプラグインなら、同じ番号のターミナルペインとは別なので、左端になる
        let mut mode = SelectMode::default();
        assert_eq!(mode.enter(PaneId::Plugin(1), &ORDER, calm), Effect::Enter);
        assert_eq!(mode.selected(), Some(3));
    }

    #[test]
    fn enter_does_nothing_without_agents_or_when_active() {
        let mut mode = SelectMode::default();
        assert_eq!(mode.enter(FOCUSED, &[], calm), Effect::None);
        assert!(!mode.is_active());

        let mut mode = active();
        assert_eq!(mode.enter(FOCUSED, &ORDER, |id| id == 4), Effect::None);
        assert_eq!(mode.selected(), Some(1));
    }

    #[test]
    fn leave_only_when_active() {
        let mut mode = active();
        assert_eq!(mode.leave(), Effect::Leave);
        assert!(!mode.is_active());
        assert_eq!(mode.leave(), Effect::None);
    }

    #[test]
    fn key_moves_selection() {
        let mut mode = active();
        assert_eq!(press(&mut mode, &ORDER, "Right"), Effect::Redraw);
        assert_eq!(mode.selected(), Some(4));
        assert_eq!(press(&mut mode, &ORDER, "Left"), Effect::Redraw);
        assert_eq!(mode.selected(), Some(1));
        // 1 件だけなら動かないので、再描画しない
        assert_eq!(press(&mut mode, &[1], "Right"), Effect::None);
        assert_eq!(mode.selected(), Some(1));
    }

    #[test]
    fn key_confirms_or_cancels() {
        let mut mode = active();
        assert_eq!(press(&mut mode, &ORDER, "Right"), Effect::Redraw);
        assert_eq!(press(&mut mode, &ORDER, "Enter"), Effect::Confirm(4));
        assert!(!mode.is_active());

        let mut mode = active();
        assert_eq!(press(&mut mode, &ORDER, "Esc"), Effect::Leave);
        assert!(!mode.is_active());
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut mode = active();
        assert_eq!(press(&mut mode, &ORDER, "a"), Effect::None);
        assert_eq!(mode.selected(), Some(1));
    }

    #[test]
    fn key_leaves_when_tab_is_not_active() {
        // 決定のキーでも、フォーカスせずに抜けるだけにする
        for key in ["Right", "Enter", "a"] {
            let mut mode = active();
            let key = KeyWithModifier::from_str(key).unwrap();
            assert_eq!(mode.key(&key, &ORDER, || false), Effect::Leave);
            assert!(!mode.is_active());
        }
    }

    #[test]
    fn key_stops_interception_when_not_active() {
        let mut mode = SelectMode::default();
        assert_eq!(press(&mut mode, &ORDER, "Enter"), Effect::Leave);
    }

    #[test]
    fn leaves_when_focus_changes() {
        let mut mode = active();
        assert_eq!(mode.on_focus(|| Some(FOCUSED)), Effect::None);
        assert!(mode.is_active());

        // 別のペイン、番号が同じプラグインのペイン、問い合わせの失敗
        for focused in [Some(PaneId::Terminal(3)), Some(PaneId::Plugin(1)), None] {
            let mut mode = active();
            assert_eq!(mode.on_focus(|| focused), Effect::Leave, "{focused:?}");
            assert!(!mode.is_active());
        }
    }

    #[test]
    fn keep_visible_reselects_only_when_selected_agent_disappears() {
        // 並びが変わっても、残っていれば同じエージェントを選び続ける
        let mut mode = active();
        let order = [4, 3, 1];
        assert_eq!(
            mode.keep_visible(&order, |id| id == 4, || Some(FOCUSED)),
            Effect::None
        );
        assert_eq!(mode.selected(), Some(1));

        // 消えたら、最初に選ぶときと同じ順で選び直す（注意が必要 → フォーカス中 → 左端）
        let order = [3, 4];
        let focused = || Some(PaneId::Terminal(4));
        assert_eq!(
            mode.keep_visible(&order, |id| id == 3, focused),
            Effect::Redraw
        );
        assert_eq!(mode.selected(), Some(3));

        let mut mode = active();
        assert_eq!(mode.keep_visible(&order, calm, focused), Effect::Redraw);
        assert_eq!(mode.selected(), Some(4));

        let mut mode = active();
        assert_eq!(mode.keep_visible(&order, calm, || None), Effect::Redraw);
        assert_eq!(mode.selected(), Some(3));
    }

    #[test]
    fn keep_visible_leaves_without_agents() {
        let mut mode = active();
        assert_eq!(
            mode.keep_visible(&[], calm, || Some(FOCUSED)),
            Effect::Leave
        );
        assert!(!mode.is_active());
    }

    /// 選択モードでないとき、選び直しが要らないときは、Zellij に問い合わせない
    #[test]
    fn focus_is_asked_only_when_needed() {
        let unexpected = || -> Option<PaneId> { panic!("問い合わせは不要") };
        let mut mode = SelectMode::default();
        assert_eq!(mode.on_focus(unexpected), Effect::None);
        assert_eq!(mode.keep_visible(&ORDER, calm, unexpected), Effect::None);
        let key = KeyWithModifier::from_str("Enter").unwrap();
        let effect = mode.key(&key, &ORDER, || panic!("問い合わせは不要"));
        assert_eq!(effect, Effect::Leave);

        assert_eq!(
            active().keep_visible(&ORDER, calm, unexpected),
            Effect::None
        );
    }
}
