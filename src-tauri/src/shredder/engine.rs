// src-tauri/src/shredder/engine.rs
//
// v2 policy-driven overwrite engine (Phase 1, additive). Since Phase 4 the
// engine is self-contained: `write_pass` / `fill_pattern_buffer` were moved
// here from the deleted legacy `algorithms::common` (ora-2 amendment 4),
// reusing `verification::PrngSeed` for the deterministic random streams.

use crate::shredder::errors::ShredError;
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::{DeletionMethod, DeletionPolicy, PatternType, WriteCheckOutcome};
use crate::shredder::verification::{create_write_checker, PrngSeed};
use chacha20::cipher::{StreamCipher, StreamCipherSeek};
use chacha20::ChaCha20;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

/// 1 MiB overwrite buffer, matching the legacy algorithms' buffer size.
/// Chunked inside `write_pass` — never allocate the file size in memory.
/// The buffer is intentionally NOT zeroized after use (S1).
const BUFFER_SIZE: usize = 1024 * 1024;

/// Progress is reported at most once per 1 MiB of written bytes per pass
/// (moved verbatim from the legacy `algorithms::common`).
const PROGRESS_INTERVAL: u64 = 1024 * 1024;

/// Write data to file with progress reporting.
///
/// For `PatternType::Random`, the buffer is filled from `ChaCha20(seed)` when a
/// seed is supplied (deterministic, verifiable) or from the OS CSPRNG
/// (`getrandom`) when no seed is available. For fixed patterns (Zeros, Ones)
/// the buffer is used as-is.
// Legacy-verbatim signature (moved from algorithms::common in Phase 4);
// bundling the parameters would churn the engine's only call site for no gain.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_pass(
    file: &mut File,
    file_size: u64,
    pattern: PatternType,
    buffer: &mut [u8],
    progress: &dyn ProgressReporter,
    bytes_written_so_far: u64,
    total_bytes: u64,
    seed: Option<&PrngSeed>,
    path: &std::path::Path,
) -> Result<u64, ShredError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

    let mut written = 0u64;
    let mut remaining = file_size;
    let mut last_progress = 0u64;
    let mut cipher = seed.map(|s| s.cipher());

    while remaining > 0 {
        // Check cancellation between chunks (chunk size = buffer.len or remaining)
        if crate::shredder::cancel::is_cancelled_global() {
            return Err(ShredError::IoError {
                path: path.to_path_buf(),
                kind: "Cancelled".to_string(),
                message: "Shredding cancelled during write".to_string(),
            });
        }

        let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;

        // Fill buffer based on pattern at absolute file position `written`.
        // For Random-with-seed this seeks the cipher to `written` (O(1)) and
        // runs the keystream into the buffer.
        fill_pattern_buffer(buffer, pattern, &mut cipher, written, path)?;

        file.write_all(&buffer[..to_write])
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
        written += to_write as u64;
        remaining -= to_write as u64;

        if written - last_progress >= PROGRESS_INTERVAL {
            progress.on_progress(bytes_written_so_far + written, total_bytes);
            last_progress = written;
        }
    }

    Ok(written)
}

/// Fill `buffer` with the configured pattern at keystream offset `pos`.
///
/// `pos` is the absolute byte offset within the file (and thus within the
/// ChaCha20 keystream). For Random-with-seed we use `try_seek` to jump
/// directly to that offset in O(1), avoiding skip buffers.
fn fill_pattern_buffer(
    buffer: &mut [u8],
    pattern: PatternType,
    cipher: &mut Option<ChaCha20>,
    pos: u64,
    path: &std::path::Path,
) -> Result<(), ShredError> {
    match pattern {
        PatternType::Zeros => buffer.fill(0x00),
        PatternType::Ones => buffer.fill(0xFF),
        PatternType::Random => {
            if let Some(cipher_ref) = cipher.as_mut() {
                cipher_ref.try_seek(pos).map_err(|e| ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "CipherSeek".to_string(),
                    message: e.to_string(),
                })?;
                buffer.fill(0u8); // Zero buffer before XOR-ing keystream
                cipher_ref.apply_keystream(buffer);
            } else {
                getrandom::getrandom(buffer).map_err(|e| ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "RandomGeneration".to_string(),
                    message: e.to_string(),
                })?;
            }
        }
    }
    Ok(())
}

