// src-tauri/src/shredder/verification.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::VerificationStrategy;
use crate::shredder::types::{PatternType, VerificationLevel, VerificationResult};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub struct NoVerification;

impl VerificationStrategy for NoVerification {
    fn verify(
        &self,
        _file: &mut File,
        _pattern: &PatternType,
        _size: u64,
    ) -> Result<VerificationResult, ShredError> {
        Ok(VerificationResult {
            passed: true,
            blocks_checked: 0,
            mismatches: 0,
        })
    }
}

pub struct SampleVerification {
    block_size: usize,
}

impl SampleVerification {
    pub fn new() -> Self {
        Self { block_size: 4096 }
    }
}

impl VerificationStrategy for SampleVerification {
    fn verify(
        &self,
        file: &mut File,
        pattern: &PatternType,
        size: u64,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult {
                passed: true,
                blocks_checked: 0,
                mismatches: 0,
            });
        }

        let positions = [0u64, size / 2, size.saturating_sub(self.block_size as u64)];
        let mut buffer = vec![0u8; self.block_size];
        let mut mismatches = 0;

        for pos in &positions {
            file.seek(SeekFrom::Start(*pos))
                .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;
            let n = file
                .read(&mut buffer)
                .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;
            if n == 0 {
                continue;
            }

            match pattern {
                PatternType::Random => {
                    if buffer[..n].iter().all(|&b| b == 0) || buffer[..n].iter().all(|&b| b == 0xFF)
                    {
                        mismatches += 1;
                    }
                }
                PatternType::Zeros => {
                    if buffer[..n].iter().any(|&b| b != 0) {
                        mismatches += 1;
                    }
                }
                PatternType::Ones => {
                    if buffer[..n].iter().any(|&b| b != 0xFF) {
                        mismatches += 1;
                    }
                }
            }
        }

        Ok(VerificationResult {
            passed: mismatches == 0,
            blocks_checked: 3,
            mismatches,
        })
    }
}

pub struct FullVerification;

impl VerificationStrategy for FullVerification {
    fn verify(
        &self,
        file: &mut File,
        pattern: &PatternType,
        size: u64,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult {
                passed: true,
                blocks_checked: 0,
                mismatches: 0,
            });
        }

        file.seek(SeekFrom::Start(0))
            .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;

        let mut buffer = vec![0u8; 65536];
        let mut mismatches = 0;
        let mut remaining = size;

        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            let n = file
                .read(&mut buffer[..to_read])
                .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;
            if n == 0 {
                break;
            }

            match pattern {
                PatternType::Random => {
                    if buffer[..n].iter().all(|&b| b == 0) || buffer[..n].iter().all(|&b| b == 0xFF)
                    {
                        mismatches += 1;
                    }
                }
                PatternType::Zeros => {
                    if buffer[..n].iter().any(|&b| b != 0) {
                        mismatches += 1;
                    }
                }
                PatternType::Ones => {
                    if buffer[..n].iter().any(|&b| b != 0xFF) {
                        mismatches += 1;
                    }
                }
            }

            remaining -= n as u64;
        }

        Ok(VerificationResult {
            passed: mismatches == 0,
            blocks_checked: (size / 65536 + 1) as usize,
            mismatches,
        })
    }
}

pub fn create_verifier(level: VerificationLevel) -> Box<dyn VerificationStrategy> {
    match level {
        VerificationLevel::None => Box::new(NoVerification),
        VerificationLevel::Sample => Box::new(SampleVerification::new()),
        VerificationLevel::Full => Box::new(FullVerification),
    }
}
