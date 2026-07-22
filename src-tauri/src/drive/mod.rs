// src-tauri/src/drive/mod.rs

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

use serde::Serialize;
use std::path::Path;

/// Classification of a physical storage device or mount point.
///
/// Serialized as snake_case so the frontend can use it directly as a
/// string literal type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveType {
    /// NVMe or SATA SSD
    Ssd,
    /// Spinning hard disk
    Hdd,
    /// Network-attached share / UNC path / mapped drive
    Network,
    /// USB-attached SSD
    UsbSsd,
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
/// On Linux, delegates to `linux::detect_drive_info` (sysfs + lsblk).
/// On macOS, delegates to `macos::detect_drive_info` (diskutil plist).
/// On other platforms, returns an Unknown placeholder.
#[cfg(windows)]
pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    windows::detect_drive_info(path)
}

#[cfg(target_os = "linux")]
pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    linux::detect_drive_info(path)
}

#[cfg(target_os = "macos")]
pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    macos::detect_drive_info(path)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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
