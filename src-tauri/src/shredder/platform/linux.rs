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
        let output = std::process::Command::new("df")
            .args(["--output=source", path.to_str().unwrap_or("")])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = stdout.trim().lines().collect();
                if lines.len() >= 2 {
                    let device = lines[1].trim();
                    let dev_name = device.split('/').last().unwrap_or("");
                    let base_dev = dev_name.trim_end_matches(|c: char| c.is_ascii_digit());
                    let rotational_path = format!("/sys/block/{}/queue/rotational", base_dev);
                    if let Ok(rot) = std::fs::read_to_string(&rotational_path) {
                        let rot = rot.trim();
                        if rot == "0" {
                            return Ok(MediaType::Ssd);
                        } else if rot == "1" {
                            return Ok(MediaType::Hdd);
                        }
                    }
                }
                Ok(MediaType::Unknown)
            }
            Err(_) => Ok(MediaType::Unknown),
        }
    }

    fn find_locking_processes(&self, path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        let path_str = path.to_string_lossy();
        let output = std::process::Command::new("lsof")
            .arg(&*path_str)
            .output()
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes = Vec::new();
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                processes.push(ProcessInfo {
                    pid: parts[1].parse().unwrap_or(0),
                    name: parts[0].to_string(),
                });
            }
        }
        Ok(processes)
    }

    fn issue_trim(&self, path: &Path) -> Result<(), ShredError> {
        // TODO: Run fstrim on the mount point
        Ok(())
    }
}
