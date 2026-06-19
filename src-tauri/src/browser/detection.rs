// src-tauri/src/browser/detection.rs

use crate::browser::paths::{find_browser_profiles, get_browser_base_paths, BROWSER_PATHS};
use crate::browser::process::is_browser_running;
use crate::browser::types::*;
use std::path::Path;

/// Detect all installed browsers on the system
pub fn detect_browsers() -> Vec<DetectedBrowser> {
    let mut browsers = Vec::new();

    for browser_path in BROWSER_PATHS {
        let base_paths = get_browser_base_paths(browser_path);

        for base_path in base_paths {
            if base_path.exists() {
                // Find all profiles in this browser
                let profiles = find_browser_profiles(&base_path, browser_path.profile_glob);

                for profile_path in profiles {
                    if profile_path.exists() {
                        let is_running = is_browser_running(
                            browser_path.process_names,
                            &base_path,
                            browser_path.lock_file_pattern,
                        );
                        let size = estimate_directory_size(&profile_path);

                        browsers.push(DetectedBrowser {
                            name: browser_path.name.to_string(),
                            profile_path: profile_path.to_string_lossy().to_string(),
                            is_running,
                            data_types: detect_data_types(&profile_path),
                            estimated_size_bytes: size,
                        });
                    }
                }

                break; // Found this browser, move to next
            }
        }
    }

    browsers
}

/// Detect what data types exist in a browser profile
pub fn detect_data_types(profile_path: &Path) -> Vec<BrowserDataType> {
    let mut types = Vec::new();

    if !profile_path.exists() {
        return types;
    }

    types.push(BrowserDataType::Profile);

    // Check for cache
    let cache_names = ["Cache", "cache2", "Code Cache", "GPUCache", "OfflineCache"];
    for name in &cache_names {
        if profile_path.join(name).exists() {
            types.push(BrowserDataType::Cache);
            break;
        }
    }

    // Check for cookies
    let cookie_files = [
        "Cookies",
        "cookies.sqlite",
        "cookies.txt",
        "Network/Cookies",
    ];
    for name in &cookie_files {
        if profile_path.join(name).exists() {
            types.push(BrowserDataType::Cookies);
            break;
        }
    }

    // Check for history
    let history_files = ["History", "places.sqlite", "Favicons"];
    for name in &history_files {
        if profile_path.join(name).exists() {
            types.push(BrowserDataType::History);
            break;
        }
    }

    // Check for passwords
    let password_files = ["Login Data", "logins.json", "signons.sqlite"];
    for name in &password_files {
        if profile_path.join(name).exists() {
            types.push(BrowserDataType::Passwords);
            break;
        }
    }

    types
}

/// Estimate directory size in bytes
pub fn estimate_directory_size(path: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let metadata = entry.metadata();
            if let Ok(meta) = metadata {
                if meta.is_file() {
                    size += meta.len();
                } else if meta.is_dir() {
                    size += estimate_directory_size(&entry.path());
                }
            }
        }
    }
    size
}
