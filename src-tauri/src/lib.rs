// src-tauri/src/lib.rs

mod commands;
mod pin;
mod shredder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::get_algorithms,
            commands::pin::set_pin,
            commands::pin::verify_pin,
            commands::pin::is_pin_enabled,
            commands::pin::disable_pin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
