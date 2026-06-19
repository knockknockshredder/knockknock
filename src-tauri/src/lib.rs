// src-tauri/src/lib.rs

mod commands;
mod shredder;
mod updater;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::get_algorithms,
            commands::updater::check_update,
            commands::updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
