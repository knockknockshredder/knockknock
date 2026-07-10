// src-tauri/src/vault/crypto.rs
//
// AES-256-GCM encryption with PBKDF2-SHA256 key derivation.
// Used by the encrypted vault for session persistence (Task 6).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;

/// OWASP 2023 recommendation: 600,000+ iterations for PBKDF2-SHA256.
/// Using 1,000,000 for extra security on a file-shredder app where the
/// vault is the only barrier between an attacker and the user's pending
/// shred list. Iteration count is intentionally high.
const PBKDF2_ITERATIONS: u32 = 1_000_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Vault format version. Bumped whenever the on-disk layout changes so
/// old vaults can be migrated or rejected explicitly.
pub const VAULT_VERSION: u32 = 1;

/// Serialized encrypted blob. Stored on disk as JSON inside `VaultFile`.
#[derive(Debug, Clone)]
pub struct EncryptedData {
    pub version: u32,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Derive a 256-bit AES key from the user's PIN and a salt.
///
/// Uses PBKDF2-HMAC-SHA256 with [`PBKDF2_ITERATIONS`].
pub fn derive_key(pin: &str, salt: &[u8]) -> Vec<u8> {
    let mut key = vec![0u8; 32]; // 256 bits
    pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// Encrypt `plaintext` under a key derived from `pin`.
///
/// A fresh random salt and nonce are generated per call; both are required
/// to decrypt later. The salt is stored alongside the ciphertext so we can
/// re-derive the same key from the user's PIN.
pub fn encrypt(plaintext: &[u8], pin: &str) -> Result<EncryptedData, String> {
    let mut salt = vec![0u8; SALT_LEN];
    let mut nonce_bytes = vec![0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key(pin, &salt);

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to create cipher: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    Ok(EncryptedData {
        version: VAULT_VERSION,
        salt,
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Decrypt `encrypted` using a key derived from `pin`.
///
/// AEAD authentication means decryption will fail if the PIN is wrong OR if
/// the ciphertext has been tampered with on disk.
pub fn decrypt(encrypted: &EncryptedData, pin: &str) -> Result<Vec<u8>, String> {
    if encrypted.version != VAULT_VERSION {
        return Err(format!(
            "Unsupported vault version: {} (expected {})",
            encrypted.version, VAULT_VERSION
        ));
    }

    let key = derive_key(pin, &encrypted.salt);

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to create cipher: {}", e))?;
    let nonce = Nonce::from_slice(&encrypted.nonce);

    cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|e| format!("Decryption failed (wrong PIN or corrupted vault): {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_succeeds() {
        let pin = "123456";
        let plaintext = b"hello vault";
        let encrypted = encrypt(plaintext, pin).expect("encrypt");
        let decrypted = decrypt(&encrypted, pin).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_pin_fails() {
        let encrypted = encrypt(b"secret", "correct-pin").expect("encrypt");
        let err = decrypt(&encrypted, "wrong-pin").expect_err("should fail");
        assert!(err.contains("Decryption failed"));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let mut encrypted = encrypt(b"secret", "pin-1234").expect("encrypt");
        // Flip a byte deep in the ciphertext. AEAD auth tag must reject.
        let last = encrypted.ciphertext.len() - 1;
        encrypted.ciphertext[last] ^= 0x01;
        let err = decrypt(&encrypted, "pin-1234").expect_err("should fail");
        assert!(err.contains("Decryption failed"));
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let mut encrypted = encrypt(b"x", "123456").expect("encrypt");
        encrypted.version = VAULT_VERSION + 1;
        let err = decrypt(&encrypted, "123456").expect_err("should fail");
        assert!(err.contains("Unsupported vault version"));
    }

    #[test]
    fn derive_key_is_deterministic() {
        let salt = b"some-fixed-salt-1234";
        let a = derive_key("123456", salt);
        let b = derive_key("123456", salt);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_key_differs_per_pin() {
        let salt = b"some-fixed-salt-1234";
        let a = derive_key("111111", salt);
        let b = derive_key("222222", salt);
        assert_ne!(a, b);
    }
}
