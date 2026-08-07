// src-tauri/src/browser/paths.rs

use std::path::PathBuf;

/// Whether `symlink_metadata` describes a filesystem link that destructive
/// traversal must never follow (M9): Unix symlinks, and Windows symlinks or
/// reparse-point directories (junctions). On Windows, `file_type().is_symlink()`
/// already reflects `FILE_ATTRIBUTE_REPARSE_POINT`, but the attribute is
/// checked explicitly so junction detection does not depend on that mapping.
pub(crate) fn is_link_metadata(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub struct BrowserPath {
    pub name: &'static str,
    pub windows_paths: &'static [&'static str],
    pub macos_paths: &'static [&'static str],
    pub linux_paths: &'static [&'static str],
    pub lock_file_pattern: &'static str, // Glob pattern for lock file
    pub profile_glob: &'static str,      // Glob pattern for profiles
}

pub const BROWSER_PATHS: &[BrowserPath] = &[
    BrowserPath {
        name: "Chrome",
        windows_paths: &[
            "Google\\Chrome\\User Data",
            "Google\\Chrome Beta\\User Data",
            "Google\\Chrome SxS\\User Data", // Chrome Canary
        ],
        macos_paths: &["Google/Chrome"],
        linux_paths: &["google-chrome"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Firefox",
        windows_paths: &["Mozilla\\Firefox"],
        macos_paths: &["Firefox"],
        linux_paths: &[".mozilla/firefox"],
        lock_file_pattern: "*.default*/lock",
        profile_glob: "*.default*",
    },
    BrowserPath {
        name: "Edge",
        windows_paths: &[
            "Microsoft\\Edge\\User Data",
            "Microsoft\\Edge Beta\\User Data",
        ],
        macos_paths: &["Microsoft Edge"],
        linux_paths: &["microsoft-edge"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Brave",
        windows_paths: &[
            "BraveSoftware\\Brave-Browser\\User Data",
            "BraveSoftware\\Brave-Browser-Beta\\User Data",
        ],
        macos_paths: &["BraveSoftware/Brave-Browser"],
        linux_paths: &["BraveSoftware/Brave-Browser"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Opera",
        windows_paths: &[
            "Opera Software\\Opera Stable",
            "Opera Software\\Opera Next", // Opera Beta
        ],
        macos_paths: &["com.operasoftware.Opera"],
        linux_paths: &["opera"],
        lock_file_pattern: "lock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Vivaldi",
        windows_paths: &["Vivaldi\\User Data"],
        macos_paths: &["Vivaldi"],
        linux_paths: &["vivaldi"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Safari",
        windows_paths: &[], // Safari not on Windows
        macos_paths: &[
            "Safari",
            "Caches/com.apple.Safari",
            "Caches/com.apple.Safari.WebClips",
            "Containers/com.apple.Safari",
            "WebKit",
            "Saved Application State/com.apple.Safari.savedState",
        ],
        linux_paths: &[], // Safari not on Linux
        lock_file_pattern: "",
        profile_glob: "",
    },
    BrowserPath {
        name: "Tor Browser",
        windows_paths: &["Tor Browser\\Browser\\TorBrowser\\Data\\Browser"],
        macos_paths: &["TorBrowser/Data/Browser"],
        linux_paths: &[".tor-browser"],
        lock_file_pattern: "parent.lock",
        profile_glob: "*.default",
    },
    BrowserPath {
        name: "Chromium",
        windows_paths: &["Chromium\\User Data"],
        macos_paths: &["Chromium"],
        linux_paths: &["chromium"],
        lock_file_pattern: "SingletonLock",
        profile_glob: "Default",
    },
    BrowserPath {
        name: "Internet Explorer",
        windows_paths: &["Microsoft\\Internet Explorer"],
        macos_paths: &[], // IE not on macOS
        linux_paths: &[], // IE not on Linux
        lock_file_pattern: "",
        profile_glob: "",
    },
];

pub fn get_browser_base_paths(browser: &BrowserPath) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Windows: LOCALAPPDATA / APPDATA are only set on Windows.
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

    // macOS: paths are relative to ~/Library/Application Support.
    {
        if let Some(home) = std::env::var("HOME").ok() {
            for mac_path in browser.macos_paths {
                paths.push(
                    PathBuf::from(&home)
                        .join("Library/Application Support")
                        .join(mac_path),
                );
            }
        }
    }

    // Linux: dot-prefixed paths are HOME-relative; others are XDG_CONFIG_HOME-relative.
    {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/.config", home)
        });

        for linux_path in browser.linux_paths {
            if linux_path.starts_with('.') {
                if let Some(home) = std::env::var("HOME").ok() {
                    paths.push(PathBuf::from(home).join(linux_path));
                }
            } else if linux_path.starts_with("snap/") || linux_path.starts_with(".var/") {
                if let Some(home) = std::env::var("HOME").ok() {
                    paths.push(PathBuf::from(home).join(linux_path));
                }
            } else {
                paths.push(PathBuf::from(&config_home).join(linux_path));
            }
        }
    }

    paths
}

/// Find all profile directories for a browser
///
/// Discovery is non-destructive, so inspection failures here degrade to an
/// empty candidate set; the destructive collectors (commands/browser.rs) are
/// the fail-loud boundary. Profile entries that are symlinks or Windows
/// reparse points are never accepted as profiles (M9).
pub fn find_browser_profiles(base_path: &PathBuf, profile_glob: &str) -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    let Ok(base_metadata) = std::fs::symlink_metadata(base_path) else {
        return profiles;
    };
    if !base_metadata.is_dir() || is_link_metadata(&base_metadata) {
        return profiles;
    }

    // Look for profile directories
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_dir() || is_link_metadata(&metadata) {
                continue;
            }
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

    // If no profiles found, use base path itself
    if profiles.is_empty() {
        profiles.push(base_path.clone());
    }

    profiles
}
