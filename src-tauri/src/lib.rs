// src-tauri/src/lib.rs

mod browser;
mod commands;
mod shredder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::get_algorithms,
            commands::browser::detect_browsers,
            commands::browser::shred_browser_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
