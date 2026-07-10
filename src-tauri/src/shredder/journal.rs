// src-tauri/src/shredder/journal.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub original_path_hash: String,
    pub renamed_path: PathBuf,
    pub timestamp: u64,
}

fn journal_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        .join("KnockKnock")
        .join(".knockknock-journal.json")
}

fn hash_path(path: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn write_orphan(original: &std::path::Path, renamed: &std::path::Path) {
    let entry = JournalEntry {
        original_path_hash: hash_path(original),
        renamed_path: renamed.to_path_buf(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let mut entries = read_orphans();
    entries.push(entry);
    let path = journal_path();
    let tmp = path.with_extension("tmp");
    match serde_json::to_string_pretty(&entries) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&tmp, &json) {
                eprintln!("[KnockKnock] Journal write failed for {:?}: {}", tmp, e);
                return;
            }
            if let Err(e) = std::fs::rename(&tmp, &path) {
                eprintln!(
                    "[KnockKnock] Journal rename failed {:?} -> {:?}: {}",
                    tmp, path, e
                );
            }
        }
        Err(e) => {
            eprintln!("[KnockKnock] Journal serialize failed: {}", e);
        }
    }
}

pub fn clear_orphan(renamed: &std::path::Path) {
    let mut entries = read_orphans();
    entries.retain(|e| e.renamed_path != renamed);
    let path = journal_path();
    let tmp = path.with_extension("tmp");
    match serde_json::to_string_pretty(&entries) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&tmp, &json) {
                eprintln!("[KnockKnock] Journal write failed for {:?}: {}", tmp, e);
                return;
            }
            if let Err(e) = std::fs::rename(&tmp, &path) {
                eprintln!(
                    "[KnockKnock] Journal rename failed {:?} -> {:?}: {}",
                    tmp, path, e
                );
            }
        }
        Err(e) => {
            eprintln!("[KnockKnock] Journal serialize failed: {}", e);
        }
    }
}

pub fn read_orphans() -> Vec<JournalEntry> {
    let path = journal_path();
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn cleanup_orphans() -> Vec<JournalEntry> {
    let entries = read_orphans();
    let mut remaining = Vec::new();
    for entry in entries {
        match std::fs::remove_file(&entry.renamed_path) {
            Ok(_) => clear_orphan(&entry.renamed_path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => clear_orphan(&entry.renamed_path),
            Err(_) => remaining.push(entry),
        }
    }
    remaining
}
