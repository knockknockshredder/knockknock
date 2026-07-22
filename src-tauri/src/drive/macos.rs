// src-tauri/src/drive/macos.rs

//! macOS-specific drive detection.
//!
//! Classifies drives into SSD / HDD / Network / USB SSD / USB HDD / Unknown
//! by combining:
//!
//! - `df -P <path>` — find the mount point
//! - `df -T <path>` — determine filesystem type (network detection)
//! - `diskutil info -plist <mount_point>` — query SolidState, Protocol, VolumeName
//! - `libc::statvfs` — capacity info

use super::{DriveInfo, DriveType};
use std::ffi::CString;
use std::path::Path;
use std::process::Command;

/// Extract the value of a `<string>` element following a given `<key>` in a
/// plist XML document.
fn plist_string(plist: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{}</key>", key);
    let pos = plist.find(&key_tag)?;
    let rest = &plist[pos + key_tag.len()..];

    // Skip whitespace/newlines between key and value
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")? + start;

    Some(rest[start..end].to_string())
}

/// Extract a boolean `<true/>` / `<false/>` value following a given `<key>`
/// in a plist XML document.
fn plist_bool(plist: &str, key: &str) -> Option<bool> {
    let key_tag = format!("<key>{}</key>", key);
    let pos = plist.find(&key_tag)?;
    let rest = &plist[pos + key_tag.len()..];

    let after_key = rest.trim_start();
    if after_key.starts_with("<true/>") {
        Some(true)
    } else if after_key.starts_with("<false/>") {
        Some(false)
    } else {
        None
    }
}

/// Get the mount point for a path using `df -P`.
fn get_mount_point(path: &Path) -> Result<String, String> {
    let output = Command::new("df")
        .args(["-P", &path.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run df: {}", e))?;

    if !output.status.success() {
        return Err("df command failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();

    // Skip header line
    let _header = lines.next();

    let line = lines.next().ok_or("No mount point found in df output")?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let mount = fields.last().ok_or("Cannot parse mount point")?;

    Ok(mount.to_string())
}

/// Check if a filesystem is a network mount.
fn is_network_mount(path: &Path) -> bool {
    let output = match Command::new("df")
        .args(["-T", &path.to_string_lossy()])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();

    // Skip header line
    let _header = lines.next();

    let line = match lines.next() {
        Some(l) => l.trim(),
        None => return false,
    };

    if line.is_empty() {
        return false;
    }

    // macOS df -T output: "Filesystem  Type  ...  Mounted on"
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 2 {
        return false;
    }

    // Second field is the filesystem type
    let fstype = fields[1];
    matches!(fstype, "nfs" | "nfs4" | "smbfs" | "cifs" | "afp" | "webdav")
}

/// Get capacity via `libc::statvfs`.
fn get_capacity(path: &Path) -> (u64, u64) {
    let path_str = path.to_string_lossy();
    let c_path = match CString::new(path_str.as_ref()) {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let mut stat = unsafe { std::mem::zeroed::<libc::statvfs>() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

    if ret != 0 {
        return (0, 0);
    }

    let frag_size = stat.f_frsize.max(1) as u64;
    let total = frag_size.saturating_mul(stat.f_blocks);
    let free = frag_size.saturating_mul(stat.f_bfree);
    (total, free)
}

/// Build a placeholder `DriveInfo` with Unknown type when detection fails.
fn placeholder_info(path: &Path) -> DriveInfo {
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

    DriveInfo {
        drive_letter,
        drive_type: DriveType::Unknown,
        label: "Unknown".to_string(),
        total_bytes: 0,
        free_bytes: 0,
    }
}

pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    // 1. Find the mount point via df
    let mount_point = match get_mount_point(path) {
        Ok(m) => m,
        Err(_) => return Ok(placeholder_info(path)),
    };

    // 2. Check for network filesystem
    if is_network_mount(path) {
        return Ok(DriveInfo {
            drive_letter: mount_point,
            drive_type: DriveType::Network,
            label: "Network Share".to_string(),
            total_bytes: 0,
            free_bytes: 0,
        });
    }

    // 3. Get detailed info via diskutil
    let diskutil_output = match Command::new("diskutil")
        .args(["info", "-plist", &mount_point])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => {
            // Fallback: return what we have with Unknown type
            let (total_bytes, free_bytes) = get_capacity(path);
            return Ok(DriveInfo {
                drive_letter: mount_point,
                drive_type: DriveType::Unknown,
                label: "Macintosh HD".to_string(),
                total_bytes,
                free_bytes,
            });
        }
    };

    let plist = String::from_utf8_lossy(&diskutil_output.stdout);

    // 4. Parse plist fields
    let solid_state = plist_bool(&plist, "SolidState");
    let protocol = plist_string(&plist, "Protocol");
    let volume_name = plist_string(&plist, "VolumeName");

    // 5. Classify drive type
    let drive_type = match (protocol.as_deref(), solid_state) {
        (Some("USB"), Some(true)) => DriveType::UsbSsd,
        (Some("USB"), Some(false)) => DriveType::UsbHdd,
        (Some("USB"), None) => DriveType::UsbHdd, // conservative: assume HDD
        (_, Some(true)) => DriveType::Ssd,
        (_, Some(false)) => DriveType::Hdd,
        _ => DriveType::Unknown,
    };

    // 6. Capacity via statvfs
    let (total_bytes, free_bytes) = get_capacity(path);

    // 7. Label
    let label = volume_name.unwrap_or_else(|| "Macintosh HD".to_string());

    Ok(DriveInfo {
        drive_letter: mount_point,
        drive_type,
        label,
        total_bytes,
        free_bytes,
    })
}
