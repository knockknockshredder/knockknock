// src-tauri/src/shredder/platform/windows.rs

use super::map_drive_type;
use crate::shredder::errors::ShredError;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::MediaType;
use std::path::Path;

pub struct WindowsIo;

impl WindowsIo {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformIo for WindowsIo {
    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // Delegate to the drive module which uses IOCTL_STORAGE_QUERY_PROPERTY
        // (seek-penalty query) to distinguish SSD from HDD on fixed drives,
        // and also handles USB SSD vs USB HDD on removable drives.
        match crate::drive::detect_drive_info(path) {
            Ok(info) => Ok(map_drive_type(info.drive_type)),
            Err(_) => Ok(MediaType::Unknown),
        }
    }
}
