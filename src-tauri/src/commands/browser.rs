// src-tauri/src/commands/browser.rs

use crate::browser;
use crate::browser::types::*;
use crate::shredder::types::ShredReport;

#[tauri::command]
pub async fn detect_browsers() -> Result<Vec<DetectedBrowser>, String> {
    eprintln!("[detect_browsers command] called");
    let result = tokio::task::spawn_blocking(|| browser::detection::detect_browsers())
        .await
        .map_err(|e| format!("Detection panicked: {:?}", e))?;
    eprintln!("[detect_browsers command] returning {} browsers", result.len());
    Ok(result)
}

#[tauri::command]
pub fn shred_browser_data(_request: BrowserShredRequest) -> Result<ShredReport, String> {
    // TODO: Implement browser data shredding
    // 1. Check if browser is running
    // 2. Warn if running
    // 3. Shred selected data types
    Err("Not implemented yet".to_string())
}
