//! キー操作でのエージェントの選択（選択モード）。
//!
//! 選択モードには、キーバインド（[`keybind`]）から届くパイプで入る。入ってからは
//! `intercept_key_presses` でキーを横取りし、[`KeyAction`] に従って選択を動かす（API の呼び出しは `main.rs`）。
//! ここには、そのうち Zellij の API を呼ばない部分（キーの意味・最初の選択・選択の移動）を置く。
//!
//! 選択は位置ではなくペインの id で持つ。選択中にエージェントが増減したり並びが変わったりしても、
//! 同じエージェントを選び続けるようにするため。選べるのは、その時点の行に表示されているエージェント
//! （[`Line::pane_ids`](crate::view::Line::pane_ids)）だけにする。
//!
//! - [`keybind`]：選択モードに入るキーの登録
//! - [`focus`]：このプラグインのタブ、アクティブなタブ、フォーカス中のペインの追跡

pub mod focus;
pub mod keybind;

use zellij_tile::prelude::*;

/// 選択モード中のキーの意味
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// 左（前）のエージェントを選ぶ
    Prev,
    /// 右（次）のエージェントを選ぶ
    Next,
    /// 選んだエージェントのペインにフォーカスし、選択モードを抜ける
    Confirm,
    /// 何もせずに選択モードを抜ける
    Cancel,
}

impl KeyAction {
    /// 横取りしたキーの意味。None のキーは無視する（選択モードは続く）
    pub fn from_key(key: &KeyWithModifier) -> Option<Self> {
        let plain = |bare| key.is_key_without_modifier(bare);
        if plain(BareKey::Left) || plain(BareKey::Char('h')) {
            Some(KeyAction::Prev)
        } else if plain(BareKey::Right) || plain(BareKey::Char('l')) {
            Some(KeyAction::Next)
        } else if plain(BareKey::Enter) {
            Some(KeyAction::Confirm)
        } else if plain(BareKey::Esc) || key.is_key_with_ctrl_modifier(BareKey::Char('c')) {
            Some(KeyAction::Cancel)
        } else {
            None
        }
    }
}

/// 選択モードに入ったときに最初に選ぶエージェント。`order` は表示順のペインの id。
/// 1. 注意が必要な状態のエージェント（複数あれば左端）
/// 2. フォーカス中のペインのエージェント
/// 3. 左端のエージェント
///
/// 表示中のエージェントがなければ None（選択モードに入らない）。
pub fn initial(
    order: &[u32],
    needs_attention: impl Fn(u32) -> bool,
    focused: Option<u32>,
) -> Option<u32> {
    order
        .iter()
        .copied()
        .find(|&id| needs_attention(id))
        .or_else(|| focused.filter(|id| order.contains(id)))
        .or_else(|| order.first().copied())
}

/// `current` から左右に 1 つ動かす。端まで来たら反対側に戻る。
/// `current` が表示されていなければ、そのまま返す（呼び出し側で選び直す）。
pub fn step(order: &[u32], current: u32, action: KeyAction) -> u32 {
    let Some(index) = order.iter().position(|&id| id == current) else {
        return current;
    };
    let len = order.len();
    let index = match action {
        KeyAction::Prev => (index + len - 1) % len,
        KeyAction::Next => (index + 1) % len,
        KeyAction::Confirm | KeyAction::Cancel => index,
    };
    order[index]
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn action(text: &str) -> Option<KeyAction> {
        KeyAction::from_key(&KeyWithModifier::from_str(text).unwrap())
    }

    #[test]
    fn keys() {
        assert_eq!(action("Left"), Some(KeyAction::Prev));
        assert_eq!(action("h"), Some(KeyAction::Prev));
        assert_eq!(action("Right"), Some(KeyAction::Next));
        assert_eq!(action("l"), Some(KeyAction::Next));
        assert_eq!(action("Enter"), Some(KeyAction::Confirm));
        assert_eq!(action("Esc"), Some(KeyAction::Cancel));
        assert_eq!(action("Ctrl c"), Some(KeyAction::Cancel));
        // それ以外は無視する
        for text in ["c", "H", "Ctrl h", "Alt Left", "Up", "j", "a", "Space"] {
            assert_eq!(action(text), None, "{text}");
        }
    }

    #[test]
    fn initial_prefers_attention_then_focus_then_left() {
        let order = [3, 1, 4, 2];
        let none = |_| false;
        // 注意が必要なエージェント（複数あれば左端）
        assert_eq!(initial(&order, |id| id == 4 || id == 2, Some(1)), Some(4));
        // フォーカス中のペイン
        assert_eq!(initial(&order, none, Some(1)), Some(1));
        // フォーカス中のペインが表示されていなければ、左端
        assert_eq!(initial(&order, none, Some(9)), Some(3));
        assert_eq!(initial(&order, none, None), Some(3));
        // 表示中のエージェントがなければ入らない
        assert_eq!(initial(&[], |_| true, Some(1)), None);
    }

    #[test]
    fn step_wraps_around() {
        let order = [3, 1, 4];
        assert_eq!(step(&order, 3, KeyAction::Next), 1);
        assert_eq!(step(&order, 4, KeyAction::Next), 3);
        assert_eq!(step(&order, 3, KeyAction::Prev), 4);
        assert_eq!(step(&order, 1, KeyAction::Prev), 3);
        assert_eq!(step(&[5], 5, KeyAction::Next), 5);
        assert_eq!(step(&[5], 5, KeyAction::Prev), 5);
        // 表示されていないエージェントはそのまま
        assert_eq!(step(&order, 9, KeyAction::Next), 9);
    }
}
