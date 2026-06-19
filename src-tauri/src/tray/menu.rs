// src-tauri/src/tray/menu.rs

use tauri::{
    menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem},
    AppHandle, Emitter, Manager, Wry,
};

/// Build the tray context menu with all items.
///
/// Items:
/// - Quick Shred: triggered from frontend (file picker flow)
/// - Shred Clipboard: triggered from frontend
/// - Open/Hide Window: toggled directly here
/// - Settings: triggered from frontend
/// - Quit: handled directly here
pub fn create_tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let quick_shred = MenuItemBuilder::with_id("quick_shred", "Quick Shred")
        .enabled(true)
        .build(app)?;
    let shred_clipboard = MenuItemBuilder::with_id("shred_clipboard", "Shred Clipboard")
        .enabled(true)
        .build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let toggle_window = MenuItemBuilder::with_id("toggle_window", "Open/Hide Window")
        .enabled(true)
        .build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Settings")
        .enabled(true)
        .build(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .enabled(true)
        .build(app)?;

    Menu::with_items(
        app,
        &[
            &quick_shred,
            &shred_clipboard,
            &sep1,
            &toggle_window,
            &settings,
            &sep2,
            &quit,
        ],
    )
}

/// Refresh / re-show the tray context menu.
///
/// The menu is attached to the tray icon at setup time, so the OS
/// handles right-click display automatically. This function is reserved
/// for future scenarios that require an explicit menu refresh or popup.
pub fn show_context_menu(_app: &AppHandle) {
    // Intentionally empty: see doc comment above.
}

/// Handle a tray menu item click.
///
/// Items that affect window state or app lifecycle are handled here.
/// Items needing user interaction (Quick Shred, Shred Clipboard,
/// Settings) are forwarded to the frontend via the
/// `tray-menu-action` event.
pub fn handle_event(app: &AppHandle, event: &MenuEvent) {
    match event.id.as_ref() {
        "toggle_window" => {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ => {
            // Forward other actions to the frontend for UI handling.
            let _ = app.emit_to("main", "tray-menu-action", event.id.as_ref());
        }
    }
}
