// src-tauri/src/shredder/traits.rs

use crate::shredder::errors::ShredError;
use crate::shredder::types::*;
use crate::shredder::verification::PrngSeed;
use std::fs::File;
use std::path::Path;

/// Trait for verification strategies
pub trait VerificationStrategy: Send + Sync {
    fn verify(
        &self,
        file: &mut File,
        expected_pattern: &PatternType,
        file_size: u64,
        seed: Option<&PrngSeed>,
        path: &Path,
    ) -> Result<VerificationResult, ShredError>;
}

/// Trait for progress reporting
pub trait ProgressReporter: Send + Sync {
    fn on_file_start(&self, path: &Path, file_size: u64);
    fn on_pass_start(&self, pass: u32, total_passes: u32);
    fn on_progress(&self, bytes_written: u64, total: u64);
    fn on_pass_complete(&self, pass: u32, total_passes: u32);
    fn on_file_complete(&self, path: &Path, result: &ShredResult, total_passes: u32);
    fn on_error(&self, path: &Path, error: &ShredError);
    fn on_warning(&self, path: &Path, message: &str);
}

/// Trait for platform-specific I/O operations. Only media-type detection
/// survived Phase 4: the legacy open/sync/rename/delete surface was deleted
/// with the legacy pipeline (root execution now uses `SecureTreeIo`).
pub trait PlatformIo: Send + Sync {
    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError>;
}
