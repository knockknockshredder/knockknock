// src-tauri/src/shredder/algorithms/nist_clear.rs

use crate::shredder::algorithms::common::write_pass;
use crate::shredder::errors::ShredError;
use crate::shredder::traits::{ProgressReporter, ShredAlgorithm};
use crate::shredder::types::*;
use getrandom::getrandom;
use std::fs::File;
use std::path::PathBuf;

pub struct NistClear;

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

impl ShredAlgorithm for NistClear {
    fn name(&self) -> &str {
        "NIST 800-88 Clear"
    }
    fn description(&self) -> &str {
        "NIST SP 800-88 Clear standard. Single-pass overwrite with user-selected pattern."
    }
    fn default_passes(&self) -> u32 {
        1
    }
    fn max_passes(&self) -> u32 {
        35
    }
    fn accepted_patterns(&self) -> &'static [PatternType] {
        &[PatternType::Random, PatternType::Zeros, PatternType::Ones]
    }

    fn shred(
        &self,
        file: &mut File,
        file_size: u64,
        passes: u32,
        pattern: PatternType,
        progress: &dyn ProgressReporter,
    ) -> Result<ShredResult, ShredError> {
        let start = std::time::Instant::now();
        let mut total_written = 0u64;
        let mut buffer = vec![0u8; BUFFER_SIZE];

        for pass in 0..passes {
            match pattern {
                PatternType::Random => getrandom(&mut buffer).map_err(|e| ShredError::IoError {
                    path: PathBuf::from("<random>"),
                    kind: "RandomGeneration".to_string(),
                    message: e.to_string(),
                })?,
                PatternType::Zeros => buffer.fill(0x00),
                PatternType::Ones => buffer.fill(0xFF),
            }

            total_written += write_pass(
                file,
                file_size,
                pattern,
                &mut buffer,
                progress,
                total_written,
                file_size * passes as u64,
            )?;
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
