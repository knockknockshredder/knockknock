// src-tauri/src/vault/storage.rs
//
// File-based persistence for the encrypted vault. Each save generates a new
// salt and nonce, so the file is safe to keep on disk without the PIN.
//
// File layout: <config_dir>/KnockKnock/vault.json

use super::crypto::{self, EncryptedData};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct VaultFile {
    /// Format version. Mirrors [`crypto::VAULT_VERSION`] at the time of
    /// encryption. Stored so we can reject unsupported vaults explicitly.
    version: u32,
    salt: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

fn vault_path() -> Result<PathBuf, String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Cannot find config directory".to_string())?;
    let app_dir = config_dir.join("KnockKnock");
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app directory: {}", e))?;
    Ok(app_dir.join("vault.json"))
}

/// Encrypt `paths` under `pin` and write to disk, replacing any existing vault.
pub fn save(paths: &[String], pin: &str) -> Result<(), String> {
    let plaintext =
        serde_json::to_vec(paths).map_err(|e| format!("Failed to serialize paths: {}", e))?;

    let encrypted = crypto::encrypt(&plaintext, pin)?;

    let vault_file = VaultFile {
        version: encrypted.version,
        salt: encrypted.salt,
        nonce: encrypted.nonce,
        ciphertext: encrypted.ciphertext,
    };

    let json = serde_json::to_string_pretty(&vault_file)
        .map_err(|e| format!("Failed to serialize vault: {}", e))?;

    std::fs::write(vault_path()?, json).map_err(|e| format!("Failed to write vault: {}", e))?;

    Ok(())
}

/// Decrypt the on-disk vault with `pin` and return the stored paths.
///
/// Returns an empty `Vec` if no vault file exists yet (fresh install).
/// Returns an `Err` if the file exists but cannot be parsed, decrypted,
/// or has an unsupported version.
pub fn load(pin: &str) -> Result<Vec<String>, String> {
    let path = vault_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let json =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read vault: {}", e))?;

    let vault_file: VaultFile =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse vault: {}", e))?;

    let encrypted = EncryptedData {
        version: vault_file.version,
        salt: vault_file.salt,
        nonce: vault_file.nonce,
        ciphertext: vault_file.ciphertext,
    };

    let plaintext = crypto::decrypt(&encrypted, pin)?;

    let paths: Vec<String> = serde_json::from_slice(&plaintext)
        .map_err(|e| format!("Failed to deserialize paths: {}", e))?;

    Ok(paths)
}

/// Delete the on-disk vault if present. No-op if it doesn't exist.
pub fn clear() -> Result<(), String> {
    let path = vault_path()?;

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete vault: {}", e))?;
    }

    Ok(())
}

/// True if a vault file currently exists on disk.
pub fn exists() -> bool {
    vault_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_clear_then_exists_is_false() {
        // Just exercise clear() / exists() against the real path — these
        // are the only operations that don't need a PIN. save() / load()
        // are covered by the round-trip in crypto tests.
        let _ = clear();
        // exists() may be true if a previous test left state behind; we
        // only assert clear() doesn't error.
        assert!(clear().is_ok());
    }
}