/// v2 overwrite lifecycle state for one file (M2).
///
/// `NotStarted` is never returned in an `Ok` outcome: a failure before any
/// byte was written is returned as `Err`, and the caller must treat `Err`
/// as `NotStarted` (target intact — no cleanup). `Partial` means the
/// overwrite did not complete with at least one byte written — or was
/// cancelled with none written, since the cancel flag may have been
/// observed mid-chunk and the engine errs toward cleanup (recorded
/// deviation) — the caller MUST run best-effort cleanup. `Completed` means
/// every planned pass ran;
/// a failed final write check is still `Completed` with
/// `write_check: Failed` and a `WriteCheckFailed` issue (M2 rule 3) — the
/// file's removal continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverwriteState {
    NotStarted,
    Partial,
    Completed,
}

/// Structured outcome of one file's overwrite (v2, M2/M3).
#[derive(Debug)]
pub(crate) struct OverwriteOutcome {
    pub state: OverwriteState,
    pub bytes_written: u64,
    pub passes_completed: u32,
    pub write_check: WriteCheckOutcome,
    pub issues: Vec<ShredError>,
}

/// v2 pass sequence per method — tested directly; no production test hooks
/// elsewhere (S2). Automatic → one random pass; LegacyThreePass → the fixed
/// zeros → ones → random sequence.
pub(crate) fn pass_plan(method: DeletionMethod) -> Vec<PatternType> {
    match method {
        DeletionMethod::Automatic => vec![PatternType::Random],
        DeletionMethod::LegacyThreePass => {
            vec![PatternType::Zeros, PatternType::Ones, PatternType::Random]
        }
    }
}

/// One file's full destructive write lifecycle: passes → sync → final write
/// check. Generates a fresh `PrngSeed` and delegates to
/// `overwrite_file_with_seed`.
///
/// # Error contract (M2)
/// - `Err` means "no chunk completed": no byte of any pass reached the file
///   (state would be `NotStarted`; target intact — the caller must NOT
///   journal/rename). A non-cancel `write_pass` failure is classified by
///   recovering the handle cursor (`write_pass` seeks to 0 and writes
///   sequentially, so the cursor is the interrupted pass's byte count):
///   cursor 0 with no completed pass → `Err`; anything else → `Ok(Partial)`.
/// - `Ok(Partial)` with the issue preserved in `issues`: at least one byte
///   was written — the caller MUST run best-effort cleanup.
/// - `Ok(Completed)` — every pass ran; `write_check` is `Passed`, `NotRun`
///   (Off or zero-length), or `Failed` (a `WriteCheckFailed` issue is
///   pushed; removal still continues).
///
/// # Cancellation
/// `write_pass` keeps its legacy per-chunk global cancel check. A mid-pass
/// stop surfaces as a `Cancelled` `write_pass` error and yields a `Partial`
/// outcome (even when no completed pass exists and no byte was written,
/// because the flag may have been observed mid-chunk — the engine errs
/// toward cleanup): the caller runs best-effort cleanup for the current
/// file, and the batch layer stops at the next boundary (stop-after-current-
/// file). Any interrupted pass — cancelled or errored — skips the final
/// `sync_all` and write check entirely (`write_check: NotRun`).
///
/// # Progress (M5)
/// `on_pass_start(pass, total)` / `on_pass_complete(pass, total)` with
/// `total = policy.total_passes()`; `on_progress` is pass-local
/// (`bytes_written_so_far = 0`, `total_bytes = file_size` per pass), so the
/// frontend can combine pass + pass-local percent without exceeding 100%.
/// A zero-length file emits NO pass events and returns the vacuous
/// `Completed` outcome (0 bytes, 0 passes, `NotRun`).
pub(crate) fn overwrite_file(
    file: &mut File,
    file_size: u64,
    policy: DeletionPolicy,
    progress: &dyn ProgressReporter,
    path: &Path,
) -> Result<OverwriteOutcome, ShredError> {
    // One fresh seed per file; the (single) random pass of either method
    // uses it, and the final write check reproduces the stream from it.
    let seed = PrngSeed::generate()?;
    overwrite_file_with_seed(file, file_size, policy, progress, path, seed)
}

