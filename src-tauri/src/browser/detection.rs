// src-tauri/src/browser/detection.rs

use crate::browser::paths::{find_browser_profiles, get_browser_base_paths, BROWSER_PATHS};
use crate::browser::types::*;
use std::path::Path;

/// Map browser name to icon identifier for frontend
fn browser_icon(name: &str) -> String {
    match name {
        "Chrome" => "GoogleChrome",
        "Firefox" => "FirefoxLogo",
        "Edge" => "MicrosoftEdge",
        "Brave" => "BraveLogo",
        "Opera" => "OperaLogo",
        "Vivaldi" => "VivaldiLogo",
        "Safari" => "SafariLogo",
        _ => "Globe",
    }
    .to_string()
}

/// Detect all installed browsers on the system
pub fn detect_browsers() -> Vec<DetectedBrowser> {
    let mut browsers = Vec::new();

    for browser_path in BROWSER_PATHS {
        let base_paths = get_browser_base_paths(browser_path);

        for base_path in base_paths {
            if base_path.exists() {
                // Skip process check — tasklist can hang on some systems.
                // is_running is reported as false; can be checked lazily later.
                let is_running = false;

                // Find all profiles in this browser
                let profile_paths = find_browser_profiles(&base_path, browser_path.profile_glob);
                let mut profiles = Vec::new();

                for profile_path in &profile_paths {
                    if profile_path.exists() {
                        let name = profile_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size = estimate_directory_size(profile_path);

                        profiles.push(BrowserProfile {
                            id: format!(
                                "{}-{}",
                                browser_path.name.to_lowercase(),
                                name.to_lowercase().replace(' ', "-")
                            ),
                            name,
                            path: profile_path.to_string_lossy().to_string(),
                            size,
                            selected: false,
                        });
                    }
                }

                if !profiles.is_empty() {
                    browsers.push(DetectedBrowser {
                        id: browser_path.name.to_lowercase(),
                        name: browser_path.name.to_string(),
                        icon: browser_icon(browser_path.name),
                        is_running,
                        profiles,
                    });
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
