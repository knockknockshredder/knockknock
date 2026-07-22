// src-tauri/src/drive/windows.rs

//! Windows-specific drive detection.
//!
//! Classifies drives into SSD / HDD / Network / USB SSD / USB HDD / Unknown
//! by combining `GetDriveTypeW` with an `IOCTL_STORAGE_QUERY_PROPERTY`
//! round-trip that queries the seek-penalty property.
//!
//! For `DRIVE_FIXED` the IOCTL distinguishes SSD (no seek penalty) from HDD.
//! For `DRIVE_REMOVABLE` the same IOCTL distinguishes USB SSD from USB HDD.
//! When the IOCTL fails (e.g. permission denied on some external enclosures,
//! or a non-storage device) we fall back to `GetDriveTypeW` alone, reporting
//! the drive as `DriveType::Unknown`.

use super::{DriveInfo, DriveType};
use std::mem::size_of;
use std::path::Path;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
};

// Use the `windows` (not `windows-sys`) crate for IOCTL types and calls because
// they provide better type safety for the storage query structures.  These are
// kept in a separate import block to distinguish them from the platform-API
// calls above.
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
    IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
};
use windows::Win32::System::IO::DeviceIoControl;

const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;

/// Query whether a physical drive has seek penalty via
/// `IOCTL_STORAGE_QUERY_PROPERTY` + `StorageDeviceSeekPenaltyProperty`.
///
/// Returns:
/// - `Some(true)`  → seek penalty incurred → spinning media (HDD)
/// - `Some(false)` → no seek penalty       → flash media (SSD)
/// - `None`        → query failed          → caller should fall back
fn query_seek_penalty(drive_letter: &str) -> Option<bool> {
    // Build \\.\X: device path (e.g. \\.\C:)
    // drive_letter is already "C:" — don't add a second colon.
    let device_path = format!("\\\\.\\{}", drive_letter);
    let device_wide: Vec<u16> = device_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = match CreateFileW(
            PCWSTR(device_wide.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None::<HANDLE>,
        ) {
            Ok(h) => h,
            Err(_) => {
                eprintln!(
                    "[KnockKnock] Failed to open volume handle for {}: CreateFileW error",
                    device_path
                );
                return None;
            }
        };

        if handle == INVALID_HANDLE_VALUE {
            eprintln!(
                "[KnockKnock] Failed to open volume handle for {}: INVALID_HANDLE_VALUE",
                device_path
            );
            return None;
        }

        // Build STORAGE_PROPERTY_QUERY for seek-penalty property.
        let mut query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceSeekPenaltyProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0; 1],
        };

        let mut result = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
        let mut bytes_returned: u32 = 0;

        let status = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&mut query as *mut STORAGE_PROPERTY_QUERY as *const core::ffi::c_void),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(&mut result as *mut DEVICE_SEEK_PENALTY_DESCRIPTOR as *mut core::ffi::c_void),
            size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            Some(&mut bytes_returned as *mut u32),
            None,
        );

        let _ = CloseHandle(handle);

        if status.is_ok() && bytes_returned >= size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32 {
            // IncursSeekPenalty == true  ⟹ HDD (has seek penalty)
            // IncursSeekPenalty == false ⟹ SSD (no seek penalty)
            Some(result.IncursSeekPenalty)
        } else {
            eprintln!(
                "[KnockKnock] IOCTL_STORAGE_QUERY_PROPERTY failed for {}: status={:?}, bytes_returned={}",
                device_path, status, bytes_returned
            );
            None
        }
    }
}

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
    // Silence unused-result warning.
    let _ = ok as BOOL;

    // Classify drive: use GetDriveTypeW for the broad category, then refine
    // fixed/removable drives with an IOCTL seek-penalty query.
    let drive_letter = drive_root.trim_end_matches('\\').to_string();
    let media_type = match drive_type_raw {
        DRIVE_REMOTE => DriveType::Network,
        DRIVE_REMOVABLE => {
            // Removable drive — likely USB. Query seek penalty to
            // distinguish USB SSD from USB HDD.
            match query_seek_penalty(&drive_letter) {
                Some(false) => DriveType::UsbSsd,
                Some(true) | None => DriveType::UsbHdd,
            }
        }
        DRIVE_FIXED => {
            // Fixed drive — query seek penalty to distinguish SSD from HDD.
            match query_seek_penalty(&drive_letter) {
                Some(false) => DriveType::Ssd,
                Some(true) => DriveType::Hdd,
                // IOCTL failed — report Unknown rather than guessing.
                None => DriveType::Unknown,
            }
        }
        DRIVE_CDROM | DRIVE_RAMDISK | DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR | _ => DriveType::Unknown,
    };

    Ok(DriveInfo {
        drive_letter,
        drive_type: media_type,
        label,
        total_bytes,
        free_bytes,
    })
}
