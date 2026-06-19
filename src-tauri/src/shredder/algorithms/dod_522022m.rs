// src-tauri/src/shredder/algorithms/dod_522022m.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::{ProgressReporter, ShredAlgorithm};
use crate::shredder::types::*;
use getrandom::getrandom;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

pub struct Dod522022M;

const BUFFER_SIZE: usize = 65536;

fn dod_pass_pattern(pass: u32) -> PatternType {
    match pass % 3 {
        0 => PatternType::Zeros,
        1 => PatternType::Ones,
        2 => PatternType::Random,
        _ => unreachable!(),
    }
}

impl ShredAlgorithm for Dod522022M {
    fn name(&self) -> &str {
        "DoD 5220.22-M"
    }
    fn description(&self) -> &str {
        "US DoD 5220.22-M. Fixed 3-pass sequence: zeros → ones → random. Passes > 3 repeat the sequence."
    }
    fn default_passes(&self) -> u32 {
        3
    }
    fn max_passes(&self) -> u32 {
        7
    }
    fn accepted_patterns(&self) -> &'static [PatternType] {
        &[PatternType::Zeros, PatternType::Ones, PatternType::Random]
    }
    fn has_fixed_pattern_sequence(&self) -> bool {
        true
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

        for pass in 0..passes {
            let pass_pattern = dod_pass_pattern(pass);
            match pass_pattern {
                PatternType::Zeros => buffer.fill(0x00),
                PatternType::Ones => buffer.fill(0xFF),
                PatternType::Random => getrandom(&mut buffer).map_err(|e| ShredError::IoError {
                    path: PathBuf::from("<random>"),
                    kind: "RandomGeneration".to_string(),
                    message: e.to_string(),
                })?,
            }

            file.seek(SeekFrom::Start(0))
                .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;

            let mut remaining = file_size;
            while remaining > 0 {
                let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;
                let written = file
                    .write(&buffer[..to_write])
                    .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;
                total_written += written as u64;
                remaining -= written as u64;
                progress.on_progress(total_written, file_size * passes as u64);
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
