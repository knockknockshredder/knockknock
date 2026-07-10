// src-tauri/src/shredder/traits.rs

use crate::shredder::errors::ShredError;
use crate::shredder::types::*;
use std::fs::File;
use std::path::Path;

/// Trait that all shredding algorithms must implement
pub trait ShredAlgorithm: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn default_passes(&self) -> u32;
    fn max_passes(&self) -> u32;
    fn accepted_patterns(&self) -> &'static [PatternType];

    fn has_fixed_pattern_sequence(&self) -> bool {
        false
    }

    /// For fixed-sequence algorithms, return the pattern used in the final pass.
    /// Default: returns the user-selected pattern.
    fn final_pattern(&self, user_pattern: PatternType) -> PatternType {
        user_pattern
    }

    fn shred(
        &self,
        file: &mut File,
        file_size: u64,
        passes: u32,
        pattern: PatternType,
        progress: &dyn ProgressReporter,
    ) -> Result<ShredResult, ShredError>;
}

/// Trait for verification strategies
pub trait VerificationStrategy: Send + Sync {
    fn verify(
        &self,
        file: &mut File,
        expected_pattern: &PatternType,
        file_size: u64,
    ) -> Result<VerificationResult, ShredError>;
}

/// Trait for progress reporting
pub trait ProgressReporter: Send + Sync {
    fn on_file_start(&self, path: &Path, file_size: u64);
    fn on_pass_start(&self, pass: u32, total_passes: u32);
    fn on_progress(&self, bytes_written: u64, total: u64);
    fn on_pass_complete(&self, pass: u32, total_passes: u32);
    fn on_file_complete(&self, path: &Path, result: &ShredResult);
    fn on_error(&self, path: &Path, error: &ShredError);
    fn on_warning(&self, path: &Path, message: &str);
}

/// Trait for platform-specific I/O operations
pub trait PlatformIo: Send + Sync {
    fn open_for_shred(&self, path: &Path) -> Result<File, ShredError>;
    fn write_data(&self, file: &mut File, data: &[u8]) -> Result<usize, ShredError>;
    fn sync_to_disk(&self, file: &mut File) -> Result<(), ShredError>;
    fn rename_random(&self, path: &Path) -> Result<std::path::PathBuf, ShredError>;
    fn truncate_to_zero(&self, file: &mut File) -> Result<(), ShredError>;
    fn delete(&self, path: &Path) -> Result<(), ShredError>;
    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError>;

    fn schedule_delete_on_reboot(&self, _path: &Path) -> Result<(), ShredError> {
        Err(ShredError::IoError {
            path: _path.to_path_buf(),
            kind: "Unsupported".to_string(),
            message: "Delete-on-reboot not supported on this platform".to_string(),
        })
    }

    fn find_locking_processes(&self, _path: &Path) -> Result<Vec<ProcessInfo>, ShredError> {
        Err(ShredError::IoError {
            path: _path.to_path_buf(),
            kind: "Unsupported".to_string(),
            message: "Process lock detection not supported on this platform".to_string(),
        })
    }

    fn issue_trim(&self, _path: &Path) -> Result<(), ShredError> {
        Ok(()) // Default: no-op, platform can override
    }
}
