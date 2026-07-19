// src-tauri/src/pin/config.rs

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Atomic file write: write to `<path>.tmp`, then rename.
/// Prevents partial writes from corrupting sensitive config.
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(|e| format!("Failed to write {}: {}", tmp.display(), e))?;
    fs::rename(&tmp, path).map_err(|e| format!("Failed to rename {}: {}", path.display(), e))?;
    set_owner_only(path)?;
    Ok(())
}

// --- Permission helpers ---
//
// All sensitive files live under <config_dir>/KnockKnock/. On Unix we tighten
// the directory to 0o700 (owner-only traversable) and every file to 0o600
// (owner-only readable/writable). On Windows the default ACL for files
// inside a user profile directory already restricts access to the owning
// user, so these are no-ops there.

#[cfg(unix)]
pub(crate) fn set_owner_only(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let meta =
        std::fs::metadata(path).map_err(|e| format!("Failed to stat {}: {}", path.display(), e))?;
    let mut perms = meta.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed to chmod 0o600 {}: {}", path.display(), e))
}

#[cfg(not(unix))]
pub(crate) fn set_owner_only(_path: &Path) -> Result<(), String> {
    // Windows: default ACL is owner-only for files in user profile dirs.
    // Future: explicit ACL hardening if multi-user scenarios arise.
    Ok(())
}

#[cfg(unix)]
pub(crate) fn set_owner_only_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let meta =
        std::fs::metadata(path).map_err(|e| format!("Failed to stat {}: {}", path.display(), e))?;
    let mut perms = meta.permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed to chmod 0o700 {}: {}", path.display(), e))
}

#[cfg(not(unix))]
pub(crate) fn set_owner_only_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn set_owner_only_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        set_owner_only_dir(parent)?;
    }
    Ok(())
}

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
    set_owner_only_parent(&path)?;
    let config = serde_json::json!({ "pin_hash": hash });
    atomic_write(&path, &config.to_string())?;
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
    set_owner_only_parent(&path)?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize lockout state: {}", e))?;
    atomic_write(&path, &json)?;
    Ok(())
}

pub fn load_lockout_state() -> Result<LockoutState, String> {
    let path = get_lockout_path();
    if !path.exists() {
        return Ok(LockoutState::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read lockout state: {}", e))?;

    let state: LockoutState = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse lockout state: {}", e))?;

    Ok(state)
}

/// `true` only if the lockout file exists and is parseable but the file's
/// content is empty / corrupt / not a valid `LockoutState`. Used by the
/// startup path to decide whether to refuse to start (tampering signal).
pub fn lockout_file_is_corrupt() -> bool {
    let path = get_lockout_path();
    if !path.exists() {
        return false;
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return true;
    };
    serde_json::from_str::<LockoutState>(&content).is_err()
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
    set_owner_only_parent(&path)?;
    let value = if enabled { "1" } else { "0" };
    atomic_write(&path, value)?;
    Ok(())
}

pub fn load_pin_enabled() -> Result<bool, String> {
    let path = get_enabled_path();
    if !path.exists() {
        return Ok(false); // never been set → not enabled
    }
    fs::read_to_string(&path)
        .map(|s| s.trim() == "1")
        .map_err(|e| format!("Failed to read PIN enabled state: {}", e))
}
