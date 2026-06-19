// src-tauri/src/shredder/algorithms/common.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::ProgressReporter;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

const PROGRESS_INTERVAL: u64 = 1024 * 1024; // Report every 1 MB

/// Write data to file with progress reporting
pub fn write_pass(
    file: &mut File,
    file_size: u64,
    buffer: &[u8],
    progress: &dyn ProgressReporter,
    bytes_written_so_far: u64,
    total_bytes: u64,
) -> Result<u64, ShredError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;

    let mut written = 0u64;
    let mut remaining = file_size;
    let mut last_progress = 0u64;

    while remaining > 0 {
        let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;
        file.write_all(&buffer[..to_write])
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;
        written += to_write as u64;
        remaining -= to_write as u64;

        if written - last_progress >= PROGRESS_INTERVAL {
            progress.on_progress(bytes_written_so_far + written, total_bytes);
            last_progress = written;
        }
    }

    Ok(written)
}
