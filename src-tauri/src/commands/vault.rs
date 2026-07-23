// src-tauri/src/commands/vault.rs
//
// Tauri commands that expose the vault over IPC. All commands operate on
// the user's pending shred list (Vec<String> of paths). The PIN is passed
// per-call and never persisted by the frontend.

use crate::{pin, vault};

#[tauri::command]
pub async fn save_vault(paths: Vec<String>, pin: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || vault::storage::save(&paths, &pin))
        .await
        .map_err(|e| format!("Task panicked: {:?}", e))?
}

#[tauri::command]
pub async fn load_vault(pin: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || vault::storage::load(&pin))
        .await
        .map_err(|e| format!("Task panicked: {:?}", e))?
}

#[tauri::command]
pub async fn clear_vault(current_pin: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if pin::has_pin() && !pin::verify_pin(&current_pin)? {
            return Err("PIN is incorrect".to_string());
        }
        vault::storage::clear()
    })
    .await
    .map_err(|e| format!("Task panicked: {:?}", e))?
}

#[tauri::command]
pub fn vault_exists() -> bool {
    vault::storage::exists()
}
