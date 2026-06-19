// src-tauri/src/commands/pin.rs

use crate::pin;

#[tauri::command]
pub fn set_pin(pin_value: String) -> Result<(), String> {
    pin::set_pin(&pin_value)
}

#[tauri::command]
pub fn verify_pin(pin_value: String) -> Result<bool, String> {
    pin::verify_pin(&pin_value)
}

#[tauri::command]
pub fn is_pin_enabled() -> bool {
    pin::is_pin_enabled()
}

#[tauri::command]
pub fn disable_pin() -> Result<(), String> {
    pin::disable_pin()
}
