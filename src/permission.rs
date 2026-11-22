//! permission の要求と、プラグインのペインの操作。
//!
//! データの収集（Collector）とは分けて持つ。Collector は許可の後に届くイベントを変換するだけにする。
//!
//! permission ダイアログはプラグインのペインに出るため、許可が得られるまでは
//! selectable のままにしておき、許可を受け取ったときに unselectable にする。

use zellij_tile::prelude::*;

/// `ReadApplicationState` の許可状態。
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
        request_permission(&[PermissionType::ReadApplicationState]);
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
