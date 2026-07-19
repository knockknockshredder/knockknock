// src-tauri/src/commands/vault.rs
//
// Tauri commands that expose the vault over IPC. All commands operate on
// the user's pending shred list (Vec<String> of paths). The PIN is passed
// per-call and never persisted by the frontend.

use crate::{pin, vault};

#[tauri::command]
pub fn save_vault(paths: Vec<String>, pin: String) -> Result<(), String> {
    vault::storage::save(&paths, &pin)
}

#[tauri::command]
pub fn load_vault(pin: String) -> Result<Vec<String>, String> {
    vault::storage::load(&pin)
}

#[tauri::command]
pub fn clear_vault(current_pin: String) -> Result<(), String> {
    if pin::has_pin() && !pin::verify_pin(&current_pin)? {
        return Err("PIN is incorrect".to_string());
    }
    vault::storage::clear()
}

#[tauri::command]
pub fn vault_exists() -> bool {
    vault::storage::exists()
}
