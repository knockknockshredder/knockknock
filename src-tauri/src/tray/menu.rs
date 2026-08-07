// src-tauri/src/tray/menu.rs

use tauri::{
    menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem},
    AppHandle, Emitter, Manager, Wry,
};

use crate::tray::actions;

/// Build the tray context menu with all items.
///
/// Items:
/// - Delete Selected Targets: handled directly via tray action (file picker + shred)
/// - Clear Clipboard: handled directly via tray action (clipboard clear)
/// - Show/Hide Window: toggled directly here
/// - Settings: triggered from frontend
/// - Quit: handled directly here
///
/// Menu item handles for Delete Selected Targets and Clear Clipboard are
/// stored in `TrayState` so their enabled state can be toggled during
/// shredding.
pub fn create_tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let quick_shred =
        MenuItemBuilder::with_id("quick_shred", crate::native_ui_copy::TRAY_DELETE_SELECTED)
            .enabled(true)
            .build(app)?;
    let shred_clipboard = MenuItemBuilder::with_id(
        "shred_clipboard",
        crate::native_ui_copy::TRAY_CLEAR_CLIPBOARD,
    )
    .enabled(true)
    .build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let toggle_window =
        MenuItemBuilder::with_id("toggle_window", crate::native_ui_copy::TRAY_TOGGLE_WINDOW)
            .enabled(true)
            .build(app)?;
    let settings = MenuItemBuilder::with_id("settings", crate::native_ui_copy::TRAY_SETTINGS)
        .enabled(true)
        .build(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", crate::native_ui_copy::TRAY_QUIT)
        .enabled(true)
        .build(app)?;

    // Store menu item handles in TrayState for dynamic enable/disable.
    let state = app.state::<crate::tray::actions::TrayState>();
    *state
        .quick_shred_item
        .lock()
        .expect("Failed to lock TrayState") = Some(quick_shred.clone());
    *state
        .shred_clipboard_item
        .lock()
        .expect("Failed to lock TrayState") = Some(shred_clipboard.clone());

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
/// - Delete Selected Targets and Clear Clipboard are handled directly in
///   Rust via tray actions (file shredding / clipboard clear).
/// - Toggle Window and Quit affect window state / app lifecycle here.
/// - Settings shows the window and emits `open-settings` for the
///   frontend to navigate to the Settings section.
pub fn handle_event(app: &AppHandle, event: &MenuEvent) {
    match event.id.as_ref() {
        "quick_shred" => {
            actions::quick_shred(app);
        }
        "shred_clipboard" => {
            actions::shred_clipboard(app);
        }
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
        "settings" => {
            actions::open_settings(app);
        }
        _ => {
            // Forward unknown items to the frontend.
            let _ = app.emit_to("main", "tray-menu-action", event.id.as_ref());
        }
    }
}
