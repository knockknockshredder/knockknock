// src-tauri/src/shredder/algorithms/dod_522022m.rs

use crate::shredder::algorithms::common::write_pass;
use crate::shredder::errors::ShredError;
use crate::shredder::traits::{ProgressReporter, ShredAlgorithm};
use crate::shredder::types::*;
use crate::shredder::verification::PrngSeed;
use std::fs::File;

pub struct Dod522022M;

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

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
        "US DoD 5220.22-M. Fixed 3-pass sequence: zeros, ones, random. Passes > 3 repeat the sequence."
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

    fn final_pattern(&self, _user_pattern: PatternType) -> PatternType {
        // DoD's final pass is always random (pass 2 in 0-indexed: zeros, ones, random)
        PatternType::Random
    }

    fn shred(
        &self,
        file: &mut File,
        file_size: u64,
        passes: u32,
        _pattern: PatternType,
        progress: &dyn ProgressReporter,
        seed: Option<&PrngSeed>,
    ) -> Result<ShredResult, ShredError> {
        let mut total_written = 0u64;
        let mut buffer = vec![0u8; BUFFER_SIZE];

        for pass in 0..passes {
            let pass_pattern = dod_pass_pattern(pass);

            total_written += write_pass(
                file,
                file_size,
                pass_pattern,
                &mut buffer,
                progress,
                total_written,
                file_size * passes as u64,
                seed,
            )?;
            // Note: do NOT emit on_pass_complete here. The pipeline in mod.rs
            // emits pass-complete after the algorithm returns to avoid double-emit.
        }

        Ok(ShredResult {
            success: true,
            passes_completed: passes,
            bytes_written: total_written,
            errors: vec![],
        })
    }
}
