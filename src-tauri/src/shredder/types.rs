// src-tauri/src/shredder/types.rs

use crate::shredder::errors::ShredError;
use serde::{Deserialize, Serialize};

/// Byte patterns for overwriting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    Random,
    Zeros,
    Ones,
}

impl PatternType {
    // PatternType is used for serialization/deserialization
}

/// Media type for SSD-aware shredding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Ssd,
    Unknown,
}

/// Status of a shredding operation
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ShredStatus {
    Shredding,
    Complete,
    Warning { message: String },
    Error { message: String },
}

/// Result of a single file shredding operation
#[derive(Debug)]
pub struct ShredResult {
    pub success: bool,
    pub passes_completed: u32,
    pub bytes_written: u64,
    pub errors: Vec<ShredError>,
}

/// Result of verification
#[derive(Debug)]
pub struct VerificationResult {
    pub passed: bool,
}

/// Information about a hard link
#[derive(Debug)]
pub struct HardLinkInfo {
    pub link_count: u32,
}

/// Information about a process holding a file lock
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Summary report after batch shredding
#[derive(Debug, Serialize)]
pub struct ShredReport {
    pub total_files: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: Vec<ShredReportError>,
    pub total_bytes_shredded: u64,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct ShredReportError {
    pub path: String,
    pub error: String,
}

/// Progress event sent to frontend
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub file_path: String,
    pub file_size: u64,
    pub bytes_written: u64,
    pub current_pass: u32,
    pub total_passes: u32,
    pub speed_bytes_per_sec: u64,
    pub estimated_time_remaining_secs: u64,
    pub status: ShredStatus,
}

/// Verification levels (user-configurable)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationLevel {
    None,
    Sample,
    Full,
}

/// Metadata returned to the frontend for each file discovered during
/// `validate_paths`.
///
/// `is_shortcut` flags `.lnk` shell shortcuts, NTFS symlinks, junctions, and
/// Unix symlinks — any path whose target would survive the link's destruction.
/// `shortcut_target` is the resolved target path when classification found one.
#[derive(Debug, Clone, Serialize)]
pub struct FileMetadata {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub is_shortcut: bool,
    pub shortcut_target: Option<String>,
}
