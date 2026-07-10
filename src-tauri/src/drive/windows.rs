// src-tauri/src/drive/windows.rs

//! Windows-specific drive detection.
//!
//! Uses `GetDriveTypeW` to classify the drive and `GetVolumeInformationW`
//! + `GetDiskFreeSpaceExW` to retrieve the volume label and capacity.
//!
//! SSD vs HDD is NOT reliably distinguishable without an IOCTL round-trip
//! (StorageDeviceProperty + bus type), so fixed drives are reported as
//! `DriveType::Unknown`. The frontend is free to refine this with a
//! follow-up command if a more precise signal is required.

use super::{DriveInfo, DriveType};
use std::path::Path;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
};

const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;

pub fn detect_drive_info(path: &Path) -> Result<DriveInfo, String> {
    let path_str = path.to_string_lossy();

    // UNC path: "\\server\share" - treat as a network share.
    if path_str.starts_with("\\\\") || path_str.starts_with("//") {
        return Ok(DriveInfo {
            drive_letter: "Network".to_string(),
            drive_type: DriveType::Network,
            label: "Network Share".to_string(),
            total_bytes: 0,
            free_bytes: 0,
        });
    }

    // Drive-letter path: "C:" or "C:\..."
    let drive_root = if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
        format!("{}:\\", &path_str[..1])
    } else {
        return Err(format!("Cannot determine drive for path: {}", path_str));
    };

    let drive_wide: Vec<u16> = drive_root
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let drive_type_raw = unsafe { GetDriveTypeW(drive_wide.as_ptr()) };

    // Volume label
    let mut label_buf = [0u16; 256];
    let label = unsafe {
        GetVolumeInformationW(
            drive_wide.as_ptr(),
            label_buf.as_mut_ptr(),
            label_buf.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    let label = if label != 0 {
        let nul_pos = label_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(label_buf.len());
        String::from_utf16_lossy(&label_buf[..nul_pos])
    } else {
        "Local Disk".to_string()
    };

    // Capacity
    let mut total_bytes: u64 = 0;
    let mut free_bytes: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            drive_wide.as_ptr(),
            std::ptr::null_mut(),
            &mut total_bytes as *mut u64,
            &mut free_bytes as *mut u64,
        )
    };
    // Silence unused-result warning if the cast ever needs cleanup.
    let _ = ok as BOOL;

    // Map raw drive type to our enum. SSD vs HDD for DRIVE_FIXED is left
    // as Unknown — accurate detection requires IOCTL_STORAGE_QUERY_PROPERTY
    // (not enabled in our windows-sys feature set).
    let media_type = match drive_type_raw {
        DRIVE_REMOTE => DriveType::Network,
        DRIVE_REMOVABLE => DriveType::UsbHdd,
        DRIVE_FIXED => DriveType::Unknown,
        DRIVE_CDROM => DriveType::Unknown,
        DRIVE_RAMDISK => DriveType::Unknown,
        DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR | _ => DriveType::Unknown,
    };

    Ok(DriveInfo {
        drive_letter: drive_root.trim_end_matches('\\').to_string(),
        drive_type: media_type,
        label,
        total_bytes,
        free_bytes,
    })
}
