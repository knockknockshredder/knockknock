// src-tauri/src/commands/shred.rs

use crate::shredder::algorithms::all_algorithms;
use crate::shredder::progress::TauriProgressReporter;
use crate::shredder::types::*;
use crate::shredder::VerificationLevel;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn shred_files(
    app: AppHandle,
    paths: Vec<String>,
    algorithm_index: usize,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
) -> Result<ShredReport, String> {
    let algorithms = all_algorithms();
    let algorithm = algorithms
        .get(algorithm_index)
        .ok_or_else(|| format!("Invalid algorithm index: {}", algorithm_index))?
        .clone();

    if passes > algorithm.max_passes() {
        return Err(format!("Passes {} exceeds maximum {}", passes, algorithm.max_passes()));
    }

    // Reset cancellation token for fresh operation
    crate::shredder::cancel::reset_global();

    let progress: Arc<dyn crate::shredder::traits::ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app));

    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

    let report = tokio::task::spawn_blocking(move || {
        crate::shredder::shred_files(path_bufs, algorithm, passes, pattern, verification_level, progress)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    Ok(report)
}

#[tauri::command]
pub fn cancel_shred() {
    crate::shredder::cancel::cancel_global();
}

#[tauri::command]
pub fn cleanup_orphans() -> Vec<String> {
    let remaining = crate::shredder::journal::cleanup_orphans();
    remaining
        .iter()
        .map(|e| format!("Orphaned: {:?}", e.renamed_path))
        .collect()
}

#[derive(serde::Serialize)]
pub struct AlgorithmInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub default_passes: u32,
    pub max_passes: u32,
    pub accepted_patterns: Vec<String>,
    pub has_fixed_pattern_sequence: bool,
}

#[tauri::command]
pub fn get_algorithms() -> Vec<AlgorithmInfo> {
    all_algorithms()
        .iter()
        .enumerate()
        .map(|(i, algo)| AlgorithmInfo {
            index: i,
            name: algo.name().to_string(),
            description: algo.description().to_string(),
            default_passes: algo.default_passes(),
            max_passes: algo.max_passes(),
            accepted_patterns: algo.accepted_patterns().iter().map(|p| format!("{:?}", p)).collect(),
            has_fixed_pattern_sequence: algo.has_fixed_pattern_sequence(),
        })
        .collect()
}

#[derive(serde::Serialize)]
pub struct FileMetadata {
    pub path: String,
    pub name: String,
    pub size: u64,
}

/// Collect metadata for a single file
fn collect_file_metadata(path: &std::path::Path, path_str: &str) -> Option<FileMetadata> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return None,
    };

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some(FileMetadata {
        path: path_str.to_string(),
        name,
        size: metadata.len(),
    })
}

/// Recursively collect all files from a directory
fn collect_files_from_dir(
    dir: &std::path::Path,
    valid: &mut Vec<FileMetadata>,
    errors: &mut Vec<String>,
    depth: usize,
) {
    const MAX_DEPTH: usize = 50;
    if depth > MAX_DEPTH {
        errors.push(format!(
            "Max directory depth ({}) exceeded at: {:?}",
            MAX_DEPTH, dir
        ));
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Cannot read directory {:?}: {}", dir, e));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("Cannot stat {:?}: {}", path, e));
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            continue; // Skip symlinks
        }

        if metadata.file_type().is_file() {
            let path_str = path.to_string_lossy().to_string();
            if let Some(meta) = collect_file_metadata(&path, &path_str) {
                valid.push(meta);
            }
        } else if metadata.file_type().is_dir() {
            collect_files_from_dir(&path, valid, errors, depth + 1);
        }
    }
}

#[tauri::command]
pub fn validate_paths(
    paths: Vec<String>,
) -> Result<(Vec<FileMetadata>, Vec<String>), String> {
    let mut valid = Vec::new();
    let mut errors = Vec::new();
    for path_str in paths {
        let path = std::path::Path::new(&path_str);

        // Use symlink_metadata so a symlink to a missing target isn't silently
        // treated as an existing file (path.exists()/is_file() follow links).
        let sym_meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if sym_meta.file_type().is_symlink() {
            // Symlinks are never shredded — skip, never recurse. The actual
            // target is also rejected by validate_path() during shred.
            continue;
        }

        if sym_meta.file_type().is_file() {
            if let Some(meta) = collect_file_metadata(path, &path_str) {
                valid.push(meta);
            }
        } else if sym_meta.file_type().is_dir() {
            collect_files_from_dir(path, &mut valid, &mut errors, 0);
        }
    }
    Ok((valid, errors))
}
