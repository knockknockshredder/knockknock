// src-tauri/src/tray/mod.rs

pub mod menu;

use tauri::{
    AppHandle, Manager,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Initialize the system tray icon with context menu.
///
/// The menu is attached to the tray icon so the OS auto-displays it on
/// right-click. Left-click toggles the main window visibility.
/// `show_menu_on_left_click(false)` keeps left-click available for the
/// custom toggle handler instead of showing the menu.
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = menu::create_tray_menu(app)?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("KnockKnock - Emergency File Shredder")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Show/hide main window
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Context menu is auto-displayed by the OS when the
                    // tray icon is right-clicked. This call is kept for
                    // future dynamic menu refresh scenarios.
                    let app = tray.app_handle();
                    menu::show_context_menu(app);
                }
                _ => {}
            }
        })
        .on_menu_event(|app, event| {
            menu::handle_event(app, &event);
        })
        .build(app)?;

    Ok(())
}