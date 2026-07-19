// src-tauri/src/shredder/journal.rs

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub original_path_hash: String,
    pub renamed_path: PathBuf,
    pub timestamp: u64,
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(crate::paths::portable_data_dir()?.join(".knockknock-journal.json"))
}

fn hash_path(path: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Atomic write with fsync — orphan tracking is critical-state, not cosmetic.
/// If this fails the journal is silent data corruption.
fn write_journal_atomic(entries: &[JournalEntry]) -> Result<(), String> {
    let path = journal_path()?;
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Journal serialize failed: {e}"))?;

    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);

    let mut file =
        std::fs::File::create(&tmp).map_err(|e| format!("Journal tmp create failed: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Journal tmp write failed: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Journal fsync failed: {e}"))?;
    drop(file);

    std::fs::rename(&tmp, &path).map_err(|e| format!("Journal rename failed: {e}"))
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
    if let Err(e) = write_journal_atomic(&entries) {
        eprintln!("[KnockKnock] Journal write failed: {e}");
    }
}

pub fn clear_orphan(renamed: &std::path::Path) {
    let mut entries = read_orphans();
    entries.retain(|e| e.renamed_path != renamed);
    if let Err(e) = write_journal_atomic(&entries) {
        eprintln!("[KnockKnock] Journal clear failed: {e}");
    }
}

pub fn read_orphans() -> Vec<JournalEntry> {
    let path = match journal_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Auto-expire journal entries older than this. Stops the journal from
/// accumulating dead entries forever if the cleanup path never runs (e.g.
/// app killed mid-shred on the same file repeatedly).
const JOURNAL_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

pub fn cleanup_orphans() -> Vec<JournalEntry> {
    let entries = read_orphans();
    let mut remaining = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for entry in entries {
        // Drop entries whose renamed file may have disappeared (TTL expired or
        // a clean shred was followed by an external delete). Either way, the
        // rename target is uninteresting past 7 days.
        if now.saturating_sub(entry.timestamp) > JOURNAL_TTL_SECS {
            clear_orphan(&entry.renamed_path);
            continue;
        }

        match std::fs::remove_file(&entry.renamed_path) {
            Ok(_) => {
                clear_orphan(&entry.renamed_path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                clear_orphan(&entry.renamed_path);
            }
            Err(_) => remaining.push(entry),
        }
    }
    remaining
}
