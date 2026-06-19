// src-tauri/src/browser/paths.rs

use std::path::PathBuf;

pub struct BrowserPath {
    pub name: &'static str,
    pub windows: Option<&'static str>,
    pub macos: Option<&'static str>,
    pub linux: Option<&'static str>,
}

pub const BROWSER_PATHS: &[BrowserPath] = &[
    BrowserPath {
        name: "Chrome",
        windows: Some("Google\\Chrome\\User Data"),
        macos: Some("Google/Chrome"),
        linux: Some("google-chrome"),
    },
    BrowserPath {
        name: "Firefox",
        windows: Some("Mozilla\\Firefox\\Profiles"),
        macos: Some("Firefox/Profiles"),
        linux: Some(".mozilla/firefox"),
    },
    BrowserPath {
        name: "Edge",
        windows: Some("Microsoft\\Edge\\User Data"),
        macos: Some("Microsoft Edge"),
        linux: Some("microsoft-edge"),
    },
    BrowserPath {
        name: "Brave",
        windows: Some("BraveSoftware\\Brave-Browser\\User Data"),
        macos: Some("BraveSoftware/Brave-Browser"),
        linux: Some("BraveSoftware/Brave-Browser"),
    },
    BrowserPath {
        name: "Opera",
        windows: Some("Opera Software\\Opera Stable"),
        macos: Some("com.operasoftware.Opera"),
        linux: Some("opera"),
    },
    BrowserPath {
        name: "Vivaldi",
        windows: Some("Vivaldi\\User Data"),
        macos: Some("Vivaldi"),
        linux: Some("vivaldi"),
    },
    BrowserPath {
        name: "Safari",
        windows: None,
        macos: Some("Safari"),
        linux: None,
    },
];

pub fn get_browser_base_paths(browser: &BrowserPath) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        if let Some(local) = std::env::var("LOCALAPPDATA").ok() {
            if let Some(win_path) = browser.windows {
                paths.push(PathBuf::from(local).join(win_path));
            }
        }
        if let Some(roaming) = std::env::var("APPDATA").ok() {
            if let Some(win_path) = browser.windows {
                paths.push(PathBuf::from(roaming).join(win_path));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var("HOME").ok() {
            if let Some(mac_path) = browser.macos {
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
        if let Some(home) = std::env::var("HOME").ok() {
            if let Some(linux_path) = browser.linux {
                paths.push(PathBuf::from(home).join(".config").join(linux_path));
                paths.push(PathBuf::from(home).join(linux_path));
            }
        }
    }

    paths
}
