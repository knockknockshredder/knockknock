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
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_WRITE_THROUGH)
            .open(path)
            .map_err(|e| {
                // Distinguish lock-by-process (FileLocked) from pure ACL/privilege
                // denial (PermissionDenied) so the UI can offer the right remedy
                // ("close the holding process" vs "retry as administrator").
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    match self.find_locking_processes(path) {
                        Ok(processes) if !processes.is_empty() => {
                            // Join up to 3 process names so the message stays short.
                            let summary = processes
                                .iter()
                                .take(3)
                                .map(|p| p.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ");
                            ShredError::FileLocked {
                                path: path.to_path_buf(),
                                process: summary,
                            }
                        }
                        _ => ShredError::PermissionDenied(path.to_path_buf()),
                    }
                } else {
                    ShredError::from_io_error(path.to_path_buf(), e)
                }
            })
    }

    fn sync_to_disk(&self, file: &mut File, path: &Path) -> Result<(), ShredError> {
        file.sync_all()
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn rename_random(&self, path: &Path) -> Result<PathBuf, ShredError> {
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut new_path;
        let mut attempts = 0;
        loop {
            let new_name = generate_random_name();
            new_path = parent.join(&new_name);
            if !new_path.exists() {
                break;
            }
            attempts += 1;
            if attempts > 100 {
                return Err(ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "RenameCollision".to_string(),
                    message: "Failed to generate unique random name after 100 attempts".to_string(),
                });
            }
        }
        std::fs::rename(path, &new_path)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
        Ok(new_path)
    }

    fn truncate_to_zero(&self, file: &mut File, path: &Path) -> Result<(), ShredError> {
        file.set_len(0)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn delete(&self, path: &Path) -> Result<(), ShredError> {
        std::fs::remove_file(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // Delegate to the drive module which uses IOCTL_STORAGE_QUERY_PROPERTY
        // (seek-penalty query) to distinguish SSD from HDD on fixed drives,
        // and also handles USB SSD vs USB HDD on removable drives.
        match crate::drive::detect_drive_info(path) {
            Ok(info) => match info.drive_type {
                crate::drive::DriveType::Ssd | crate::drive::DriveType::UsbSsd => {
                    Ok(MediaType::Ssd)
                }
                crate::drive::DriveType::Hdd | crate::drive::DriveType::UsbHdd => {
                    Ok(MediaType::Hdd)
                }
                _ => Ok(MediaType::Unknown),
            },
            Err(_) => Ok(MediaType::Unknown),
        }
    }

    fn find_locking_processes(&self, path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        // Try to open exclusively — if it fails, someone has it locked
        match OpenOptions::new().write(true).share_mode(0).open(path) {
            Ok(_) => Ok(vec![]), // Not locked
            Err(_) => Ok(vec![ProcessInfo {
                pid: 0,
                name: "Unknown (file is locked by another process)".to_string(),
            }]),
        }
    }
}
