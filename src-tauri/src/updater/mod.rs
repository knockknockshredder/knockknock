// src-tauri/src/updater/mod.rs

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub async fn check_for_updates(app: AppHandle) -> Result<bool, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    match updater.check().await {
        Ok(update) => Ok(update.is_some()),
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    if let Ok(update) = updater.check().await {
        if let Some(update) = update {
            update
                .download_and_install(|_chunk, _total| {}, || {})
                .await
                .map_err(|e| format!("Install failed: {}", e))?;
        }
    }

    Ok(())
}
