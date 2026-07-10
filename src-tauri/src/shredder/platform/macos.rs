// src-tauri/src/shredder/platform/macos.rs

use crate::shredder::errors::ShredError;
use crate::shredder::platform::common::generate_random_name;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::{MediaType, ProcessInfo};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

pub struct MacOsIo;

impl MacOsIo {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformIo for MacOsIo {
    fn open_for_shred(&self, path: &Path) -> Result<File, ShredError> {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

        // Set F_NOCACHE to bypass buffer cache
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            unsafe {
                libc::fcntl(fd, libc::F_NOCACHE, 1);
            }
        }

        Ok(file)
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
        let mut new_path;
        let mut attempts = 0;
        loop {
            new_path = parent.join(generate_random_name());
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

    fn truncate_to_zero(&self, file: &mut File) -> Result<(), ShredError> {
        file.set_len(0)
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<open file>"), e))
    }

    fn delete(&self, path: &Path) -> Result<(), ShredError> {
        std::fs::remove_file(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        let output = std::process::Command::new("diskutil")
            .args(["info", "-plist"])
            .arg(path)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("<key>Solid State</key>") && stdout.contains("<true/>") {
                    Ok(MediaType::Ssd)
                } else if stdout.contains("<key>Solid State</key>") && stdout.contains("<false/>") {
                    Ok(MediaType::Hdd)
                } else {
                    Ok(MediaType::Unknown)
                }
            }
            Err(_) => Ok(MediaType::Unknown),
        }
    }

    fn find_locking_processes(&self, _path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        // TODO: Implement using lsof
        Err(ShredError::IoError {
            path: _path.to_path_buf(),
            kind: "NotImplemented".to_string(),
            message: "Process lock detection not yet implemented on macOS".to_string(),
        })
    }
}
