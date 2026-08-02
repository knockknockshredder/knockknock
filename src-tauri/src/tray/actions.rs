// src-tauri/src/tray/actions.rs

use std::sync::Mutex;

use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};

/// Tauri-managed state for tray actions.
///
/// Tracks whether a shred operation is in progress and holds references to
/// menu items so their enabled state can be updated dynamically.
pub struct TrayState {
    pub is_shredding: Mutex<bool>,
    pub quick_shred_item: Mutex<Option<MenuItem<Wry>>>,
    pub shred_clipboard_item: Mutex<Option<MenuItem<Wry>>>,
}

impl TrayState {
    pub fn new() -> Self {
        Self {
            is_shredding: Mutex::new(false),
            quick_shred_item: Mutex::new(None),
            shred_clipboard_item: Mutex::new(None),
        }
    }
}

/// Handle "Quick Shred" tray action.
///
/// Emits a `quick-shred-request` event to the frontend, which owns the
/// file list, PIN verification, and confirmation dialog. The frontend
/// then runs the same `executeShred` flow (vault flush, typed
/// `execute_roots` command) as the main Shred button — there is no second
/// destructive path. This design keeps file paths behind the PIN gate —
/// the Rust backend never sees them without frontend-mediated verification.
pub fn quick_shred(app: &AppHandle) {
    use tauri::Emitter;

    let state = app.state::<TrayState>();
    let is_shredding = state.is_shredding.lock().expect("TrayState mutex poisoned");

    if *is_shredding {
        crate::notifications::send_notification(app, "KnockKnock", "Shred already in progress");
        return;
    }

    // Show and focus the window so the user sees the PIN/confirmation dialogs
    show_window(app);

    // Emit event — frontend listens and handles PIN + confirmation + shred
    if let Err(e) = app.emit_to("main", "quick-shred-request", ()) {
        eprintln!("[tray] failed to emit quick-shred-request: {}", e);
    }
}

/// Handle "Settings" tray action.
///
/// Shows and focuses the main window, then emits an event the frontend
/// uses to navigate to the Settings section.
pub fn open_settings(app: &AppHandle) {
    use tauri::Emitter;

    show_window(app);

    if let Err(e) = app.emit_to("main", "open-settings", ()) {
        eprintln!("[tray] failed to emit open-settings: {}", e);
    }
}

/// Clear the system clipboard and send a notification.
///
/// Clipboard wipe is a simple `clear()` call, not a multi-pass overwrite —
/// the clipboard contents live in system-managed memory where secure deletion
/// semantics do not apply.
pub fn shred_clipboard(app: &AppHandle) {
    let app_handle = app.clone();

    std::thread::spawn(move || match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.clear() {
                eprintln!("[KnockKnock] Failed to clear clipboard: {e}");
            }
            crate::notifications::send_notification(
                &app_handle,
                "Clipboard Cleared",
                "Clipboard contents have been wiped.",
            );
        }
        Err(e) => {
            eprintln!("[KnockKnock] Failed to access clipboard: {e}");
            crate::notifications::send_notification(
                &app_handle,
                "Clipboard Error",
                "Could not access the system clipboard.",
            );
        }
    });
}

/// Enable or disable the Quick Shred and Shred Clipboard menu items.
pub fn update_menu_state(app: &AppHandle, enabled: bool) {
    let state = app.state::<TrayState>();

    {
        let guard = state
            .quick_shred_item
            .lock()
            .expect("Failed to lock TrayState");
        if let Some(item) = guard.as_ref() {
            if let Err(e) = item.set_enabled(enabled) {
                eprintln!("[KnockKnock] Failed to set Quick Shred enabled state: {e}");
            }
        }
    }

    {
        let guard = state
            .shred_clipboard_item
            .lock()
            .expect("Failed to lock TrayState");
        if let Some(item) = guard.as_ref() {
            if let Err(e) = item.set_enabled(enabled) {
                eprintln!("[KnockKnock] Failed to set Shred Clipboard enabled state: {e}");
            }
        }
    }
}

/// Show, unminimize, and focus the main window.
fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.unminimize() {
            eprintln!("[tray] failed to unminimize window: {}", e);
        }
        if let Err(e) = window.show() {
            eprintln!("[tray] failed to show window: {}", e);
        }
        if let Err(e) = window.set_focus() {
            eprintln!("[tray] failed to focus window: {}", e);
        }
    }
}
