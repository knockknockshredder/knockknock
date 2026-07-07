// src-tauri/src/commands/browser.rs

use crate::browser;
use crate::browser::types::*;
use crate::shredder::types::ShredReport;
use std::path::Path;

#[tauri::command]
pub async fn detect_browsers() -> Vec<DetectedBrowser> {
    tokio::task::spawn_blocking(browser::detection::detect_browsers)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn get_profile_size(path: String) -> u64 {
    tokio::task::spawn_blocking(move || {
        browser::detection::estimate_directory_size(Path::new(&path))
    })
    .await
    .unwrap_or(0)
}

#[tauri::command]
pub fn shred_browser_data(_request: BrowserShredRequest) -> Result<ShredReport, String> {
    // TODO: Implement browser data shredding
    // 1. Check if browser is running
    // 2. Warn if running
    // 3. Shred selected data types
    Err("Not implemented yet".to_string())
}
