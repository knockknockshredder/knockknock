// src-tauri/src/browser/process.rs

use std::path::Path;

/// Check if a browser is currently running
pub fn is_browser_running(browser_name: &str) -> bool {
    #[cfg(windows)]
    {
        is_process_running_windows(browser_name)
    }

    #[cfg(unix)]
    {
        is_process_running_unix(browser_name)
    }
}

#[cfg(windows)]
fn is_process_running_windows(browser_name: &str) -> bool {
    // Use tasklist to check
    let output = std::process::Command::new("tasklist")
        .args([
            "/FI",
            &format!("IMAGENAME eq {}.exe", browser_name.to_lowercase()),
            "/NH",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            !stdout.contains("No tasks are running")
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
