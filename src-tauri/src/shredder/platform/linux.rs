// src-tauri/src/shredder/platform/linux.rs

use crate::shredder::errors::ShredError;
use crate::shredder::platform::common::generate_random_name;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::{MediaType, ProcessInfo};
use std::fs::{File, OpenOptions};
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
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    match self.find_locking_processes(path) {
                        Ok(processes) if !processes.is_empty() => {
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
                        _ => ShredError::from_io_error(path.to_path_buf(), e),
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

    fn truncate_to_zero(&self, file: &mut File, path: &Path) -> Result<(), ShredError> {
        file.set_len(0)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn delete(&self, path: &Path) -> Result<(), ShredError> {
        std::fs::remove_file(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))
    }

    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // Delegate to the drive module for centralized detection.
        match crate::drive::detect_drive_info(path) {
            Ok(info) => match info.drive_type {
                crate::drive::DriveType::Ssd => Ok(MediaType::Ssd),
                crate::drive::DriveType::Hdd => Ok(MediaType::Hdd),
                _ => Ok(MediaType::Unknown),
            },
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
        // Find the mount point for this path so we can pass it to fstrim.
        // `df --output=target` prints the mount point column with the header
        // on the first line, so the actual mount point is line index 1.
        let parent = path.parent().unwrap_or(path);
        let output = std::process::Command::new("df")
            .args(["--output=target", parent.to_str().unwrap_or("")])
            .output();

        let mount_point = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = stdout.trim().lines().collect();
                if lines.len() >= 2 {
                    lines[1].trim().to_string()
                } else {
                    // Couldn't determine mount point — TRIM is best-effort, skip.
                    return Ok(());
                }
            }
            Err(_) => {
                // `df` not available — TRIM is best-effort, skip.
                return Ok(());
            }
        };

        // Run fstrim on the mount point. Requires CAP_SYS_ADMIN or an
        // fstrim-enabled sudoers rule; we don't fail the shred if the user
        // lacks privilege — the file is already gone, TRIM is just an SSD
        // wear-leveling hint to the FTL.
        let trim_result = std::process::Command::new("fstrim")
            .arg("-v")
            .arg(&mount_point)
            .output();

        match trim_result {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    eprintln!("[KnockKnock] fstrim failed for {}: {}", mount_point, stderr);
                    // Don't fail the shred — TRIM is a best-effort optimization.
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("[KnockKnock] fstrim not available: {}", e);
                Ok(()) // TRIM is best-effort
            }
        }
    }
}
