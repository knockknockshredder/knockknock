// src-tauri/src/shredder/platform/macos.rs

use super::map_drive_type;
use crate::shredder::errors::ShredError;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::MediaType;
use std::path::Path;

pub struct MacOsIo;

impl MacOsIo {
    pub fn new() -> Self {
        Self
    }
}

pub(crate) fn ensure_local_volume(path: &Path) -> Result<(), ShredError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        ShredError::ValidationFailed("volume path contains an embedded NUL byte".to_string())
    })?;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: `c_path` is NUL-terminated and `stats` points to writable
    // storage for the platform's `statfs` structure.
    let result = unsafe { libc::statfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(ShredError::from_io_error(
            path.to_path_buf(),
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: `statfs` returned success and initialized `stats`.
    let stats = unsafe { stats.assume_init() };
    if stats.f_flags & libc::MNT_LOCAL as u32 == 0 {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }
    Ok(())
}

impl PlatformIo for MacOsIo {
    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // Delegate to the drive module for centralized detection.
        match crate::drive::detect_drive_info(path) {
            Ok(info) => Ok(map_drive_type(info.drive_type)),
            Err(_) => Ok(MediaType::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::map_drive_type;
    use crate::drive::DriveType;
    use crate::shredder::types::MediaType;

    #[test]
    fn maps_usb_and_conservative_drive_types() {
        let cases = [
            (DriveType::Hdd, MediaType::Hdd),
            (DriveType::UsbHdd, MediaType::Hdd),
            (DriveType::Ssd, MediaType::Ssd),
            (DriveType::UsbSsd, MediaType::Ssd),
            (DriveType::Unknown, MediaType::Unknown),
            (DriveType::Network, MediaType::Unknown),
        ];

        for (drive_type, expected) in cases {
            assert_eq!(map_drive_type(drive_type), expected);
        }
    }
}
