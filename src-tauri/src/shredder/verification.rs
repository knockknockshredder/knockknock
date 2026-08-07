// src-tauri/src/shredder/verification.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::VerificationStrategy;
use crate::shredder::types::{PatternType, VerificationLevel, VerificationResult, WriteCheck};
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::ChaCha20;
use getrandom::getrandom;
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

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
        _path: &Path,
    ) -> Result<VerificationResult, ShredError> {
        Ok(VerificationResult { passed: true })
    }
}

/// Compare `buffer[..n]` against the expected bytes at absolute file offset
/// `pos`. For `Random` with a seed, regenerate via ChaCha20 with `try_seek`
/// (O(1)) so the buffer length doesn't matter.
///
/// Shared by `SampleVerification` (legacy) and `SpotVerification` (v2);
/// extracted verbatim from the original `SampleVerification::compare` — no
/// behavior change.
fn compare_block(
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
        seed: Option<&PrngSeed>,
        path: &Path,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult { passed: true });
        }

        let positions = [0u64, size / 2, size.saturating_sub(self.block_size as u64)];
        let mut buffer = vec![0u8; self.block_size];
        let mut mismatches = 0;

        for pos in &positions {
            file.seek(SeekFrom::Start(*pos))
                .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
            let n = file
                .read(&mut buffer)
                .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
            if n == 0 {
                continue;
            }

            if !compare_block(&buffer, n, *pos, pattern, seed) {
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
        path: &Path,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult { passed: true });
        }

        file.seek(SeekFrom::Start(0))
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

        let mut buffer = vec![0u8; 65536];
        let mut mismatches = 0;
        let mut remaining = size;
        let mut pos = 0u64;

        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            let n = file
                .read(&mut buffer[..to_read])
                .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
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

// ---------------------------------------------------------------------------
// v2 final-state write check (Phase 1, additive). The legacy
// `VerificationLevel` / `create_verifier` / `SampleVerification` stay live
// until the Phase 4 cutover.
// ---------------------------------------------------------------------------

/// Size in bytes of one spot-check read block.
pub(crate) const SPOT_BLOCK: u64 = 4096;

/// Number of evenly spaced interior blocks read for files above
/// `SMALL_FILE_LIMIT` (plus the first and last block).
pub(crate) const SPOT_INTERIOR: usize = 8;

/// Files at or below this size are spot-checked in full (one range).
pub(crate) const SMALL_FILE_LIMIT: u64 = 64 * 1024;

/// Deterministic `(pos, len)` read-back plan for the Spot write check.
///
/// `size <= SMALL_FILE_LIMIT` → the full range `[(0, size)]`. Larger files →
/// the first block, `SPOT_INTERIOR` evenly spaced interior blocks
/// (`(size * (i+1)) / (SPOT_INTERIOR + 1)`), and the last block. Positions
/// are deduplicated and clamped so `pos + len <= size` always holds; interior
/// positions colliding with the first or last block are skipped.
pub(crate) fn spot_check_plan(size: u64) -> Vec<(u64, u64)> {
    if size <= SMALL_FILE_LIMIT {
        return vec![(0, size)];
    }

    let last_pos = size - SPOT_BLOCK;
    let mut positions: Vec<u64> = Vec::with_capacity(SPOT_INTERIOR + 2);
    positions.push(0);
    for i in 0..SPOT_INTERIOR {
        let pos = size.saturating_mul(i as u64 + 1) / (SPOT_INTERIOR as u64 + 1);
        if pos != 0 && pos != last_pos {
            positions.push(pos);
        }
    }
    positions.push(last_pos);

    let mut seen: HashSet<u64> = HashSet::with_capacity(positions.len());
    let mut plan = Vec::with_capacity(positions.len());
    for pos in positions {
        if !seen.insert(pos) {
            continue;
        }
        let len = SPOT_BLOCK.min(size - pos);
        plan.push((pos, len));
    }
    plan
}

/// v2 Spot write checker: seeks to each `(pos, len)` in `spot_check_plan`
/// and compares the read-back bytes against the expected stream via
/// `compare_block` (same ChaCha20-with-seed / zeros / ones machinery as the
/// legacy sample verifier). An empty file (size 0) always passes.
pub(crate) struct SpotVerification {
    block_size: usize,
}

impl SpotVerification {
    pub(crate) fn new() -> Self {
        Self {
            block_size: SPOT_BLOCK as usize,
        }
    }
}

