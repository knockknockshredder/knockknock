// src-tauri/src/browser/paths.rs

use std::path::PathBuf;

pub struct BrowserPath {
    pub name: &'static str,
    pub process_names: &'static [&'static str], // Process names to check
    pub windows_paths: &'static [&'static str], // Windows paths (can have multiple)
    pub macos_paths: &'static [&'static str],   // macOS paths
    pub linux_paths: &'static [&'static str],   // Linux paths
    pub lock_file_pattern: &'static str,        // Glob pattern for lock file
    pub profile_glob: &'static str,             // Glob pattern for profiles
}

pub const BROWSER_PATHS: &[BrowserPath] = &[
    BrowserPath {
        name: "Chrome",
        process_names: &["chrome", "chrome.exe"],
        windows_paths: &[
            "Google\\Chrome\\User Data",
            "Google\\Chrome Beta\\User Data",
            "Google\\Chrome SxS\\User Data", // Chrome Canary
        ],
        macos_paths: &[
            "Google/Chrome",
            "Google/Chrome Beta",
            "Google/Chrome Canary",
        ],
        linux_paths: &[
            "google-chrome",
            "google-chrome-beta",
            "google-chrome-unstable",
        ],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Firefox",
        process_names: &["firefox", "firefox.exe"],
        windows_paths: &["Mozilla\\Firefox"],
        macos_paths: &["Firefox"],
        linux_paths: &[
            ".mozilla/firefox",
            "snap/firefox/common/.mozilla/firefox", // Snap
            ".var/app/org.mozilla.firefox/.mozilla/firefox", // Flatpak
        ],
        lock_file_pattern: "*.default*/lock",
        profile_glob: "*.default*",
    },
    BrowserPath {
        name: "Edge",
        process_names: &["msedge", "msedge.exe"],
        windows_paths: &[
            "Microsoft\\Edge\\User Data",
            "Microsoft\\Edge Beta\\User Data",
        ],
        macos_paths: &["Microsoft Edge", "Microsoft Edge Beta"],
        linux_paths: &[
            "microsoft-edge",
            "microsoft-edge-beta",
            "microsoft-edge-dev",
        ],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Brave",
        process_names: &["brave", "brave.exe"],
        windows_paths: &[
            "BraveSoftware\\Brave-Browser\\User Data",
            "BraveSoftware\\Brave-Browser-Beta\\User Data",
        ],
        macos_paths: &[
            "BraveSoftware/Brave-Browser",
            "BraveSoftware/Brave-Browser-Beta",
        ],
        linux_paths: &[
            "BraveSoftware/Brave-Browser",
            "BraveSoftware/Brave-Browser-Beta",
            "snap/brave/current/.config/BraveSoftware/Brave-Browser", // Snap
        ],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Opera",
        process_names: &["opera", "opera.exe"],
        windows_paths: &[
            "Opera Software\\Opera Stable",
            "Opera Software\\Opera Next", // Opera Beta
        ],
        macos_paths: &["com.operasoftware.Opera"],
        linux_paths: &["opera", "opera-beta", "opera-developer"],
        lock_file_pattern: "lock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Vivaldi",
        process_names: &["vivaldi", "vivaldi.exe"],
        windows_paths: &["Vivaldi\\User Data"],
        macos_paths: &["Vivaldi"],
        linux_paths: &["vivaldi", "vivaldi-snapshot"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Safari",
        process_names: &["Safari"],
        windows_paths: &[], // Safari not on Windows
        macos_paths: &["Safari"],
        linux_paths: &[], // Safari not on Linux
        lock_file_pattern: "",
        profile_glob: "",
    },
    BrowserPath {
        name: "Tor Browser",
        process_names: &["tor-browser", "firefox"], // Tor uses Firefox under the hood
        windows_paths: &["Tor Browser\\Browser\\TorBrowser\\Data\\Browser"],
        macos_paths: &["TorBrowser-Data/Browser"],
        linux_paths: &[
            ".local/share/torbrowser/tbb/x86_64/tor-browser/Browser/TorBrowser/Data/Browser",
            "tor-browser/Browser/TorBrowser/Data/Browser",
        ],
        lock_file_pattern: "parent.lock",
        profile_glob: "*.default",
    },
    BrowserPath {
        name: "Chromium",
        process_names: &["chromium", "chromium.exe"],
        windows_paths: &["Chromium\\User Data"],
        macos_paths: &["Chromium"],
        linux_paths: &[
            "chromium",
            "snap/chromium/common/chromium",                   // Snap
            ".var/app/org.chromium.Chromium/.config/chromium", // Flatpak
        ],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Internet Explorer",
        process_names: &["iexplore", "iexplore.exe"],
        windows_paths: &["Microsoft\\Internet Explorer"],
        macos_paths: &[], // IE not on macOS
        linux_paths: &[], // IE not on Linux
        lock_file_pattern: "",
        profile_glob: "",
    },
];

pub fn get_browser_base_paths(browser: &BrowserPath) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        for win_path in browser.windows_paths {
            if let Some(local) = std::env::var("LOCALAPPDATA").ok() {
                paths.push(PathBuf::from(local).join(win_path));
            }
            if let Some(roaming) = std::env::var("APPDATA").ok() {
                paths.push(PathBuf::from(roaming).join(win_path));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var("HOME").ok() {
            for mac_path in browser.macos_paths {
                paths.push(
                    PathBuf::from(home)
                        .join("Library/Application Support")
                        .join(mac_path),
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/.config", home)
        });

        for linux_path in browser.linux_paths {
            if linux_path.starts_with('.') {
                // Relative to HOME (e.g. ~/.mozilla/firefox)
                if let Some(home) = std::env::var("HOME").ok() {
                    paths.push(PathBuf::from(home).join(linux_path));
                }
            } else if linux_path.starts_with("snap/") || linux_path.starts_with(".var/") {
                // Snap or Flatpak paths (e.g. ~/snap/firefox/...)
                if let Some(home) = std::env::var("HOME").ok() {
                    paths.push(PathBuf::from(home).join(linux_path));
                }
            } else {
                // Relative to XDG_CONFIG_HOME (e.g. $XDG_CONFIG_HOME/google-chrome)
                paths.push(PathBuf::from(&config_home).join(linux_path));
            }
        }
    }

    paths
}

/// Find all profile directories for a browser
pub fn find_browser_profiles(base_path: &PathBuf, profile_glob: &str) -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    if !base_path.exists() {
        return profiles;
    }

    // Look for profile directories
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                // Check if matches profile pattern
                if name == "Default"
                    || name.starts_with("Profile ")
                    || (profile_glob.contains('*') && name.contains("default"))
                {
                    profiles.push(path);
                }
            }
        }
    }

    // If no profiles found, use base path itself
    if profiles.is_empty() {
        profiles.push(base_path.clone());
    }

    profiles
}
