// src-tauri/src/commands/browser.rs

use crate::browser;
use crate::browser::types::*;
use crate::shredder::types::ShredReport;

#[tauri::command]
pub fn detect_browsers() -> Vec<DetectedBrowser> {
    browser::detection::detect_browsers()
}

#[tauri::command]
pub fn shred_browser_data(_request: BrowserShredRequest) -> Result<ShredReport, String> {
    // TODO: Implement browser data shredding
    // 1. Check if browser is running
    // 2. Warn if running
    // 3. Shred selected data types
    Err("Not implemented yet".to_string())
}
