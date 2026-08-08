// src-tauri/src/shredder/platform/mod.rs

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::shredder::traits::PlatformIo;
use crate::shredder::types::MediaType;

/// Map drive detection's platform-specific categories to the shredder's
/// storage policy categories. USB transport does not change the underlying
/// rotational-versus-solid-state classification.
pub(crate) fn map_drive_type(drive_type: crate::drive::DriveType) -> MediaType {
    match drive_type {
        crate::drive::DriveType::Hdd | crate::drive::DriveType::UsbHdd => MediaType::Hdd,
        crate::drive::DriveType::Ssd | crate::drive::DriveType::UsbSsd => MediaType::Ssd,
        crate::drive::DriveType::Network | crate::drive::DriveType::Unknown => MediaType::Unknown,
    }
}

pub fn create_platform_io() -> Box<dyn PlatformIo> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsIo::new())
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsIo::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxIo::new())
    }
}
