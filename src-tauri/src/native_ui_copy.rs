// src-tauri/src/native_ui_copy.rs
//
// Centralized copy for native UI surfaces that cannot use the React
// frontend: tray UI, native tray-generated notifications, fatal
// startup UI before the WebView, and package/build metadata.

pub const TRAY_TOOLTIP: &str = "KnockKnock — Local Data Deletion";

pub const TRAY_DELETE_SELECTED: &str = "Delete Selected Targets";
pub const TRAY_CLEAR_CLIPBOARD: &str = "Clear Clipboard";
pub const TRAY_TOGGLE_WINDOW: &str = "Show/Hide Window";
pub const TRAY_SETTINGS: &str = "Settings";
pub const TRAY_QUIT: &str = "Quit";

pub const NOTIFICATION_OPERATION_IN_PROGRESS_TITLE: &str = "Operation in progress";
pub const NOTIFICATION_OPERATION_IN_PROGRESS_BODY: &str =
    "A deletion operation is already in progress.";

pub const NOTIFICATION_CLIPBOARD_CLEARED_TITLE: &str = "Clipboard cleared";
pub const NOTIFICATION_CLIPBOARD_CLEARED_BODY: &str = "Current clipboard contents were cleared.";

pub const NOTIFICATION_CLIPBOARD_ERROR_TITLE: &str = "Clipboard error";
pub const NOTIFICATION_CLIPBOARD_ERROR_BODY: &str = "Could not access the system clipboard.";

pub const STARTUP_ERROR_TITLE: &str = "KnockKnock — Startup Error";
pub const STARTUP_WRITABLE_LOCATION_GUIDANCE: &str =
    "Move KnockKnock to a writable folder such as Desktop, Documents, or ~/Applications, then start it again.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_copy_matches_expected_values() {
        assert_eq!(TRAY_TOOLTIP, "KnockKnock — Local Data Deletion");
        assert_eq!(TRAY_DELETE_SELECTED, "Delete Selected Targets");
        assert_eq!(TRAY_CLEAR_CLIPBOARD, "Clear Clipboard");
        assert_eq!(TRAY_TOGGLE_WINDOW, "Show/Hide Window");
        assert_eq!(TRAY_SETTINGS, "Settings");
        assert_eq!(TRAY_QUIT, "Quit");
    }

    #[test]
    fn notification_copy_matches_expected_values() {
        assert_eq!(
            NOTIFICATION_OPERATION_IN_PROGRESS_TITLE,
            "Operation in progress"
        );
        assert_eq!(
            NOTIFICATION_OPERATION_IN_PROGRESS_BODY,
            "A deletion operation is already in progress."
        );
        assert_eq!(NOTIFICATION_CLIPBOARD_CLEARED_TITLE, "Clipboard cleared");
        assert_eq!(
            NOTIFICATION_CLIPBOARD_CLEARED_BODY,
            "Current clipboard contents were cleared."
        );
        assert_eq!(NOTIFICATION_CLIPBOARD_ERROR_TITLE, "Clipboard error");
        assert_eq!(
            NOTIFICATION_CLIPBOARD_ERROR_BODY,
            "Could not access the system clipboard."
        );
    }

    #[test]
    fn startup_copy_matches_expected_values() {
        assert_eq!(STARTUP_ERROR_TITLE, "KnockKnock — Startup Error");
        assert_eq!(
            STARTUP_WRITABLE_LOCATION_GUIDANCE,
            "Move KnockKnock to a writable folder such as Desktop, Documents, or ~/Applications, then start it again."
        );
    }
}