/// Overwrite core with the seed supplied instead of generated, so tests can
/// prove the final bytes match the expected stream (spec §34/§35). This is
/// the parameterized implementation `overwrite_file` is built on — not a
/// test-only hook: it is `pub(crate)` and never crosses IPC, and the seed it
/// takes is always a fresh `PrngSeed` in production. For `Automatic` the
/// seed is the single random pass's seed; for `LegacyThreePass` it is the
/// pass-3 (final) seed; the final write check always uses this seed (the
/// zeros/ones passes are seedless).
pub(crate) fn overwrite_file_with_seed(
    file: &mut File,
    file_size: u64,
    policy: DeletionPolicy,
    progress: &dyn ProgressReporter,
    path: &Path,
    seed: PrngSeed,
) -> Result<OverwriteOutcome, ShredError> {
    let total_passes = policy.total_passes();
    if file_size == 0 {
        // Vacuous overwrite: nothing to seek/write/check. The caller
        // proceeds straight to journal → rename → unlink (M4 lifecycle).
        return Ok(OverwriteOutcome {
            state: OverwriteState::Completed,
            bytes_written: 0,
            passes_completed: 0,
            write_check: WriteCheckOutcome::NotRun,
            issues: Vec::new(),
        });
    }

    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut bytes_written_total = 0u64;
    let mut passes_completed = 0u32;
    let mut issues: Vec<ShredError> = Vec::new();
    let mut final_seed: Option<PrngSeed> = None;
    let plan = pass_plan(policy.method);

    for (idx, pattern) in plan.iter().enumerate() {
        let pass = (idx as u32) + 1;
        // One seed per random pass; the supplied seed is the last random
        // pass's seed and is reused by the final write check.
        let pass_seed = if *pattern == PatternType::Random {
            final_seed = Some(seed.clone());
            Some(&seed)
        } else {
            None
        };

        progress.on_pass_start(pass, total_passes);
        let written = write_pass(
            file,
            file_size,
            *pattern,
            &mut buffer,
            progress,
            0, // pass-local progress (M5): byte counter resets each pass
            file_size,
            pass_seed,
            path,
        );
        match written {
            Ok(bytes_in_pass) => {
                bytes_written_total += bytes_in_pass;
                passes_completed = pass;
                progress.on_pass_complete(pass, total_passes);
            }
            Err(error) => {
                let cancelled = matches!(
                    error,
                    ShredError::IoError { ref kind, .. } if kind == "Cancelled"
                );
                // `write_pass` reports no partial byte count on error, so
                // recover how much of the interrupted pass reached the file
                // from the handle cursor: `write_pass` seeks to 0 at the
                // start of every pass and writes sequentially, so the cursor
                // is the pass's written byte count. If the cursor cannot be
                // read, fail safe toward Partial — claiming the target
                // intact without proof would violate M2.
                let pass_bytes = file
                    .stream_position()
                    .map(|pos| pos.min(file_size))
                    .unwrap_or(file_size);
                if !cancelled && bytes_written_total == 0 && pass_bytes == 0 {
                    // No byte of any pass reached the file: target intact.
                    // `Err` means "no chunk completed" (NotStarted).
                    return Err(error);
                }
                // `Cancelled` always yields Partial (the flag may have been
                // observed mid-chunk; erring toward cleanup per the
                // destructive-lifecycle contract — recorded deviation).
                // Non-cancel errors yield Partial once any byte was written.
                // The final sync/write check must NEVER run after an
                // interrupted pass: the caller decides cleanup from
                // `state == Partial` (M2).
                issues.push(error);
                return Ok(OverwriteOutcome {
                    state: OverwriteState::Partial,
                    bytes_written: bytes_written_total + pass_bytes,
                    passes_completed,
                    write_check: WriteCheckOutcome::NotRun,
                    issues,
                });
            }
        }
    }

    if let Err(error) = file.sync_all() {
        let sync_error = ShredError::from_io_error(path.to_path_buf(), error);
        if bytes_written_total == 0 {
            return Err(sync_error);
        }
        issues.push(sync_error);
        return Ok(OverwriteOutcome {
            state: OverwriteState::Partial,
            bytes_written: bytes_written_total,
            passes_completed,
            write_check: WriteCheckOutcome::NotRun,
            issues,
        });
    }

    // Final-state write check against the last pass's stream. Both v2
    // methods end on Random, so the final pattern is the last plan entry.
    let final_pattern = *plan.last().unwrap();
    let check = create_write_checker(policy.write_check).verify(
        file,
        &final_pattern,
        file_size,
        final_seed.as_ref(),
        path,
    );
    let write_check = match check {
        Ok(result) if result.passed => WriteCheckOutcome::Passed,
        // Mismatch or read error: outcome stays Ok(Completed); the failure
        // is surfaced via write_check + a structured issue (M2 rule 3).
        Ok(_) | Err(_) => {
            issues.push(ShredError::WriteCheckFailed {
                path: path.to_path_buf(),
            });
            WriteCheckOutcome::Failed
        }
    };

    Ok(OverwriteOutcome {
        state: OverwriteState::Completed,
        bytes_written: bytes_written_total,
        passes_completed,
        write_check,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shredder::progress::NoopProgressReporter;
    use crate::shredder::types::{DeletionMethod, DeletionPolicy, WriteCheck};
    use crate::shredder::verification::PrngSeed;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;

    fn automatic_policy(write_check: WriteCheck) -> DeletionPolicy {
        DeletionPolicy {
            method: DeletionMethod::Automatic,
            write_check,
        }
    }

    fn legacy_policy(write_check: WriteCheck) -> DeletionPolicy {
        DeletionPolicy {
            method: DeletionMethod::LegacyThreePass,
            write_check,
        }
    }

    /// Keystream a seed produces for `len` bytes at offset 0.
    fn expected_stream(seed: &PrngSeed, len: usize) -> Vec<u8> {
        let mut expected = vec![0u8; len];
        let mut cipher = seed.cipher();
        cipher.apply_keystream(&mut expected);
        expected
    }

    fn read_all(mut file: &File) -> Vec<u8> {
        let mut contents = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut contents).unwrap();
        contents
    }

    /// Recording progress reporter: captures pass events and progress calls.
    /// With `cancel_on_first_progress` set, the first `on_progress` call
    /// additionally fires the global cancel flag — the flag takes effect
    /// mid-pass because `write_pass` reports after each 1 MiB chunk.
    #[derive(Default)]
    struct RecordingProgress {
        pass_starts: Mutex<Vec<(u32, u32)>>,
        pass_completes: Mutex<Vec<(u32, u32)>>,
        progress_events: Mutex<Vec<(u64, u64)>>,
        cancel_on_first_progress: AtomicBool,
    }

    impl ProgressReporter for RecordingProgress {
        fn on_file_start(&self, _path: &Path, _file_size: u64) {}
        fn on_pass_start(&self, pass: u32, total_passes: u32) {
            self.pass_starts.lock().unwrap().push((pass, total_passes));
        }
        fn on_progress(&self, bytes_written: u64, total: u64) {
            if self.cancel_on_first_progress.swap(false, Ordering::Relaxed) {
                crate::shredder::cancel::cancel_global();
            }
            self.progress_events
                .lock()
                .unwrap()
                .push((bytes_written, total));
        }
        fn on_pass_complete(&self, pass: u32, total_passes: u32) {
            self.pass_completes
                .lock()
                .unwrap()
                .push((pass, total_passes));
        }
        fn on_file_complete(
            &self,
            _path: &Path,
            _result: &crate::shredder::types::ShredResult,
            _total_passes: u32,
        ) {
        }
        fn on_error(&self, _path: &Path, _error: &ShredError) {}
        fn on_warning(&self, _path: &Path, _message: &str) {}
    }

    #[test]
    fn pass_plan_automatic_is_single_random() {
        assert_eq!(
            pass_plan(DeletionMethod::Automatic),
            vec![PatternType::Random]
        );
    }

    #[test]
    fn pass_plan_legacy_is_zeros_ones_random() {
        assert_eq!(
            pass_plan(DeletionMethod::LegacyThreePass),
            vec![PatternType::Zeros, PatternType::Ones, PatternType::Random]
        );
    }

    #[test]
    fn automatic_seeded_overwrite_matches_expected_stream() {
        let _serial = engine_test_lock();
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 256 * KIB as usize]).unwrap();
        temp.flush().unwrap();

        let seed = PrngSeed {
            key: [1u8; 32],
            nonce: [2u8; 12],
        };
        let mut file = temp.reopen().unwrap();
        let progress = NoopProgressReporter;
        let outcome = overwrite_file_with_seed(
            &mut file,
            256 * KIB,
            automatic_policy(WriteCheck::Full),
            &progress,
            temp.path(),
            seed.clone(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Completed);
        assert_eq!(outcome.passes_completed, 1);
        assert_eq!(outcome.bytes_written, 256 * KIB);
        assert_eq!(outcome.write_check, WriteCheckOutcome::Passed);
        assert!(outcome.issues.is_empty());

        assert_eq!(read_all(&file), expected_stream(&seed, 256 * KIB as usize));
    }

    #[test]
    fn legacy_seeded_overwrite_matches_pass3_stream() {
        let _serial = engine_test_lock();
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 256 * KIB as usize]).unwrap();
        temp.flush().unwrap();

        let seed = PrngSeed {
            key: [9u8; 32],
            nonce: [4u8; 12],
        };
        let mut file = temp.reopen().unwrap();
        let progress = NoopProgressReporter;
        let outcome = overwrite_file_with_seed(
            &mut file,
            256 * KIB,
            legacy_policy(WriteCheck::Full),
            &progress,
            temp.path(),
            seed.clone(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Completed);
        assert_eq!(outcome.passes_completed, 3);
        assert_eq!(outcome.bytes_written, 3 * 256 * KIB);
        assert_eq!(outcome.write_check, WriteCheckOutcome::Passed);
        assert!(outcome.issues.is_empty());

        // The final content must equal the pass-3 (seeded random) stream.
        assert_eq!(read_all(&file), expected_stream(&seed, 256 * KIB as usize));
        // Intermediate pass states (pass 1 = zeros, pass 2 = ones) are not
        // observable without production test hooks (S2); they are covered by
        // the exact pass_plan sequence above plus direct `write_pass` unit
        // coverage (zeros/ones/seeded-random — see
        // `write_pass_writes_fixed_patterns_and_seeded_random`), with the
        // final-stream assertion proving the full sequence ran in order.
    }

    #[test]
    fn write_pass_writes_fixed_patterns_and_seeded_random() {
        let _serial = engine_test_lock();
        let seed = PrngSeed {
            key: [5u8; 32],
            nonce: [6u8; 12],
        };
        let size = 64 * KIB;
        for (pattern, expected) in [
            (PatternType::Zeros, vec![0x00u8; size as usize]),
            (PatternType::Ones, vec![0xFFu8; size as usize]),
            (PatternType::Random, expected_stream(&seed, size as usize)),
        ] {
            let mut temp = NamedTempFile::new().unwrap();
            temp.write_all(&vec![0xAAu8; size as usize]).unwrap();
            temp.flush().unwrap();
            let mut file = temp.reopen().unwrap();
            let progress = NoopProgressReporter;
            let mut buffer = vec![0u8; BUFFER_SIZE];
            let seed_ref = (pattern == PatternType::Random).then_some(&seed);
            let written = write_pass(
                &mut file,
                size,
                pattern,
                &mut buffer,
                &progress,
                0,
                size,
                seed_ref,
                temp.path(),
            )
            .unwrap();
            assert_eq!(written, size);
            assert_eq!(read_all(&file), expected, "pattern {pattern:?}");
        }
    }

    #[test]
    fn zero_length_file_returns_vacuous_completed() {
        let _serial = engine_test_lock();
        let temp = NamedTempFile::new().unwrap();
        let mut file = temp.reopen().unwrap();
        let progress = NoopProgressReporter;
        let outcome = overwrite_file(
            &mut file,
            0,
            legacy_policy(WriteCheck::Full),
            &progress,
            temp.path(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Completed);
        assert_eq!(outcome.bytes_written, 0);
        assert_eq!(outcome.passes_completed, 0);
        assert_eq!(outcome.write_check, WriteCheckOutcome::NotRun);
        assert!(outcome.issues.is_empty());
        assert_eq!(file.metadata().unwrap().len(), 0, "file must be untouched");
    }

    #[test]
    fn write_check_read_error_yields_completed_with_failed() {
        let _serial = engine_test_lock();
        // A write-only handle accepts the overwrite but cannot be read back:
        // the final check fails with a read error, which must surface as an
        // Ok(Completed) outcome with write_check Failed + a WriteCheckFailed
        // issue (M2 rule 3 — only pre-destructive failures are Err).
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 256 * KIB as usize]).unwrap();
        temp.flush().unwrap();

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(temp.path())
            .unwrap();
        let progress = NoopProgressReporter;
        let outcome = overwrite_file(
            &mut file,
            256 * KIB,
            automatic_policy(WriteCheck::Full),
            &progress,
            temp.path(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Completed);
        assert_eq!(outcome.write_check, WriteCheckOutcome::Failed);
        assert!(
            outcome
                .issues
                .iter()
                .any(|e| matches!(e, ShredError::WriteCheckFailed { .. })),
            "expected a WriteCheckFailed issue, got {:?}",
            outcome.issues
        );
    }

    #[test]
    fn overwrite_on_read_only_handle_errors_and_leaves_file_intact() {
        let _serial = engine_test_lock();
        let mut temp = NamedTempFile::new().unwrap();
        let original = vec![0xAAu8; 256 * KIB as usize];
        temp.write_all(&original).unwrap();
        temp.flush().unwrap();

        // Read-only handle: the first write must fail before any byte is
        // written → Err (NotStarted), file untouched.
        let mut file = std::fs::File::open(temp.path()).unwrap();
        let progress = NoopProgressReporter;
        let result = overwrite_file(
            &mut file,
            256 * KIB,
            automatic_policy(WriteCheck::Off),
            &progress,
            temp.path(),
        );
        assert!(result.is_err(), "expected Err for read-only handle");

        let reopened = std::fs::File::open(temp.path()).unwrap();
        assert_eq!(read_all(&reopened), original, "file must be intact");
    }

    /// The global cancel flag is process-wide state, and `write_pass` checks
    /// it before every chunk. A test that fires it must therefore never run
    /// concurrently with lifecycle tests. The shared guard also clears it on
    /// every exit path so the module is deterministic under the default
    /// parallel test harness.
    fn engine_test_lock() -> crate::shredder::cancel::GlobalStateTestGuard {
        crate::shredder::cancel::global_state_test_guard()
    }

    #[test]
    fn automatic_mid_pass_cancel_reports_partial_and_skips_final_check() {
        let _serial = engine_test_lock();
        // The global flag is process-wide state: never assume a prior test
        // left it clean. The guard resets it on every exit path.
        crate::shredder::cancel::reset_global();

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&vec![0xAAu8; 3 * MIB as usize]).unwrap();
        temp.flush().unwrap();

        let mut file = temp.reopen().unwrap();
        // The first progress event (after the 1 MiB chunk completes) fires
        // the global cancel flag; write_pass observes it before the next
        // chunk and aborts the pass.
        let progress = RecordingProgress {
            cancel_on_first_progress: AtomicBool::new(true),
            ..Default::default()
        };
        let outcome = overwrite_file(
            &mut file,
            3 * MIB,
            automatic_policy(WriteCheck::Spot),
            &progress,
            temp.path(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Partial);
        assert_eq!(
            outcome.write_check,
            WriteCheckOutcome::NotRun,
            "the final write check must never run after an interrupted pass"
        );
        assert_eq!(outcome.passes_completed, 0);
        assert_eq!(
            outcome.bytes_written, MIB,
            "the first 1 MiB chunk completed before the cancel fired"
        );
        assert!(
            outcome
                .issues
                .iter()
                .any(|e| matches!(e, ShredError::IoError { ref kind, .. } if kind == "Cancelled")),
            "expected a Cancelled issue, got {:?}",
            outcome.issues
        );
    }

    #[test]
    fn legacy_cancel_before_first_pass_reports_partial_never_passed() {
        let _serial = engine_test_lock();
        crate::shredder::cancel::reset_global();
        crate::shredder::cancel::cancel_global();

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&vec![0xAAu8; MIB as usize]).unwrap();
        temp.flush().unwrap();

        let mut file = temp.reopen().unwrap();
        let progress = NoopProgressReporter;
        let outcome = overwrite_file(
            &mut file,
            MIB,
            legacy_policy(WriteCheck::Spot),
            &progress,
            temp.path(),
        )
        .unwrap();

        assert_eq!(outcome.state, OverwriteState::Partial);
        assert_eq!(
            outcome.write_check,
            WriteCheckOutcome::NotRun,
            "a cancelled overwrite must never report a passed write check"
        );
        assert_eq!(outcome.bytes_written, 0);
        assert_eq!(outcome.passes_completed, 0);
        assert!(
            outcome
                .issues
                .iter()
                .any(|e| matches!(e, ShredError::IoError { ref kind, .. } if kind == "Cancelled")),
            "expected a Cancelled issue, got {:?}",
            outcome.issues
        );
    }

    #[test]
    fn automatic_progress_is_pass_local_and_bounded() {
        let _serial = engine_test_lock();
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&vec![0x00u8; 2 * MIB as usize]).unwrap();
        temp.flush().unwrap();

        let mut file = temp.reopen().unwrap();
        let progress = RecordingProgress::default();
        let outcome = overwrite_file(
            &mut file,
            2 * MIB,
            automatic_policy(WriteCheck::Off),
            &progress,
            temp.path(),
        )
        .unwrap();
        assert_eq!(outcome.state, OverwriteState::Completed);

        let starts = progress.pass_starts.lock().unwrap();
        let completes = progress.pass_completes.lock().unwrap();
        assert_eq!(*starts, vec![(1, 1)]);
        assert_eq!(*completes, vec![(1, 1)]);

        let events = progress.progress_events.lock().unwrap();
        assert!(!events.is_empty(), "2 MiB must produce progress events");
        for &(bytes, total) in events.iter() {
            assert!(bytes <= total, "progress {bytes} exceeds total {total}");
            assert_eq!(total, 2 * MIB, "pass-local total must equal file size");
        }
    }

    #[test]
    fn legacy_progress_totals_three_and_stays_bounded() {
        let _serial = engine_test_lock();
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&vec![0x00u8; 2 * MIB as usize]).unwrap();
        temp.flush().unwrap();

        let mut file = temp.reopen().unwrap();
        let progress = RecordingProgress::default();
        let outcome = overwrite_file(
            &mut file,
            2 * MIB,
            legacy_policy(WriteCheck::Off),
            &progress,
            temp.path(),
        )
        .unwrap();
        assert_eq!(outcome.state, OverwriteState::Completed);
        assert_eq!(outcome.passes_completed, 3);

        let starts = progress.pass_starts.lock().unwrap();
        let completes = progress.pass_completes.lock().unwrap();
        assert_eq!(*starts, vec![(1, 3), (2, 3), (3, 3)]);
        assert_eq!(*completes, vec![(1, 3), (2, 3), (3, 3)]);

        let events = progress.progress_events.lock().unwrap();
        assert!(!events.is_empty());
        for &(bytes, total) in events.iter() {
            assert!(bytes <= total, "progress {bytes} exceeds total {total}");
            assert_eq!(total, 2 * MIB);
        }
    }

    #[test]
    fn zero_length_emits_no_pass_events() {
        let _serial = engine_test_lock();
        let temp = NamedTempFile::new().unwrap();
        let mut file = temp.reopen().unwrap();
        let progress = RecordingProgress::default();
        let outcome = overwrite_file(
            &mut file,
            0,
            automatic_policy(WriteCheck::Spot),
            &progress,
            temp.path(),
        )
        .unwrap();
        assert_eq!(outcome.state, OverwriteState::Completed);

        assert!(progress.pass_starts.lock().unwrap().is_empty());
        assert!(progress.pass_completes.lock().unwrap().is_empty());
        assert!(progress.progress_events.lock().unwrap().is_empty());
    }
}
