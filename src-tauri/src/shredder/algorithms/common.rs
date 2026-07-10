// src-tauri/src/shredder/algorithms/common.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::PatternType;
use getrandom::getrandom;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

const PROGRESS_INTERVAL: u64 = 1024 * 1024; // Report every 1 MB
const REGENERATE_EVERY: u64 = 4 * 1024 * 1024; // Regenerate random every 4 MB

/// Write data to file with progress reporting.
///
/// For `PatternType::Random`, the buffer is regenerated every 4 MB so the
/// same pattern is not repeated across the entire file. For fixed patterns
/// (Zeros, Ones) the buffer is used as-is.
pub fn write_pass(
    file: &mut File,
    file_size: u64,
    pattern: PatternType,
    buffer: &mut [u8],
    progress: &dyn ProgressReporter,
    bytes_written_so_far: u64,
    total_bytes: u64,
) -> Result<u64, ShredError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;

    let mut written = 0u64;
    let mut remaining = file_size;
    let mut last_progress = 0u64;
    let mut since_regen = 0u64;

    while remaining > 0 {
        // Regenerate random buffer periodically for true randomness
        if pattern == PatternType::Random && since_regen >= REGENERATE_EVERY {
            getrandom(buffer).map_err(|e| ShredError::IoError {
                path: PathBuf::from("<random>"),
                kind: "RandomGeneration".to_string(),
                message: e.to_string(),
            })?;
            since_regen = 0;
        }

        let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;
        file.write_all(&buffer[..to_write])
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;
        written += to_write as u64;
        remaining -= to_write as u64;
        since_regen += to_write as u64;

        if written - last_progress >= PROGRESS_INTERVAL {
            progress.on_progress(bytes_written_so_far + written, total_bytes);
            last_progress = written;
        }
    }

    Ok(written)
}
