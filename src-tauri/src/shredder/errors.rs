// src-tauri/src/shredder/errors.rs

use serde::Serialize;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum ShredError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("File locked by process '{process}': {path}")]
    FileLocked { path: PathBuf, process: String },

    #[error("I/O error at {path}: {kind}: {message}")]
    IoError {
        path: PathBuf,
        kind: String,
        message: String,
    },

    #[error("Verification failed at pass {pass}: {path}")]
    VerificationFailed { path: PathBuf, pass: u32 },

    #[error("Network drive not supported: {0}")]
    NetworkDrive(PathBuf),

    #[error("System file protected: {0}")]
    SystemFile(PathBuf),

    #[error("Symlink detected: {0}")]
    SymlinkDetected(PathBuf),

    #[error("Path is not a file or directory: {0}")]
    InvalidPathType(PathBuf),

    #[error("Empty path")]
    EmptyPath,
}

impl ShredError {
    pub fn from_io_error(path: PathBuf, error: std::io::Error) -> Self {
        ShredError::IoError {
            path,
            kind: format!("{:?}", error.kind()),
            message: error.to_string(),
        }
    }
}
