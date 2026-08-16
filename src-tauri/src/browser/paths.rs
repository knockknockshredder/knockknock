// src-tauri/src/browser/paths.rs

use crate::browser::types::BrowserRunningStatus;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileLayout {
    Direct,
    Subdirectory(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowsRootPreference {
    LocalAppDataFirst,
    AppDataFirst,
}

pub struct BrowserPath {
    pub id: &'static str,
    pub name: &'static str,
    pub windows_paths: &'static [&'static str],
    pub(crate) windows_root_preference: WindowsRootPreference,
    pub macos_paths: &'static [&'static str],
    pub linux_paths: &'static [&'static str],
    pub profile_glob: &'static str, // Glob pattern for profiles
    /// Layout of profile directories relative to the browser base.
    pub profile_layout: ProfileLayout,
    /// Every browser must declare an explicit policy. Unsupported policies are
    /// intentionally `Unknown`, never an implicit closed state.
    pub running_detection: BrowserRunningDetection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserRunningDetection {
    ChromiumUserData,
    GeckoProfile,
    Unsupported,
}

pub const BROWSER_PATHS: &[BrowserPath] = &[
    BrowserPath {
        id: "chrome",
        name: "Chrome",
        windows_paths: &[
            "Google\\Chrome\\User Data",
            "Google\\Chrome Beta\\User Data",
            "Google\\Chrome SxS\\User Data", // Chrome Canary
        ],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["Google/Chrome"],
        linux_paths: &["google-chrome"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "firefox",
        name: "Firefox",
        windows_paths: &["Mozilla\\Firefox"],
        windows_root_preference: WindowsRootPreference::AppDataFirst,
        macos_paths: &["Firefox"],
        linux_paths: &[".mozilla/firefox"],
        profile_glob: "*.default*",
        #[cfg(target_os = "linux")]
        profile_layout: ProfileLayout::Direct,
        #[cfg(not(target_os = "linux"))]
        profile_layout: ProfileLayout::Subdirectory("Profiles"),
        running_detection: BrowserRunningDetection::GeckoProfile,
    },
    BrowserPath {
        id: "edge",
        name: "Edge",
        windows_paths: &[
            "Microsoft\\Edge\\User Data",
            "Microsoft\\Edge Beta\\User Data",
        ],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["Microsoft Edge"],
        linux_paths: &["microsoft-edge"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "brave",
        name: "Brave",
        windows_paths: &[
            "BraveSoftware\\Brave-Browser\\User Data",
            "BraveSoftware\\Brave-Browser-Beta\\User Data",
        ],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["BraveSoftware/Brave-Browser"],
        linux_paths: &["BraveSoftware/Brave-Browser"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "opera",
        name: "Opera",
        windows_paths: &[
            "Opera Software\\Opera Stable",
            "Opera Software\\Opera Next", // Opera Beta
        ],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["com.operasoftware.Opera"],
        linux_paths: &["opera"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "vivaldi",
        name: "Vivaldi",
        windows_paths: &["Vivaldi\\User Data"],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["Vivaldi"],
        linux_paths: &["vivaldi"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "safari",
        name: "Safari",
        windows_paths: &[], // Safari not on Windows
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &[
            "Safari",
            "Caches/com.apple.Safari",
            "Caches/com.apple.Safari.WebClips",
            "Containers/com.apple.Safari",
            "WebKit",
            "Saved Application State/com.apple.Safari.savedState",
        ],
        linux_paths: &[], // Safari not on Linux
        profile_glob: "",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::Unsupported,
    },
    BrowserPath {
        id: "tor browser",
        name: "Tor Browser",
        windows_paths: &["Tor Browser\\Browser\\TorBrowser\\Data\\Browser"],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["TorBrowser/Data/Browser"],
        linux_paths: &[".tor-browser"],
        profile_glob: "*.default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::GeckoProfile,
    },
    BrowserPath {
        id: "chromium",
        name: "Chromium",
        windows_paths: &["Chromium\\User Data"],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &["Chromium"],
        linux_paths: &["chromium"],
        profile_glob: "Default",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::ChromiumUserData,
    },
    BrowserPath {
        id: "internet explorer",
        name: "Internet Explorer",
        windows_paths: &["Microsoft\\Internet Explorer"],
        windows_root_preference: WindowsRootPreference::LocalAppDataFirst,
        macos_paths: &[], // IE not on macOS
        linux_paths: &[], // IE not on Linux
        profile_glob: "",
        profile_layout: ProfileLayout::Direct,
        running_detection: BrowserRunningDetection::Unsupported,
    },
];

pub(crate) fn windows_base_paths_for_roots(
    browser: &BrowserPath,
    local_app_data: Option<&Path>,
    app_data: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for win_path in browser.windows_paths {
        let roots = match browser.windows_root_preference {
            WindowsRootPreference::LocalAppDataFirst => [local_app_data, app_data],
            WindowsRootPreference::AppDataFirst => [app_data, local_app_data],
        };

        for root in roots.into_iter().flatten() {
            paths.push(root.join(win_path));
        }
    }

    paths
}

pub fn get_browser_base_paths(browser: &BrowserPath) -> Vec<PathBuf> {
    // Windows: LOCALAPPDATA / APPDATA are only set on Windows.
    let local_app_data = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
    let app_data = std::env::var("APPDATA").ok().map(PathBuf::from);
    let mut paths =
        windows_base_paths_for_roots(browser, local_app_data.as_deref(), app_data.as_deref());

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
pub fn find_browser_profiles(base_path: &Path, browser: &BrowserPath) -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    let profile_root = match browser.profile_layout {
        ProfileLayout::Direct => base_path.to_path_buf(),
        ProfileLayout::Subdirectory(directory) => base_path.join(directory),
    };

    let Ok(base_metadata) = std::fs::symlink_metadata(&profile_root) else {
        return profiles;
    };
    if !base_metadata.is_dir() || is_link_metadata(&base_metadata) {
        return profiles;
    }

    // Look for profile directories
    if let Ok(entries) = std::fs::read_dir(&profile_root) {
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
                || (browser.profile_glob.contains('*') && name.contains("default"))
            {
                profiles.push(path);
            }
        }
    }

    // If no profiles found, use base path itself
    if profiles.is_empty() && matches!(browser.profile_layout, ProfileLayout::Direct) {
        profiles.push(base_path.to_path_buf());
    }

    profiles
}

pub fn browser_by_id(id: &str) -> Option<&'static BrowserPath> {
    BROWSER_PATHS.iter().find(|browser| browser.id == id)
}

pub fn browser_running_state(browser_id: &str, profile_path: &Path) -> BrowserRunningStatus {
    let Some(browser) = browser_by_id(browser_id) else {
        return BrowserRunningStatus::Unknown;
    };
    browser.running_state(profile_path)
}

impl BrowserPath {
    pub fn running_state(&self, profile_path: &Path) -> BrowserRunningStatus {
        let metadata = match std::fs::symlink_metadata(profile_path) {
            Ok(metadata) => metadata,
            Err(_) => return BrowserRunningStatus::Unknown,
        };
        if !metadata.is_dir() || is_link_metadata(&metadata) {
            return BrowserRunningStatus::Unknown;
        }

        match self.running_detection {
            BrowserRunningDetection::ChromiumUserData => chromium_running_state(profile_path),
            BrowserRunningDetection::GeckoProfile => gecko_running_state(profile_path),
            BrowserRunningDetection::Unsupported => BrowserRunningStatus::Unknown,
        }
    }
}

fn lock_candidate_present(path: &Path) -> Result<bool, std::io::Error> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn lock_present_in(directories: &[&Path], names: &[&str]) -> BrowserRunningStatus {
    let mut unknown = false;
    for directory in directories {
        for name in names {
            match lock_candidate_present(&directory.join(name)) {
                Ok(true) => return BrowserRunningStatus::Running,
                Ok(false) => {}
                Err(_) => unknown = true,
            }
        }
    }
    if unknown {
        BrowserRunningStatus::Unknown
    } else {
        BrowserRunningStatus::Closed
    }
}

fn chromium_running_state(profile_path: &Path) -> BrowserRunningStatus {
    #[cfg(windows)]
    {
        let mut unknown = false;
        for directory in [Some(profile_path), profile_path.parent()]
            .into_iter()
            .flatten()
        {
            let lock_path = directory.join("lockfile");
            match lock_candidate_present(&lock_path) {
                Ok(false) => {}
                Ok(true) => match std::fs::OpenOptions::new().write(true).open(lock_path) {
                    Ok(_) => {}
                    Err(error) if error.raw_os_error() == Some(32) => {
                        return BrowserRunningStatus::Running;
                    }
                    Err(_) => unknown = true,
                },
                Err(_) => unknown = true,
            }
        }
        return if unknown {
            BrowserRunningStatus::Unknown
        } else {
            BrowserRunningStatus::Closed
        };
    }

    #[cfg(not(windows))]
    {
        let directories = [profile_path, profile_path.parent().unwrap_or(profile_path)];
        lock_present_in(&directories, &["SingletonLock"])
    }
}

fn gecko_running_state(profile_path: &Path) -> BrowserRunningStatus {
    #[cfg(windows)]
    {
        let lock_path = profile_path.join("parent.lock");
        return match lock_candidate_present(&lock_path) {
            Ok(false) => BrowserRunningStatus::Closed,
            Ok(true) => match std::fs::OpenOptions::new().write(true).open(lock_path) {
                Ok(_) => BrowserRunningStatus::Closed,
                Err(error) if error.raw_os_error() == Some(32) => BrowserRunningStatus::Running,
                Err(_) => BrowserRunningStatus::Unknown,
            },
            Err(_) => BrowserRunningStatus::Unknown,
        };
    }

    #[cfg(not(windows))]
    {
        lock_present_in(&[profile_path], &[".parentlock", "lock", "parent.lock"])
    }
}
