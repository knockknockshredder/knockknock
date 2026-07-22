// src-tauri/src/drive/linux.rs

//! Linux-specific drive detection.
//!
//! Classifies drives into SSD / HDD / Network / USB SSD / USB HDD / Unknown
//! by combining:
//!
//! - `df --output=source,target,fstype` — find device, mount point, fstype
//! - `/sys/block/<device>/queue/rotational` — 0 = SSD, 1 = HDD
//! - `/sys/block/<device>/removable` — 1 = removable (USB)
//! - `lsblk` / `blkid` — volume label
//! - `libc::statvfs` — capacity info

use super::{DriveInfo, DriveType};
use std::ffi::CString;
use std::path::Path;
use std::process::Command;

/// Extract the base block device name from a partition device path.
///
/// Examples:
/// - `/dev/sda1`   → `sda`
/// - `/dev/nvme0n1p1` → `nvme0n1`
/// - `/dev/mmcblk0p1` → `mmcblk0`
/// - `/dev/vda1`   → `vda`
fn base_device_name(device: &str) -> Option<String> {
    let name = device.strip_prefix("/dev/")?;
    if name.is_empty() {
        return None;
    }

    // Handle NVMe / MMC partition pattern: name ends with `p<digits>`
    // e.g. nvme0n1p1 → strip trailing digits → nvme0n1p → strip trailing p → nvme0n1
    let stripped = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if stripped.len() < name.len() && stripped.ends_with('p') {
        let base = stripped.trim_end_matches('p');
        if !base.is_empty() {
            return Some(base.to_string());
        }
    }

    // Handle sdX1, vda1, etc.: strip trailing digits
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if base.is_empty() {
        return None;
    }
    Some(base.to_string())
}

/// Read a sysfs attribute as a `u8`.
fn read_sysfs_u8(path: &str) -> Option<u8> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
}

/// Get the filesystem label via `lsblk` or `blkid`.
fn get_label(device: &str) -> String {
    // Try lsblk first (fast, no root needed for most devices)
    if let Ok(output) = Command::new("lsblk")
        .args(["-no", "LABEL", device])
        .output()
    {
        if output.status.success() {
            let label = String::from_utf8_lossy(&output.stdout);
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Fallback to blkid
    if let Ok(output) = Command::new("blkid")
        .args(["-s", "LABEL", "-o", "value", device])
        .output()
    {
        if output.status.success() {
            let label = String::from_utf8_lossy(&output.stdout);
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    "Linux Drive".to_string()
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

/// Extract the mount point from `df -P` output.
fn parse_df_output(path: &Path) -> Option<(String, String, String)> {
    let output = Command::new("df")
        .args(["--output=source,target,fstype", &path.to_string_lossy()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();

    // Skip header line
    let _header = lines.next();

    let line = lines.next()?.trim();
    if line.is_empty() {
        return None;
    }

    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 3 {
        return None;
    }

    Some((
        fields[0].to_string(), // source device
        fields[1].to_string(), // mount point
        fields[2].to_string(), // filesystem type
    ))
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
    // 1. Find device, mount point, and fstype via df
    let (source, mount_point, fstype) = match parse_df_output(path) {
        Some(info) => info,
        None => return Ok(placeholder_info(path)),
    };

    // 2. Check for network filesystem
    if matches!(
        fstype.as_str(),
        "nfs" | "nfs4" | "cifs" | "smb" | "smb2" | "fuse.sshfs"
    ) {
        return Ok(DriveInfo {
            drive_letter: mount_point,
            drive_type: DriveType::Network,
            label: "Network Share".to_string(),
            total_bytes: 0,
            free_bytes: 0,
        });
    }

    // 3. Only physical block devices have /dev/ prefix
    if !source.starts_with("/dev/") {
        return Ok(placeholder_info(path));
    }

    // 4. Extract base device name
    let base = match base_device_name(&source) {
        Some(b) => b,
        None => return Ok(placeholder_info(path)),
    };

    // 5. Read sysfs attributes
    let rotational = read_sysfs_u8(&format!("/sys/block/{}/queue/rotational", base));
    let removable = read_sysfs_u8(&format!("/sys/block/{}/removable", base));

    // 6. Classify
    let drive_type = match (rotational, removable) {
        (Some(0), Some(1)) => DriveType::UsbSsd, // removable + SSD
        (Some(1), Some(1)) => DriveType::UsbHdd, // removable + HDD
        (Some(0), _) => DriveType::Ssd,          // non-removable + SSD
        (Some(1), _) => DriveType::Hdd,          // non-removable + HDD
        _ => DriveType::Unknown,
    };

    // 7. Get label
    let label = get_label(&source);

    // 8. Get capacity via statvfs
    let (total_bytes, free_bytes) = get_capacity(path);

    Ok(DriveInfo {
        drive_letter: mount_point,
        drive_type,
        label,
        total_bytes,
        free_bytes,
    })
}
