// src-tauri/src/commands/tray.rs

use tauri::{AppHandle, Manager};

/// Read file paths from the clipboard and initiate shred.
///
/// Returns the number of paths found. Frontend listens on
/// `tray-menu-action` for the user-facing confirmation flow.
#[tauri::command]
pub fn quick_shred_from_clipboard() -> Result<String, String> {
    Err("Not implemented".to_string())
}

/// Hide the main window; the app continues to run in the system tray.
#[tauri::command]
pub fn minimize_to_tray(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}
