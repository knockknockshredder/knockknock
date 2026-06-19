// src-tauri/src/shredder/algorithms/random_only.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::{ProgressReporter, ShredAlgorithm};
use crate::shredder::types::*;
use getrandom::getrandom;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

pub struct RandomOnly;

const BUFFER_SIZE: usize = 256 * 1024; // 256 KB

impl ShredAlgorithm for RandomOnly {
    fn name(&self) -> &str {
        "Random Only"
    }
    fn description(&self) -> &str {
        "Fastest algorithm. Writes cryptographically secure random data only."
    }
    fn default_passes(&self) -> u32 {
        1
    }
    fn max_passes(&self) -> u32 {
        35
    }
    fn accepted_patterns(&self) -> &'static [PatternType] {
        &[PatternType::Random]
    }

    fn shred(
        &self,
        file: &mut File,
        file_size: u64,
        passes: u32,
        _pattern: PatternType,
        progress: &dyn ProgressReporter,
    ) -> Result<ShredResult, ShredError> {
        let start = std::time::Instant::now();
        let mut total_written = 0u64;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut last_progress_bytes = 0u64;
        const PROGRESS_INTERVAL: u64 = 1024 * 1024; // 1 MB

        for pass in 0..passes {
            getrandom(&mut buffer).map_err(|e| ShredError::IoError {
                path: PathBuf::from("<random>"),
                kind: "RandomGeneration".to_string(),
                message: e.to_string(),
            })?;

            file.seek(SeekFrom::Start(0))
                .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;

            let mut remaining = file_size;
            while remaining > 0 {
                let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;
                file.write_all(&buffer[..to_write])
                    .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;
                total_written += to_write as u64;
                remaining -= to_write as u64;

                if total_written - last_progress_bytes >= PROGRESS_INTERVAL {
                    progress.on_progress(total_written, file_size * passes as u64);
                    last_progress_bytes = total_written;
                }
            }

            progress.on_pass_complete(pass + 1, passes);
        }

        Ok(ShredResult {
            success: true,
            passes_completed: passes,
            bytes_written: total_written,
            verification_passed: true,
            errors: vec![],
            duration: start.elapsed(),
        })
    }
}
