// src-tauri/src/drive/mod.rs

#[cfg(windows)]
pub mod windows;

use serde::Serialize;
use std::path::Path;

/// Classification of a physical storage device or mount point.
///
/// Serialized as snake_case so the frontend can use it directly as a
/// string literal type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveType {
    /// Network-attached share / UNC path / mapped drive
    Network,
    /// USB-attached HDD
    UsbHdd,
    /// Unknown / not detected
    Unknown,
}

/// Information about a drive or mount point.
#[derive(Debug, Clone, Serialize)]
pub struct DriveInfo {
    /// Drive letter (`"C:"` on Windows) or mount key (`"/Users"` on Unix)
    pub drive_letter: String,
    /// Classified drive type
    pub drive_type: DriveType,
    /// Human-readable label (e.g. `"Local Disk"`, `"USB Drive"`)
    pub label: String,
    /// Total capacity in bytes (0 if unavailable)
    pub total_bytes: u64,
    /// Free space in bytes (0 if unavailable)
    pub free_bytes: u64,
}

/// Platform-aware drive detection dispatcher.
///
/// On Windows, delegates to `windows::detect_drive_info` (real OS query).
/// On macOS/Linux, returns an Unknown placeholder derived from the path
/// prefix — accurate detection on those platforms would require a
/// dedicated implementation that the v0.3.0 plan does not require.
#[cfg(windows)]
pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    windows::detect_drive_info(path)
}

#[cfg(not(windows))]
pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    let path_str = path.to_string_lossy();
    let drive_letter = if path_str.starts_with('/') {
        let parts: Vec<&str> = path_str.split('/').filter(|s| !s.is_empty()).collect();
        match parts.first() {
            Some(first) => format!("/{}", first),
            None => "/".to_string(),
        }
    } else {
        "Unknown".to_string()
    };

    Ok(DriveInfo {
        drive_letter,
        drive_type: DriveType::Unknown,
        label: "Unknown".to_string(),
        total_bytes: 0,
        free_bytes: 0,
    })
}
