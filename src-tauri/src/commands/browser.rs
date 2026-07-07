// src-tauri/src/commands/browser.rs

use crate::browser;
use crate::browser::types::*;
use crate::shredder::types::ShredReport;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn detect_browsers() -> Result<Vec<DetectedBrowser>, String> {
    eprintln!("[detect_browsers command] called");
    let result = tokio::task::spawn_blocking(|| browser::detection::detect_browsers())
        .await
        .map_err(|e| format!("Detection panicked: {:?}", e))?;
    eprintln!("[detect_browsers command] returning {} browsers", result.len());
    Ok(result)
}

#[tauri::command]
pub async fn shred_browser_data(
    app: AppHandle,
    request: BrowserShredRequest,
) -> Result<ShredReport, String> {
    use crate::shredder::algorithms::all_algorithms;
    use crate::shredder::progress::TauriProgressReporter;

    eprintln!(
        "[shred_browser_data] called for browser={}, profile={}",
        request.browser_name, request.profile_path
    );

    let profile_path = PathBuf::from(&request.profile_path);
    if !profile_path.exists() {
        return Err(format!(
            "Profile path does not exist: {}",
            request.profile_path
        ));
    }

    // Safety: refuse to shred browser data while the browser is running
    // unless the user has explicitly acknowledged the warning.
    if check_browser_lock_file(&profile_path) && !request.explicit_consent {
        return Err(format!(
            "Browser {} is currently running. Close it first or acknowledge the warning.",
            request.browser_name
        ));
    }

    // Collect files to shred based on selected data types
    let mut files_to_shred = Vec::new();

    for data_type in &request.data_types {
        let files = collect_browser_data_files(&profile_path, data_type);
        files_to_shred.extend(files);
    }

    if files_to_shred.is_empty() {
        return Err("No browser data files found to shred".to_string());
    }

    eprintln!(
        "[shred_browser_data] found {} files to shred",
        files_to_shred.len()
    );

    // Use the SAME algorithm settings as file shredding
    let algorithms = all_algorithms();
    let algorithm = algorithms
        .get(request.algorithm_index)
        .ok_or_else(|| format!("Invalid algorithm index: {}", request.algorithm_index))?
        .clone();

    if request.passes > algorithm.max_passes() {
        return Err(format!(
            "Passes {} exceeds maximum {}",
            request.passes,
            algorithm.max_passes()
        ));
    }

    let progress: Arc<dyn crate::shredder::traits::ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app));

    let report = tokio::task::spawn_blocking(move || {
        crate::shredder::shred_files(
            files_to_shred,
            algorithm,
            request.passes,
            request.pattern,
            request.verification_level,
            progress,
        )
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    eprintln!(
        "[shred_browser_data] complete: {} successful, {} failed",
        report.successful, report.failed
    );

    Ok(report)
}

/// Collect files for a specific browser data type
fn collect_browser_data_files(
    profile_path: &std::path::Path,
    data_type: &BrowserDataType,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    match data_type {
        BrowserDataType::Cache => {
            let cache_dirs = ["Cache", "cache2", "Code Cache", "GPUCache", "OfflineCache"];
            for dir in &cache_dirs {
                let cache_path = profile_path.join(dir);
                if cache_path.exists() {
                    collect_files_recursive(&cache_path, &mut files);
                }
            }
        }
        BrowserDataType::Cookies => {
            let cookie_files = [
                "Cookies",
                "cookies.sqlite",
                "cookies.txt",
                "Network/Cookies",
                "Cookies-journal",
            ];
            for name in &cookie_files {
                let path = profile_path.join(name);
                if path.exists() && path.is_file() {
                    files.push(path);
                }
            }
        }
        BrowserDataType::History => {
            let history_files = [
                "History",
                "History-journal",
                "places.sqlite",
                "places.sqlite-wal",
                "places.sqlite-shm",
                "Favicons",
            ];
            for name in &history_files {
                let path = profile_path.join(name);
                if path.exists() && path.is_file() {
                    files.push(path);
                }
            }
        }
        BrowserDataType::Passwords => {
            let password_files = [
                "Login Data",
                "Login Data-journal",
                "logins.json",
                "signons.sqlite",
                "key4.db",
            ];
            for name in &password_files {
                let path = profile_path.join(name);
                if path.exists() && path.is_file() {
                    files.push(path);
                }
            }
        }
        BrowserDataType::Extensions => {
            let ext_path = profile_path.join("Extensions");
            if ext_path.exists() {
                collect_files_recursive(&ext_path, &mut files);
            }
        }
        BrowserDataType::Profile => {
            // Shred entire profile (except Extensions which are re-downloadable)
            collect_files_recursive_excluding(profile_path, &mut files, &["Extensions"]);
        }
    }

    files
}

/// Recursively collect all files from a directory
fn collect_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_files_recursive(&path, files);
        }
    }
}

/// Recursively collect files excluding specified directories
fn collect_files_recursive_excluding(
    dir: &std::path::Path,
    files: &mut Vec<PathBuf>,
    exclude_dirs: &[&str],
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !exclude_dirs.contains(&dir_name.as_str()) {
                collect_files_recursive_excluding(&path, files, exclude_dirs);
            }
        }
    }
}

/// Detect if a browser is running by looking for lock files in the profile directory.
/// Chromium-based browsers create `SingletonLock` (or `lock`) while running;
/// Firefox creates `.parentlock`. Returns true if any lock file is present.
fn check_browser_lock_file(profile_path: &std::path::Path) -> bool {
    const LOCK_FILES: &[&str] = &["SingletonLock", "lock", ".parentlock"];
    for lock_name in LOCK_FILES {
        if profile_path.join(lock_name).exists() {
            return true;
        }
    }
    if let Some(parent) = profile_path.parent() {
        if parent.join("SingletonLock").exists() {
            return true;
        }
    }
    false
}
