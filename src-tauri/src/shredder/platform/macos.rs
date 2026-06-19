// src-tauri/src/shredder/platform/macos.rs
// Stub - implemented in later task

use crate::shredder::errors::ShredError;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::{MediaType, ProcessInfo};
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct MacOsIo;

impl MacOsIo {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformIo for MacOsIo {
    fn open_for_shred(&self, _path: &Path) -> Result<File, ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn write_data(&self, _file: &mut File, _data: &[u8]) -> Result<usize, ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn sync_to_disk(&self, _file: &mut File) -> Result<(), ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn rename_random(&self, _path: &Path) -> Result<PathBuf, ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn truncate_to_zero(&self, _file: &mut File) -> Result<(), ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn delete(&self, _path: &Path) -> Result<(), ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn detect_media_type(&self, _path: &Path) -> Result<MediaType, ShredError> {
        Err(ShredError::IoError {
            path: PathBuf::new(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }

    fn find_locking_processes(&self, path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        Err(ShredError::IoError {
            path: path.to_path_buf(),
            kind: "Unimplemented".to_string(),
            message: "MacOsIo not yet implemented".to_string(),
        })
    }
}
