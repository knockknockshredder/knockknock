// src-tauri/src/pin/mod.rs

pub mod config;

use bcrypt::{hash, verify, DEFAULT_COST};

#[derive(Debug)]
#[allow(dead_code)]
pub struct PinConfig {
    pub enabled: bool,
    pub hash: Option<String>,
}

pub fn set_pin(pin: &str) -> Result<(), String> {
    let hashed = hash(pin, DEFAULT_COST).map_err(|e| format!("Failed to hash PIN: {}", e))?;

    config::save_pin_hash(&hashed)?;
    Ok(())
}

pub fn verify_pin(pin: &str) -> Result<bool, String> {
    let stored_hash = config::load_pin_hash()?;

    match stored_hash {
        Some(hash) => verify(pin, &hash).map_err(|e| format!("PIN verification failed: {}", e)),
        None => Ok(true), // No PIN set = always valid
    }
}

pub fn is_pin_enabled() -> bool {
    config::load_pin_hash().ok().flatten().is_some()
}

pub fn disable_pin() -> Result<(), String> {
    config::remove_pin_hash()
}
