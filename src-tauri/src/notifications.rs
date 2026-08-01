// src-tauri/src/notifications.rs

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Send a desktop notification via the Tauri notification plugin.
///
/// Failures are logged but not surfaced — notifications are fire-and-forget
/// from the tray's perspective.
pub fn send_notification(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("[KnockKnock] Failed to send notification: {e}");
    }
}
