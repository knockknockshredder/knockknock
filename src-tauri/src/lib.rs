// src-tauri/src/lib.rs

mod browser;
mod commands;
mod pin;
mod shredder;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            tray::setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::get_algorithms,
            commands::shred::validate_paths,
            commands::browser::detect_browsers,
            commands::browser::shred_browser_data,
            commands::tray::quick_shred_from_clipboard,
            commands::tray::minimize_to_tray,
            commands::pin::set_pin,
            commands::pin::verify_pin,
            commands::pin::is_pin_enabled,
            commands::pin::disable_pin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
