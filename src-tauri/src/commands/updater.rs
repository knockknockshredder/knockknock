// src-tauri/src/commands/updater.rs

use crate::updater;
use tauri::command;

#[command]
pub async fn check_update(app: tauri::AppHandle) -> Result<bool, String> {
    updater::check_for_updates(app).await
}

#[command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    updater::install_update(app).await
}
