// src-tauri/src/commands/pin.rs

use crate::pin;

#[tauri::command]
pub fn setup_pin(old_pin: Option<String>, new_pin: String) -> Result<(), String> {
    pin::setup_pin(old_pin.as_deref(), &new_pin)
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
pub fn set_pin_enabled(current_pin: String, enabled: bool) -> Result<(), String> {
    pin::set_pin_enabled(&current_pin, enabled)
}

#[tauri::command]
pub fn has_pin() -> bool {
    pin::has_pin()
}

#[tauri::command]
pub fn is_pin_locked() -> Result<bool, String> {
    pin::is_pin_locked()
}

/// Seconds remaining on the current lockout window, or 0 when not locked.
/// Returns a flat `u64` for easy consumption from the frontend.
#[tauri::command]
pub fn get_lockout_remaining() -> Result<u64, String> {
    pin::lockout_remaining().map(|o| o.unwrap_or(0))
}

#[tauri::command]
pub fn change_pin(old_pin: String, new_pin: String) -> Result<(), String> {
    pin::change_pin(old_pin, new_pin)
}

/// Wipe the entire app state (PIN + lockout + vault callers). Requires
/// the current PIN to be valid as a safety check.
#[tauri::command]
pub fn reset_app(current_pin: String) -> Result<(), String> {
    pin::reset_app(&current_pin)
}

#[tauri::command]
pub fn disable_pin(current_pin: String) -> Result<(), String> {
    pin::disable_pin(&current_pin)
}
