// src-tauri/src/commands/settings.rs
//
// Portable settings persistence replacing browser localStorage.
// Reads/writes settings.json in the portable data directory.
// Uses atomic write (temp file → rename) to prevent corruption on crash.
// Saves are serialized via Mutex to prevent concurrent rename races.
// Falls back to defaults if file doesn't exist or is corrupted.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes concurrent `save_settings` calls so the .tmp rename
/// sequence is atomic across calls. Without this, two concurrent
/// saves can race on the shared `.tmp` path and corrupt the file.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub auto_clear_log: bool,
    pub default_algorithm_index: usize,
    pub log_obfuscation: String, // "none" | "numbered" | "partial_mask"
    pub left_sidebar_width: u32,
    pub right_sidebar_width: u32,
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::paths::portable_data_dir()?.join("settings.json"))
}

#[tauri::command]
pub fn get_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let data =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {e}"))?;

    match serde_json::from_str(&data) {
        Ok(s) => Ok(s),
        Err(e) => {
            // Corrupted file — return defaults so the app always starts.
            // The corrupted file will be overwritten on next save.
            eprintln!("[KnockKnock] Corrupted settings.json, using defaults: {e}");
            Ok(AppSettings::default())
        }
    }
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    let _guard = SAVE_LOCK
        .lock()
        .map_err(|e| format!("Settings save lock poisoned: {e}"))?;

    let path = settings_path()?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);

    let mut file =
        std::fs::File::create(&tmp).map_err(|e| format!("Failed to create settings tmp: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write settings: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Failed to fsync settings: {e}"))?;
    drop(file);

    // On Windows pre-1.86, std::fs::rename may not be atomic. If target
    // exists, remove it first (data already in tmp file via fsync).
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(&path);
    }

    std::fs::rename(&tmp, &path).map_err(|e| format!("Failed to save settings: {e}"))
}
