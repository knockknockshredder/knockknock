// src-tauri/src/pin/config.rs

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("KnockKnock");
    path.push("pin.json");
    path
}

fn get_lockout_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("KnockKnock");
    path.push("lockout.json");
    path
}

pub fn save_pin_hash(hash: &str) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }

    let config = serde_json::json!({ "pin_hash": hash });
    fs::write(&path, config.to_string())
        .map_err(|e| format!("Failed to write PIN config: {}", e))?;

    Ok(())
}

pub fn load_pin_hash() -> Result<Option<String>, String> {
    let path = get_config_path();
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read PIN config: {}", e))?;

    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse PIN config: {}", e))?;

    Ok(config["pin_hash"].as_str().map(|s| s.to_string()))
}

pub fn remove_pin_hash() -> Result<(), String> {
    let path = get_config_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to remove PIN config: {}", e))?;
    }
    Ok(())
}

/// Persisted lockout state. Survives app restarts so attackers cannot simply
/// relaunch the app to reset failed-attempt counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockoutState {
    pub failed_attempts: u32,
    pub lockout_until_unix: Option<u64>,
}

impl Default for LockoutState {
    fn default() -> Self {
        Self {
            failed_attempts: 0,
            lockout_until_unix: None,
        }
    }
}

pub fn save_lockout_state(state: &LockoutState) -> Result<(), String> {
    let path = get_lockout_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize lockout state: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write lockout state: {}", e))?;

    Ok(())
}

pub fn load_lockout_state() -> Result<LockoutState, String> {
    let path = get_lockout_path();
    if !path.exists() {
        return Ok(LockoutState::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read lockout state: {}", e))?;

    // If the file is corrupt, fail safe to defaults — better to lose the counter
    // than to refuse to start the app.
    let state: LockoutState = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse lockout state: {}", e))?;

    Ok(state)
}

pub fn clear_lockout_state() -> Result<(), String> {
    let path = get_lockout_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to remove lockout state: {}", e))?;
    }
    Ok(())
}

// --- PIN enabled flag (separate from PIN hash existence) ---

fn get_enabled_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("KnockKnock");
    path.push("pin_enabled");
    path
}

pub fn save_pin_enabled(enabled: bool) -> Result<(), String> {
    let path = get_enabled_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    fs::write(&path, if enabled { "1" } else { "0" })
        .map_err(|e| format!("Failed to write PIN enabled state: {}", e))
}

pub fn load_pin_enabled() -> bool {
    let path = get_enabled_path();
    if !path.exists() {
        return false; // never been set → not enabled
    }
    fs::read_to_string(&path)
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}
