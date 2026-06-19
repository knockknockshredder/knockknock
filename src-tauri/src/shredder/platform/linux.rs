// src-tauri/src/shredder/platform/linux.rs

use crate::shredder::errors::ShredError;
use crate::shredder::platform::common::generate_random_name;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::{MediaType, ProcessInfo};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct LinuxIo;

impl LinuxIo {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformIo for LinuxIo {
    fn open_for_shred(&self, path: &Path) -> Result<File, ShredError> {
        OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_SYNC)
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
        let new_path = parent.join(generate_random_name());
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

    fn detect_media_type(&self, _path: &Path) -> Result<MediaType, ShredError> {
        // TODO: Check /sys/block/*/queue/rotational
        Ok(MediaType::Unknown)
    }

    fn find_locking_processes(&self, _path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        // TODO: Implement using /proc/locks or lsof
        Err(ShredError::IoError {
            path: _path.to_path_buf(),
            kind: "NotImplemented".to_string(),
            message: "Process lock detection not yet implemented on Linux".to_string(),
        })
    }

    fn issue_trim(&self, path: &Path) -> Result<(), ShredError> {
        // TODO: Run fstrim on the mount point
        Ok(())
    }
}
