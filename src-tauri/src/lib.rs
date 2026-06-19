// src-tauri/src/lib.rs

mod commands;
mod shredder;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::get_algorithms,
            commands::tray::quick_shred_from_clipboard,
            commands::tray::minimize_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
