// src-tauri/src/shredder/verification.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::VerificationStrategy;
use crate::shredder::types::{PatternType, VerificationLevel, VerificationResult};
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::ChaCha20;
use getrandom::getrandom;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Deterministic seed for ChaCha20-based PRNG used to write and verify random data.
///
/// `key` is the 32-byte ChaCha20 key; `nonce` is the 12-byte ChaCha20 nonce.
/// Generating the seed once per file lets the writer and the verifier reproduce
/// the exact same byte stream — verification becomes a deterministic byte
/// comparison instead of a statistical guess.
#[derive(Debug, Clone)]
pub struct PrngSeed {
    pub key: [u8; 32],
    pub nonce: [u8; 12],
}

impl PrngSeed {
    /// Generate a fresh seed using the OS CSPRNG (`getrandom`).
    pub fn generate() -> Result<Self, ShredError> {
        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        getrandom(&mut key).map_err(|e| ShredError::IoError {
            path: std::path::PathBuf::from("<random>"),
            kind: "RandomGeneration".to_string(),
            message: e.to_string(),
        })?;
        getrandom(&mut nonce).map_err(|e| ShredError::IoError {
            path: std::path::PathBuf::from("<random>"),
            kind: "RandomGeneration".to_string(),
            message: e.to_string(),
        })?;
        Ok(Self { key, nonce })
    }

    /// Build a fresh ChaCha20 cipher positioned at the keystream start.
    pub fn cipher(&self) -> ChaCha20 {
        ChaCha20::new(&self.key.into(), &self.nonce.into())
    }
}

pub struct NoVerification;

impl VerificationStrategy for NoVerification {
    fn verify(
        &self,
        _file: &mut File,
        _pattern: &PatternType,
        _size: u64,
        _seed: Option<&PrngSeed>,
    ) -> Result<VerificationResult, ShredError> {
        Ok(VerificationResult { passed: true })
    }
}

pub struct SampleVerification {
    block_size: usize,
}

impl SampleVerification {
    pub fn new() -> Self {
        Self { block_size: 4096 }
    }

    /// Compare `buffer[..n]` against the expected bytes at absolute file
    /// offset `pos`. For `Random` with a seed, regenerate via ChaCha20 with
    /// `try_seek` (O(1)) so the buffer length doesn't matter.
    fn compare(
        buffer: &[u8],
        n: usize,
        pos: u64,
        pattern: &PatternType,
        seed: Option<&PrngSeed>,
    ) -> bool {
        let slice = &buffer[..n];
        match pattern {
            PatternType::Zeros => slice.iter().all(|&b| b == 0),
            PatternType::Ones => slice.iter().all(|&b| b == 0xFF),
            PatternType::Random => match seed {
                Some(seed) => {
                    let mut cipher = seed.cipher();
                    if cipher.try_seek(pos).is_err() {
                        return false;
                    }
                    let mut expected = vec![0u8; n];
                    cipher.apply_keystream(&mut expected);
                    expected == slice
                }
                None => {
                    // Fallback heuristic when no seed is available — same as the
                    // original (broken) behavior. Better than a false pass.
                    !(slice.iter().all(|&b| b == 0) || slice.iter().all(|&b| b == 0xFF))
                }
            },
        }
    }
}

impl VerificationStrategy for SampleVerification {
    fn verify(
        &self,
        file: &mut File,
        pattern: &PatternType,
        size: u64,
        seed: Option<&PrngSeed>,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult { passed: true });
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

            if !Self::compare(&buffer, n, *pos, pattern, seed) {
                mismatches += 1;
            }
        }

        Ok(VerificationResult {
            passed: mismatches == 0,
        })
    }
}

pub struct FullVerification;

impl FullVerification {
    /// Compare `buffer[..n]` against the expected bytes at absolute file
    /// offset `pos` and return `true` if any byte mismatches. For
    /// deterministic patterns (Zeros/Ones) we compare inline; for
    /// Random-with-seed we regenerate the expected keystream via ChaCha20
    /// with `try_seek` (O(1) per jump, no skip buffers). The name was
    /// previously `fill_expected`, which was misleading because nothing is
    /// ever filled — the function only detects mismatches.
    fn check_block_mismatch(
        buffer: &mut [u8],
        n: usize,
        pos: u64,
        pattern: &PatternType,
        seed: Option<&PrngSeed>,
    ) -> bool {
        let slice = &mut buffer[..n];
        match pattern {
            PatternType::Zeros => {
                // Compare inline: any nonzero byte = mismatch
                slice.iter().any(|&b| b != 0)
            }
            PatternType::Ones => slice.iter().any(|&b| b != 0xFF),
            PatternType::Random => match seed {
                Some(seed) => {
                    let mut cipher = seed.cipher();
                    if cipher.try_seek(pos).is_err() {
                        return true;
                    }
                    let mut expected = vec![0u8; n];
                    cipher.apply_keystream(&mut expected);
                    expected != *slice
                }
                None => slice.iter().all(|&b| b == 0) || slice.iter().all(|&b| b == 0xFF),
            },
        }
    }
}

impl VerificationStrategy for FullVerification {
    fn verify(
        &self,
        file: &mut File,
        pattern: &PatternType,
        size: u64,
        seed: Option<&PrngSeed>,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult { passed: true });
        }

        file.seek(SeekFrom::Start(0))
            .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;

        let mut buffer = vec![0u8; 65536];
        let mut mismatches = 0;
        let mut remaining = size;
        let mut pos = 0u64;

        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            let n = file
                .read(&mut buffer[..to_read])
                .map_err(|e| ShredError::from_io_error(std::path::PathBuf::from("<file>"), e))?;
            if n == 0 {
                break;
            }

            if Self::check_block_mismatch(&mut buffer, n, pos, pattern, seed) {
                mismatches += 1;
            }

            pos += n as u64;
            remaining -= n as u64;
        }

        Ok(VerificationResult {
            passed: mismatches == 0,
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
