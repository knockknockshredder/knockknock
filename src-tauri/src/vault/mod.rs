// src-tauri/src/vault/mod.rs
//
// Encrypted vault for session persistence (Task 6).
//
// Stores the user's pending shred list as an AES-256-GCM-encrypted JSON
// blob, keyed by their PIN. PIN never touches disk in plaintext — only
// the PBKDF2-SHA256 derived AES key (transient) and the salt (stored).

pub mod crypto;
pub mod storage;
