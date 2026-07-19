// src-tauri/src/lib.rs

mod browser;
mod commands;
mod drive;
mod paths;
mod pin;
mod shredder;
mod tray;
mod vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Restore persisted PIN lockout state before any Tauri commands can run,
    // so a previously locked-out user cannot bypass the lockout by relaunching
    // the app. Surface failures loudly — silent failure would defeat the point.
    if let Err(e) = pin::init_lockout_state() {
        eprintln!("[knockknock] failed to load PIN lockout state: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            tray::setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::cancel_shred,
            commands::shred::cleanup_orphans,
            commands::shred::get_algorithms,
            commands::shred::validate_paths,
            commands::shred::get_drive_info,
            commands::shred::get_all_drive_info,
            commands::shred::request_elevation,
            commands::browser::detect_browsers,
            commands::browser::shred_browser_data,
            commands::tray::quick_shred_from_clipboard,
            commands::tray::minimize_to_tray,
            commands::pin::setup_pin,
            commands::pin::verify_pin,
            commands::pin::is_pin_enabled,
            commands::pin::set_pin_enabled,
            commands::pin::has_pin,
            commands::pin::is_pin_locked,
            commands::pin::get_lockout_remaining,
            commands::pin::change_pin,
            commands::pin::reset_app,
            commands::pin::disable_pin,
            commands::vault::save_vault,
            commands::vault::load_vault,
            commands::vault::clear_vault,
            commands::vault::vault_exists,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}