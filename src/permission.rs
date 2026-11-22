//! permission の要求と、プラグインのペインの操作。
//!
//! データの収集（Collector）とは分けて持つ。Collector は許可の後に届くイベントを変換するだけにする。
//!
//! permission ダイアログはプラグインのペインに出るため、許可が得られるまでは
//! selectable のままにしておき、許可を受け取ったときに unselectable にする。
//!
//! 要求する permission は次の 3 つ。1 回の要求でまとめて求める。
//! Zellij は 1 回の要求ごとに、許可・拒否を 1 つだけ返す（どの permission の結果かは分からない）。
//! また、許可を保存するときはプラグインごとに一覧を丸ごと置き換える（`PermissionCache::cache`）ので、
//! 分けて要求すると後の結果で前の許可が消えるおそれがある。
//!
//! - `ReadApplicationState`：エージェントの状態の収集（[`crate::collect`]）
//! - `ChangeApplicationState`：選んだエージェントのペインへのフォーカスの移動（`focus_terminal_pane`）
//! - `InterceptInput`：選択モード中のキーの横取り（`intercept_key_presses`。[`crate::select`]）
use zellij_tile::prelude::*;

/// 要求した permission の許可状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Permission {
    /// 結果がまだ届いていない
    #[default]
    Pending,
    Granted,
    Denied,
}

impl Permission {
    /// `ZellijPlugin::load` から呼ぶ。permission を要求し、結果のイベントを購読する。
    pub fn request() {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::InterceptInput,
        ]);
        subscribe(&[EventType::PermissionRequestResult]);
    }

    /// `PermissionRequestResult` を受け取ったときに呼ぶ。再描画が必要なときに true を返す。
    /// 許可に変わったときに、プラグインのペインを unselectable にする。
    pub fn update(&mut self, status: PermissionStatus) -> bool {
        let changed = self.apply(status);
        if changed && *self == Permission::Granted {
            set_selectable(false);
        }
        changed
    }

    /// `update` のうち、Zellij の API を呼ばない部分（テスト用に分けている）。
    fn apply(&mut self, status: PermissionStatus) -> bool {
        let permission = match status {
            PermissionStatus::Granted => Permission::Granted,
            PermissionStatus::Denied => Permission::Denied,
        };
        let changed = *self != permission;
        *self = permission;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_is_recorded() {
        let mut permission = Permission::default();
        assert_eq!(permission, Permission::Pending);

        assert!(permission.apply(PermissionStatus::Granted));
        assert_eq!(permission, Permission::Granted);

        assert!(!permission.apply(PermissionStatus::Granted));

        assert!(permission.apply(PermissionStatus::Denied));
        assert_eq!(permission, Permission::Denied);
    }
}
