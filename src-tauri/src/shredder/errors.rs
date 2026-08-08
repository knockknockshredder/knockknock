// src-tauri/src/shredder/errors.rs

use crate::shredder::types::{DeletionMethod, MediaType};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
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
    // --- v2 variants (Phase 1, additive). Technical Display only, no UI
    // prose (M11) — presentation copy belongs to the frontend. ---
    // `HardLinkBlocked` is constructed by root-execution preflight (Phase 2);
    // `UnsupportedStorageForMethod` by storage preflight (Phase 3).
    // `BrowserCollectionFailed` is constructed by the browser collectors
    // (Phase 3).
    #[error("hard link blocked at {path}: link count {count}")]
    HardLinkBlocked { path: PathBuf, count: u64 },

    #[error("unsupported storage for method {method:?} at {path}: media type {media:?}")]
    UnsupportedStorageForMethod {
        path: PathBuf,
        method: DeletionMethod,
        media: MediaType,
    },

    #[error("write check failed at {path}")]
    WriteCheckFailed { path: PathBuf },

    #[error("browser data collection failed at {path}: {detail}")]
    BrowserCollectionFailed { path: PathBuf, detail: String },

    #[error("Network drive not supported: {0}")]
    NetworkDrive(PathBuf),

    #[error("System file protected: {0}")]
    SystemFile(PathBuf),

    #[error("Shortcut or symlink detected: {path} -> {target}. Enable 'Also shred linked targets' to shred the target.")]
    ShortcutDetected { path: PathBuf, target: String },

    #[error("Path is not a file or directory: {0}")]
    InvalidPathType(PathBuf),

    #[error("Empty path")]
    EmptyPath,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
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

#[derive(Debug, Error, Serialize)]
pub enum JournalError {
    #[error("journal I/O failed while {operation} at {path}: {message}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },

    #[error("journal serialization failed: {0}")]
    Serialize(String),

    #[error("journal decode failed at {path}: {message}")]
    Decode { path: PathBuf, message: String },

    #[error("legacy path-only journal record at {path} is not trusted for recovery")]
    LegacyRecord { path: PathBuf },

    #[error("journal record identity mismatch at {path}: {reason}")]
    IdentityMismatch { path: PathBuf, reason: String },

    #[error("journal record has an unsafe recovery parent: {path}: {reason}")]
    UnsafeParent { path: PathBuf, reason: String },

    #[error("journal record was not found while clearing: {path}")]
    RecordNotFound { path: PathBuf },
}

impl JournalError {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Io { path, .. }
            | Self::Decode { path, .. }
            | Self::LegacyRecord { path }
            | Self::IdentityMismatch { path, .. }
            | Self::UnsafeParent { path, .. }
            | Self::RecordNotFound { path } => Some(path),
            Self::Serialize(_) => None,
        }
    }
}
