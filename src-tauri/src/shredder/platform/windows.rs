// src-tauri/src/shredder/platform/windows.rs

use crate::shredder::errors::ShredError;
use crate::shredder::platform::common::generate_random_name;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::{MediaType, ProcessInfo};
use std::fs::{File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

// Windows constants
const FILE_FLAG_WRITE_THROUGH: u32 = 0x80000000;
const FILE_SHARE_DELETE: u32 = 0x00000004;
const FILE_SHARE_READ: u32 = 0x00000001;
const FILE_SHARE_WRITE: u32 = 0x00000002;

pub struct WindowsIo;

impl WindowsIo {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformIo for WindowsIo {
    fn open_for_shred(&self, path: &Path) -> Result<File, ShredError> {
        OpenOptions::new()
            .write(true)
            .custom_flags(FILE_FLAG_WRITE_THROUGH)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(path)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn write_data(&self, file: &mut File, data: &[u8]) -> Result<usize, ShredError> {
        use std::io::Write;
        file.write(data)
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<open file>"), e))
    }

    fn sync_to_disk(&self, file: &mut File) -> Result<(), ShredError> {
        file.sync_all()
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<open file>"), e))
    }

    fn rename_random(&self, path: &Path) -> Result<PathBuf, ShredError> {
        let parent = path.parent().unwrap_or(Path::new("."));
        let new_name = generate_random_name();
        let new_path = parent.join(&new_name);
        std::fs::rename(path, &new_path)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
        Ok(new_path)
    }

    fn truncate_to_zero(&self, file: &mut File) -> Result<(), ShredError> {
        file.set_len(0)
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<open file>"), e))
    }

    fn delete(&self, path: &Path) -> Result<(), ShredError> {
        std::fs::remove_file(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // TODO: Implement via GetDriveTypeW
        Ok(MediaType::Unknown)
    }

    fn schedule_delete_on_reboot(&self, path: &Path) -> Result<(), ShredError> {
        // TODO: Implement MoveFileExW with MOVEFILE_DELAY_UNTIL_REBOOT
        Err(ShredError::IoError {
            path: path.to_path_buf(),
            kind: "NotImplemented".to_string(),
            message: "Delete-on-reboot not yet implemented".to_string(),
        })
    }
}
