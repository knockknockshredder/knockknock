// src-tauri/src/pin/config.rs

use std::fs;
use std::path::PathBuf;

fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("KnockKnock");
    path.push("pin.json");
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
