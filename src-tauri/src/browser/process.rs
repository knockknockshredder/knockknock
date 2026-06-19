// src-tauri/src/browser/process.rs

use std::path::Path;

/// Check if a browser is currently running
pub fn is_browser_running(
    process_names: &[&str],
    base_path: &Path,
    lock_file_pattern: &str,
) -> bool {
    // First check lock file
    if !lock_file_pattern.is_empty() && check_lock_file(base_path, lock_file_pattern) {
        return true;
    }

    // Then check processes
    for name in process_names {
        #[cfg(windows)]
        {
            if is_process_running_windows(name) {
                return true;
            }
        }

        #[cfg(unix)]
        {
            if is_process_running_unix(name) {
                return true;
            }
        }
    }

    false
}

/// Check for browser lock files
fn check_lock_file(base_path: &Path, pattern: &str) -> bool {
    if pattern.contains('*') {
        // Glob pattern - check the final filename component in each subdirectory
        let final_part = pattern.rsplit('/').next().unwrap_or(pattern);
        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(final_part).exists() {
                    return true;
                }
            }
        }
        false
    } else {
        // Direct lock file
        base_path.join(pattern).exists()
    }
}

#[cfg(windows)]
fn is_process_running_windows(name: &str) -> bool {
    // tasklist IMAGENAME filter requires the .exe suffix; add it if missing.
    let image_name = if name.to_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{}.exe", name)
    };

    // Use tasklist with case-insensitive matching
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", image_name), "/NH"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Check if process is listed (not just "No tasks are running")
            stdout.lines().any(|line| {
                let lower = line.to_lowercase();
                lower.contains(&image_name.to_lowercase())
                    && !lower.contains("no tasks are running")
            })
        }
        Err(_) => false,
    }
}

#[cfg(unix)]
fn is_process_running_unix(browser_name: &str) -> bool {
    let output = std::process::Command::new("pgrep")
        .args(["-f", browser_name])
        .output();

    match output {
        Ok(out) => !out.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Get processes holding a file lock
pub fn get_locking_processes(path: &Path) -> Vec<ProcessInfo> {
    // Platform-specific implementation
    vec![]
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}
