// src-tauri/src/commands/tray.rs

use tauri::{AppHandle, Manager};

use crate::tray::actions::TrayState;

/// Hide the main window; the app continues to run in the system tray.
#[tauri::command]
pub fn minimize_to_tray(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Update the tray state mirror from the frontend.
///
/// Called by frontend whenever file list or shred state changes.
/// Updates the menu item enabled states accordingly.
#[tauri::command]
pub fn sync_tray_state(app: AppHandle, _has_files: bool, is_shredding: bool) -> Result<(), String> {
    let state = app.state::<TrayState>();
    {
        let mut shredding = state.is_shredding.lock().map_err(|e| e.to_string())?;
        *shredding = is_shredding;
    }

    // Update menu items: disable during shred, enable otherwise
    crate::tray::actions::update_menu_state(&app, !is_shredding);

    Ok(())
}

/// Send a desktop notification from the frontend.
///
/// Delegates to the `tauri-plugin-notification` Rust API. The notification
/// title and body are provided by the caller; failures are logged silently.
#[tauri::command]
pub fn send_notification(app: AppHandle, title: String, body: String) -> Result<(), String> {
    crate::notifications::send_notification(&app, &title, &body);
    Ok(())
}