impl VerificationStrategy for SpotVerification {
    fn verify(
        &self,
        file: &mut File,
        pattern: &PatternType,
        size: u64,
        seed: Option<&PrngSeed>,
        path: &Path,
    ) -> Result<VerificationResult, ShredError> {
        if size == 0 {
            return Ok(VerificationResult { passed: true });
        }

        let mut buffer = vec![0u8; self.block_size];
        let mut mismatches = 0;

        for (pos, len) in spot_check_plan(size) {
            file.seek(SeekFrom::Start(pos))
                .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
            let n = file
                .read(&mut buffer[..len as usize])
                .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
            if n == 0 {
                continue;
            }

            if !compare_block(&buffer, n, pos, pattern, seed) {
                mismatches += 1;
            }
        }

        Ok(VerificationResult {
            passed: mismatches == 0,
        })
    }
}

/// Build the v2 final-state write checker for a `WriteCheck` mode:
/// `Off` → `NoVerification`, `Spot` → `SpotVerification`, `Full` →
/// `FullVerification`. (The legacy `create_verifier` remains for the legacy
/// pipeline until Phase 4.)
pub(crate) fn create_write_checker(write_check: WriteCheck) -> Box<dyn VerificationStrategy> {
    match write_check {
        WriteCheck::Off => Box::new(NoVerification),
        WriteCheck::Spot => Box::new(SpotVerification::new()),
        WriteCheck::Full => Box::new(FullVerification),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shredder::types::{PatternType, WriteCheck};
    use crate::shredder::verification::{
        create_write_checker, spot_check_plan, NoVerification, SpotVerification, SMALL_FILE_LIMIT,
        SPOT_BLOCK, SPOT_INTERIOR,
    };
    use chacha20::cipher::{StreamCipher, StreamCipherSeek};
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
    use tempfile::NamedTempFile;

    const MIB: u64 = 1024 * 1024;

    /// Generic plan invariants: bounds (pos + len <= size), no duplicate
    /// positions, full block lengths, and determinism across calls.
    fn assert_plan_invariants(size: u64) {
        let plan = spot_check_plan(size);
        let again = spot_check_plan(size);
        assert_eq!(plan, again, "plan must be deterministic for size {size}");

        let mut seen = std::collections::HashSet::new();
        for (pos, len) in &plan {
            assert!(pos + len <= size, "range {pos}+{len} exceeds size {size}");
            assert!(*len > 0, "empty range in plan for size {size}");
            assert!(
                seen.insert(*pos),
                "duplicate position {pos} in plan for size {size}"
            );
        }
    }

    #[test]
    fn spot_plan_small_files_are_checked_in_full() {
        for size in [1u64, 4095, 4096, SMALL_FILE_LIMIT] {
            assert_eq!(spot_check_plan(size), vec![(0, size)], "size {size}");
        }
    }

    #[test]
    fn spot_plan_large_files_have_first_interior_and_last_blocks() {
        let size = SMALL_FILE_LIMIT + 1; // 65537
        let plan = spot_check_plan(size);
        assert_eq!(plan.len(), SPOT_INTERIOR + 2);
        assert_eq!(plan[0], (0, SPOT_BLOCK));
        assert_eq!(plan[plan.len() - 1], (size - SPOT_BLOCK, SPOT_BLOCK));
        assert_plan_invariants(size);

        let plan = spot_check_plan(1 * MIB);
        assert_eq!(plan.len(), SPOT_INTERIOR + 2);
        assert_eq!(plan[0], (0, SPOT_BLOCK));
        assert_eq!(plan[plan.len() - 1], (1 * MIB - SPOT_BLOCK, SPOT_BLOCK));
        assert_plan_invariants(1 * MIB);

        // Interior positions must lie strictly between first and last block.
        for &(pos, len) in &plan[1..plan.len() - 1] {
            assert!(
                pos >= SPOT_BLOCK,
                "interior position {pos} inside first block"
            );
            assert!(
                pos + len <= 1 * MIB - SPOT_BLOCK,
                "interior position {pos} collides with last block"
            );
        }
    }

    #[test]
    fn spot_plan_invariants_hold_across_sizes() {
        for size in [65537u64, 100_000, 1 * MIB, 10 * MIB, 123_456_789] {
            assert_plan_invariants(size);
        }
    }

    /// Write `size` zero bytes, flip one byte at `flip_pos`, and run the
    /// given checker with the Zeros pattern.
    fn verify_zero_file_with_one_flip(
        checker: &dyn VerificationStrategy,
        size: u64,
        flip_pos: u64,
    ) -> bool {
        let temp = NamedTempFile::new().unwrap();
        temp.as_file().set_len(size).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path())
            .unwrap();
        file.seek(SeekFrom::Start(flip_pos)).unwrap();
        file.write_all(&[0x01]).unwrap();
        file.flush().unwrap();
        let result = checker
            .verify(&mut file, &PatternType::Zeros, size, None, temp.path())
            .unwrap();
        result.passed
    }

    /// Write `size` zero bytes and run the given checker with the Zeros
    /// pattern (expects a passing result).
    fn verify_zero_file(checker: &dyn VerificationStrategy, size: u64) -> bool {
        let temp = NamedTempFile::new().unwrap();
        temp.as_file().set_len(size).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path())
            .unwrap();
        let result = checker
            .verify(&mut file, &PatternType::Zeros, size, None, temp.path())
            .unwrap();
        result.passed
    }

    #[test]
    fn spot_verification_catches_wrong_byte_at_position_zero() {
        let checker = SpotVerification::new();
        // 256 KiB forces the distributed plan; position 0 is always checked.
        assert!(!verify_zero_file_with_one_flip(&checker, 256 * 1024, 0));
    }

    #[test]
    fn spot_verification_catches_wrong_byte_at_interior_position() {
        let checker = SpotVerification::new();
        let size = 256 * 1024;
        let plan = spot_check_plan(size);
        assert!(plan.len() > 2, "expected a distributed plan");
        let (interior_pos, _) = plan[1];
        assert!(interior_pos > 0 && interior_pos + SPOT_BLOCK < size);
        assert!(!verify_zero_file_with_one_flip(
            &checker,
            size,
            interior_pos
        ));
    }

    #[test]
    fn spot_verification_passes_on_correct_stream() {
        let seed = PrngSeed {
            key: [7u8; 32],
            nonce: [3u8; 12],
        };
        let temp = NamedTempFile::new().unwrap();
        let mut expected = vec![0u8; 256 * 1024];
        let mut cipher = seed.cipher();
        cipher.apply_keystream(&mut expected);
        temp.as_file().set_len(256 * 1024).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path())
            .unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&expected).unwrap();
        file.flush().unwrap();

        let checker = SpotVerification::new();
        let result = checker
            .verify(
                &mut file,
                &PatternType::Random,
                256 * 1024,
                Some(&seed),
                temp.path(),
            )
            .unwrap();
        assert!(result.passed);
    }

    #[test]
    fn full_verification_catches_mid_range_corruption() {
        let checker = FullVerification;
        // 100_000 lies mid-range: not the first or last read block.
        assert!(!verify_zero_file_with_one_flip(
            &checker,
            256 * 1024,
            100_000
        ));
    }

    #[test]
    fn spot_verification_passes_on_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path())
            .unwrap();
        let checker = SpotVerification::new();
        let result = checker
            .verify(&mut file, &PatternType::Random, 0, None, temp.path())
            .unwrap();
        assert!(result.passed);
    }

    #[test]
    fn no_verification_verifies_true_on_write_only_handle() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 4096]).unwrap();
        temp.flush().unwrap();

        // Write-only handle: any read would fail, so a passing result proves
        // no read occurred.
        let mut file = OpenOptions::new().write(true).open(temp.path()).unwrap();
        let result = NoVerification
            .verify(&mut file, &PatternType::Zeros, 4096, None, temp.path())
            .unwrap();
        assert!(result.passed);

        // Sanity: the same write-only handle cannot actually be read back �
        // proving the Off mode genuinely skipped reading.
        let mut file = OpenOptions::new().write(true).open(temp.path()).unwrap();
        let result =
            SpotVerification::new().verify(&mut file, &PatternType::Zeros, 4096, None, temp.path());
        assert!(result.is_err(), "write-only handle must not be readable");
    }

    #[test]
    fn create_write_checker_maps_modes() {
        // Off: passes without reading (write-only handle).
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 4096]).unwrap();
        temp.flush().unwrap();
        let mut file = OpenOptions::new().write(true).open(temp.path()).unwrap();
        let result = create_write_checker(WriteCheck::Off)
            .verify(&mut file, &PatternType::Zeros, 4096, None, temp.path())
            .unwrap();
        assert!(result.passed);

        // Spot and Full: read-backed checkers that pass on a correct stream
        // and reject a corrupted one. The corrupted byte must sit inside a
        // block the mode actually reads: an interior plan block for Spot
        // (mid-range would be skipped by the distributed plan), any
        // mid-range offset for Full.
        for mode in [WriteCheck::Spot, WriteCheck::Full] {
            let checker = create_write_checker(mode);
            assert!(verify_zero_file(checker.as_ref(), 256 * 1024));
            let flip_pos = match mode {
                WriteCheck::Spot => spot_check_plan(256 * 1024)[1].0,
                WriteCheck::Full => 100_000,
                WriteCheck::Off => unreachable!(),
            };
            assert!(!verify_zero_file_with_one_flip(
                checker.as_ref(),
                256 * 1024,
                flip_pos
            ));
        }
    }
}
