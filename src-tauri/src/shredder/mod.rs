// src-tauri/src/shredder/mod.rs

pub mod algorithms;
pub mod cancel;
pub mod engine;
pub mod errors;
pub mod journal;
pub mod logging;
pub mod platform;
pub mod progress;
pub mod root_execution;
pub mod traits;
pub mod types;
pub mod validation;
pub mod verification;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use crate::shredder::engine::OverwriteState;
use crate::shredder::types::{DeletionPolicy, WriteCheckOutcome};
use crate::shredder::validation::{classify_path, PathClassification};

pub use cancel::CancellationToken;
pub use errors::ShredError;
pub use traits::{PlatformIo, ProgressReporter, ShredAlgorithm, VerificationStrategy};
pub use types::{
    MediaType, PatternType, ShredReport, ShredReportError, ShredResult, VerificationLevel,
};

/// Shred a single file with full pipeline, including shortcut/symlink
/// dispatch. `visited` is the recursion guard — a `HashSet` of paths already
/// processed in this batch. If `path` is already present, the call returns a
/// successful no-op result (the path was handled by an earlier invocation).
pub fn shred_file(
    path: &std::path::Path,
    algorithm: &dyn ShredAlgorithm,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: &dyn ProgressReporter,
    visited: &mut HashSet<PathBuf>,
    cancel: &CancellationToken,
) -> Result<ShredResult, ShredError> {
    // Recursion guard. Insert the path BEFORE classifying so a circular
    // shortcut chain (A -> B -> A) cannot recurse indefinitely. If the path
    // is already in the set, the caller has already shredded (or decided not
    // to shred) it in this batch — return a successful no-op.
    if !visited.insert(path.to_path_buf()) {
        eprintln!(
            "[KnockKnock] Warning: Circular shortcut reference detected at {:?}; skipping.",
            path
        );
        progress.on_file_complete(
            path,
            &ShredResult {
                success: true,
                passes_completed: 0,
                bytes_written: 0,
                errors: vec![],
            },
            0,
        );
        return Ok(ShredResult {
            success: true,
            passes_completed: 0,
            bytes_written: 0,
            errors: vec![],
        });
    }

    // Classify the path as Normal or Shortcut (any link type: .lnk, NTFS
    // symlink, junction, Unix symlink). The classification result drives the
    // dispatch below.
    let classification = classify_path(path)?;

    match classification {
        PathClassification::Normal => {
            // Existing shred pipeline, untouched.
            shred_file_inner(
                path,
                algorithm,
                passes,
                pattern,
                verification_level,
                progress,
                cancel,
            )
        }
        PathClassification::Shortcut { target } => {
            // Always shred the link file itself first — that is what the user
            // selected. The .lnk (or symlink) is a real file on disk and goes
            // through the standard pipeline. Linked targets are never
            // followed: root execution refuses link roots, and the legacy
            // pipeline has no target-following mode.
            let link_result = shred_file_inner(
                path,
                algorithm,
                passes,
                pattern,
                verification_level,
                progress,
                cancel,
            )?;

            eprintln!(
                "[KnockKnock] Shortcut shredded. Target {} was NOT shredded.",
                target.display()
            );
            Ok(link_result)
        }
    }
}

/// Run the cleanup pipeline: rename → journal → truncate → TRIM → delete.
/// Called after the file handle is dropped to prevent partially-overwritten
/// data from surviving at the original filename.
fn cleanup_after_shred(
    path: &std::path::Path,
    platform_io: &dyn PlatformIo,
    progress: &dyn ProgressReporter,
    media_type: MediaType,
) -> Result<(), ShredError> {
    let journal = crate::shredder::journal::JournalStore::portable()
        .map_err(|error| journal_error(path, error))?;
    cleanup_after_shred_with_journal(path, platform_io, progress, media_type, &journal)
}

fn cleanup_after_shred_with_journal(
    path: &std::path::Path,
    platform_io: &dyn PlatformIo,
    progress: &dyn ProgressReporter,
    media_type: MediaType,
    journal: &crate::shredder::journal::JournalStore,
) -> Result<(), ShredError> {
    // Rename to random name
    let renamed_path = platform_io.rename_random(path)?;

    // Record orphan for crash recovery. Restore the original name if the
    // journal cannot be made durable, so no untracked renamed file remains.
    let entry = match journal.append_orphan(path, &renamed_path) {
        Ok(entry) => entry,
        Err(error) => {
            let journal_error = journal_error(&renamed_path, error);
            return match platform_io.restore_renamed(&renamed_path, path) {
                Ok(()) => Err(journal_error),
                Err(restore_error) => Err(ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "JournalRecovery".to_string(),
                    message: format!(
                        "{journal_error}; restoring renamed file failed: {restore_error}"
                    ),
                }),
            };
        }
    };

    // Truncate to zero
    {
        let mut f = platform_io.open_for_shred(&renamed_path)?;
        platform_io.truncate_to_zero(&mut f, &renamed_path)?;
    }

    // TRIM for SSDs
    if media_type == MediaType::Ssd {
        if let Err(e) = platform_io.issue_trim(&renamed_path) {
            progress.on_warning(path, &format!("TRIM failed: {}", e));
        }
    }

    // Delete
    platform_io.delete(&renamed_path)?;

    // Clear orphan entry
    journal
        .clear(&entry)
        .map_err(|error| journal_error(&renamed_path, error))?;

    Ok(())
}

fn journal_error(
    path: &std::path::Path,
    error: crate::shredder::errors::JournalError,
) -> ShredError {
    ShredError::IoError {
        path: path.to_path_buf(),
        kind: "Journal".to_string(),
        message: error.to_string(),
    }
}

/// Inner shred pipeline — the actual overwrite/rename/truncate/delete sequence
/// for a single path. Assumes the caller has already validated and classified
/// the path; this function never re-checks for shortcuts.
fn shred_file_inner(
    path: &std::path::Path,
    algorithm: &dyn ShredAlgorithm,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: &dyn ProgressReporter,
    cancel: &CancellationToken,
) -> Result<ShredResult, ShredError> {
    // 1. Validate path. `allow_shortcut: false` mirrors the original
    //    behavior (reject symlinks with an error). The outer `shred_file`
    //    wrapper already classified this path as Normal before calling
    //    here, so the shortcut check is a defense-in-depth guard against
    //    a TOCTOU race where the file becomes a link between classification
    //    and validation. Failing loud beats shredding a symlink target.
    validation::validate_path(path, false)?;

    // 2. Reject network drives
    if validation::is_network_drive(path) {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }

    // 3. Check hard links (warn, don't block)
    let hard_link_info = validation::check_hard_links(path)?;
    if hard_link_info.link_count > 1 {
        progress.on_warning(
            path,
            &format!(
                "File has {} hard links. Shredding this path will not destroy data at other links.",
                hard_link_info.link_count
            ),
        );
    }

    // 4. Detect media type
    let platform_io = platform::create_platform_io();
    let media_type = platform_io.detect_media_type(path)?;
    if media_type == MediaType::Ssd && passes > 1 {
        progress.on_warning(
            path,
            "Multi-pass shredding is less effective on SSDs due to wear leveling.",
        );
    }

    // 5. Get file size
    let metadata =
        std::fs::metadata(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
    let file_size = metadata.len();

    progress.on_file_start(path, file_size);

    // 6. Validate pattern is accepted by this algorithm
    if !algorithm.accepted_patterns().contains(&pattern) {
        return Err(ShredError::ValidationFailed(format!(
            "Algorithm '{}' does not support pattern '{:?}'",
            algorithm.name(),
            pattern
        )));
    }

    // 7. Handle empty files — skip to overwrite/rename/delete
    if file_size == 0 {
        let renamed = platform_io.rename_random(path)?;
        platform_io.delete(&renamed)?;
        let result = ShredResult {
            success: true,
            passes_completed: 0,
            bytes_written: 0,
            errors: vec![],
        };
        progress.on_file_complete(path, &result, 0);
        return Ok(result);
    }

    // 8. Open file for shredding
    let mut file = platform_io.open_for_shred(path)?;

    // 9. Generate PRNG seed for deterministic Random verification.
    //    Only Random pattern needs a seed; fixed patterns (Zeros, Ones) use
    //    direct byte comparison and don't benefit from seeding.
    let prng_seed = if pattern == PatternType::Random {
        Some(verification::PrngSeed::generate()?)
    } else {
        None
    };

    // 10. Shred with per-pass verification
    let verifier = verification::create_verifier(verification_level);
    let mut bytes_written_total = 0u64;
    let mut errors = Vec::new();

    if algorithm.has_fixed_pattern_sequence() {
        // Let algorithm handle multi-pass with its fixed sequence.
        // Cancellation is surfaced by write_pass inside the algorithm; we
        // must NOT propagate it via `?` because that would skip the
        // rename/truncate/delete cleanup pipeline. Catch Cancelled here and
        // continue to cleanup.
        progress.on_pass_start(1, passes);
        let shred_res = algorithm.shred(
            &mut file,
            file_size,
            passes,
            pattern,
            progress,
            prng_seed.as_ref(),
            path,
        );
        match shred_res {
            Ok(r) => {
                bytes_written_total += r.bytes_written;
                if let Err(e) = platform_io.sync_to_disk(&mut file, path) {
                    progress.on_error(path, &e);
                    errors.push(e);
                } else {
                    // Verify against the algorithm's final-pass pattern, not the user's
                    // selected pattern (fixed-sequence algorithms may differ).
                    let verify_pattern = algorithm.final_pattern(pattern);
                    match verifier.verify(
                        &mut file,
                        &verify_pattern,
                        file_size,
                        prng_seed.as_ref(),
                        path,
                    ) {
                        Ok(verification_result) => {
                            if !verification_result.passed {
                                errors.push(ShredError::VerificationFailed {
                                    path: path.to_path_buf(),
                                    pass: passes,
                                });
                            }
                        }
                        Err(e) => {
                            progress.on_error(path, &e);
                            errors.push(e);
                        }
                    }
                }
            }
            Err(ShredError::IoError { kind, .. }) if kind == "Cancelled" => {
                // Mid-shred cancellation: preserve partial state in `errors`
                // and continue into the cleanup pipeline below. The file
                // will still be renamed, truncated, and deleted — no
                // partially-shredded file leaks back to disk under its
                // original name.
                errors.push(ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "Cancelled".to_string(),
                    message: "Shredding cancelled during pass".to_string(),
                });
                progress.on_error(
                    path,
                    &ShredError::IoError {
                        path: path.to_path_buf(),
                        kind: "Cancelled".to_string(),
                        message: "Shredding cancelled during pass".to_string(),
                    },
                );
            }
            Err(e) => {
                progress.on_error(path, &e);
                errors.push(e);
            }
        }
        progress.on_pass_complete(passes, passes);
    } else {
        for pass in 0..passes {
            if cancel.is_cancelled() {
                errors.push(ShredError::IoError {
                    path: path.to_path_buf(),
                    kind: "Cancelled".to_string(),
                    message: format!("Shredding cancelled before pass {}", pass + 1),
                });
                progress.on_error(
                    path,
                    &ShredError::IoError {
                        path: path.to_path_buf(),
                        kind: "Cancelled".to_string(),
                        message: format!("Shredding cancelled before pass {}", pass + 1),
                    },
                );
                break;
            }

            progress.on_pass_start(pass + 1, passes);

            // Write pattern
            let result = algorithm.shred(
                &mut file,
                file_size,
                1,
                pattern,
                progress,
                prng_seed.as_ref(),
                path,
            );
            match result {
                Ok(r) => {
                    bytes_written_total += r.bytes_written;
                }
                Err(ShredError::IoError { kind, .. }) if kind == "Cancelled" => {
                    errors.push(ShredError::IoError {
                        path: path.to_path_buf(),
                        kind: "Cancelled".to_string(),
                        message: format!("Shredding cancelled during pass {}", pass + 1),
                    });
                    progress.on_error(
                        path,
                        &ShredError::IoError {
                            path: path.to_path_buf(),
                            kind: "Cancelled".to_string(),
                            message: format!("Shredding cancelled during pass {}", pass + 1),
                        },
                    );
                    break;
                }
                Err(e) => {
                    progress.on_error(path, &e);
                    errors.push(e);
                    break;
                }
            }

            // Flush to disk
            if let Err(e) = platform_io.sync_to_disk(&mut file, path) {
                progress.on_error(path, &e);
                errors.push(e);
                break;
            }

            // Verify after each pass
            match verifier.verify(&mut file, &pattern, file_size, prng_seed.as_ref(), path) {
                Ok(verification_result) => {
                    if !verification_result.passed {
                        errors.push(ShredError::VerificationFailed {
                            path: path.to_path_buf(),
                            pass: pass + 1,
                        });
                    }
                }
                Err(e) => {
                    progress.on_error(path, &e);
                    errors.push(e);
                    break;
                }
            }

            progress.on_pass_complete(pass + 1, passes);
        }
    }

    // 11. Close file handle before rename/delete
    drop(file);

    // 12. Run the cleanup pipeline (rename → truncate → TRIM → delete)
    //     even if shredding was cancelled — leaving a partially-overwritten
    //     file at its original name is the catastrophic failure mode we
    //     prevent here.
    let was_cancelled = cancel.is_cancelled();
    if let Err(cleanup_err) = cleanup_after_shred(path, &*platform_io, progress, media_type) {
        errors.push(cleanup_err);
    }

    // Surface cancellation in the final result, alongside any errors that
    // were already collected. Cleanup ran, but the user must still see the
    // operation as unsuccessful.
    let result = ShredResult {
        success: errors.is_empty() && !was_cancelled,
        passes_completed: passes,
        bytes_written: bytes_written_total,
        errors,
    };

    progress.on_file_complete(path, &result, passes);
    Ok(result)
}

/// Shred multiple files, continuing on error.
///
/// A fresh `visited` set is created per batch — cross-batch deduplication is
/// not required (each user-initiated shred is a distinct operation).
pub fn shred_files(
    paths: Vec<std::path::PathBuf>,
    algorithm: std::sync::Arc<dyn ShredAlgorithm>,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: std::sync::Arc<dyn ProgressReporter>,
) -> ShredReport {
    use crate::commands::error::ShredErrorDto;

    let start = std::time::Instant::now();
    let mut successful = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    let mut total_bytes = 0u64;

    let cancel_token = crate::shredder::cancel::get_global_token();

    // Fresh visited set per batch.
    let mut visited: HashSet<PathBuf> = HashSet::new();

    for path in &paths {
        if cancel_token.is_cancelled() {
            // Skip remaining files once cancelled
            skipped += paths.len() - successful - failed;
            break;
        }
        match shred_file(
            path,
            algorithm.as_ref(),
            passes,
            pattern,
            verification_level,
            progress.as_ref(),
            &mut visited,
            &cancel_token,
        ) {
            Ok(result) => {
                if result.success {
                    successful += 1;
                    total_bytes += result.bytes_written;
                } else {
                    failed += 1;
                    // Copy verification errors to report via the IPC DTO so the
                    // frontend gets the stable error_type/actionable fields,
                    // not just the Display string.
                    for err in result.errors {
                        let dto = ShredErrorDto::from(&err);
                        errors.push(ShredReportError {
                            path: dto
                                .path
                                .unwrap_or_else(|| path.to_string_lossy().to_string()),
                            error: dto.message,
                        });
                    }
                }
            }
            Err(e) => {
                progress.on_error(path, &e);
                errors.push(ShredReportError {
                    path: path.to_string_lossy().to_string(),
                    error: e.to_string(),
                });
                failed += 1;
            }
        }
    }

    ShredReport {
        total_files: paths.len(),
        successful,
        failed,
        skipped,
        errors,
        total_bytes_shredded: total_bytes,
        duration_secs: start.elapsed().as_secs_f64(),
    }
}

/// v2 policy-driven open-file shredder (M2/M3/M4/M6).
///
/// Owns the per-file destructive write lifecycle on top of the v2 engine:
/// identity/kind checks, the execution-time hard-link recheck against the
/// ALREADY-OPEN handle (never reopen by path — M6), the overwrite passes,
/// and the final-state write check. No truncate (`set_len(0)`) exists in
/// this lifecycle (M4). Rename/unlink stay with the secure tree adapter.
///
/// Result contract (M2): `Err` means no byte was written (NotStarted —
/// target intact, the caller must NOT journal/rename); `Ok(Partial)` means
/// best-effort cleanup must run; `Ok(Completed)` proceeds regardless of the
/// write-check status (a failed check is surfaced via `write_check_status`
/// and an issue).
pub(crate) struct PolicyFileShredder {
    policy: DeletionPolicy,
    progress: Arc<dyn ProgressReporter>,
}

impl PolicyFileShredder {
    pub(crate) fn new(policy: DeletionPolicy, progress: Arc<dyn ProgressReporter>) -> Self {
        Self { policy, progress }
    }

    /// Emit the legacy-shaped completion event the progress reporter expects.
    /// `total_passes` is always `policy.total_passes()` (>= 1, never 0 —
    /// D8); `passes_completed` is the engine's count (0 for zero-length and
    /// NotStarted outcomes). Issues are NOT mirrored into the event: the
    /// legacy event shape has no issue field and errors flow through the
    /// structured result DTO.
    fn report_completion(
        &self,
        path: &std::path::Path,
        overwrite_state: OverwriteState,
        write_check_status: WriteCheckOutcome,
        passes_completed: u32,
        bytes_shredded: u64,
    ) {
        self.progress.on_file_complete(
            path,
            &ShredResult {
                success: overwrite_state == OverwriteState::Completed
                    && write_check_status != WriteCheckOutcome::Failed,
                passes_completed,
                bytes_written: bytes_shredded,
                errors: Vec::new(),
            },
            self.policy.total_passes(),
        );
    }
}

impl crate::shredder::root_execution::OpenFileShredder for PolicyFileShredder {
    fn shred_open_file(
        &self,
        mut file: File,
        identity: crate::shredder::root_execution::NodeIdentity,
        request: &crate::shredder::root_execution::FileShredRequest,
    ) -> Result<crate::shredder::root_execution::FileShredResult, ShredError> {
        use crate::shredder::root_execution::{FileShredResult, NodeKind};

        // 1. Identity/kind checks: only already-open regular files.
        if identity.kind() != NodeKind::RegularFile {
            return Err(ShredError::ValidationFailed(
                "open-file shredder requires a regular-file identity".to_string(),
            ));
        }
        let metadata = file.metadata().map_err(|error| {
            ShredError::from_io_error(request.diagnostic_path().to_path_buf(), error)
        })?;
        if !metadata.file_type().is_file() {
            return Err(ShredError::ValidationFailed(
                "open-file shredder rejected a non-regular file handle".to_string(),
            ));
        }

        // 2. Execution-time hard-link recheck against the already-open handle
        // (M6). Preflight already blocks link counts > 1; this recheck covers
        // a link created between preflight and the overwrite without ever
        // reopening by path. A query error does NOT hard-block: preflight is
        // the enforcement point, and a failed query cannot prove a link
        // count > 1, so the already-validated target proceeds.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = file.metadata() {
                if metadata.nlink() > 1 {
                    return Err(ShredError::HardLinkBlocked {
                        path: request.diagnostic_path().to_path_buf(),
                        count: metadata.nlink(),
                    });
                }
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            if let Ok(count) = windows_link_count(file.as_raw_handle()) {
                if count > 1 {
                    return Err(ShredError::HardLinkBlocked {
                        path: request.diagnostic_path().to_path_buf(),
                        count,
                    });
                }
            }
        }

        // 3. Overwrite lifecycle: passes → sync → final write check.
        // Zero-length files get the vacuous Completed/NotRun outcome and
        // proceed straight to journal → rename → unlink (M4 lifecycle).
        let path = request.diagnostic_path();
        let file_size = metadata.len();
        self.progress.on_file_start(path, file_size);
        let outcome = match engine::overwrite_file(
            &mut file,
            file_size,
            self.policy,
            self.progress.as_ref(),
            path,
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                // `Err` == NotStarted: no byte was written, the target is
                // intact, and the caller must NOT journal/rename (M2 rule 1).
                let result = FileShredResult {
                    overwrite_state: OverwriteState::NotStarted,
                    write_check_status: WriteCheckOutcome::NotRun,
                    bytes_shredded: 0,
                    issues: vec![error],
                };
                self.report_completion(
                    path,
                    result.overwrite_state,
                    result.write_check_status,
                    0,
                    0,
                );
                return Ok(result);
            }
        };
        let result = FileShredResult {
            overwrite_state: outcome.state,
            write_check_status: outcome.write_check,
            bytes_shredded: outcome.bytes_written,
            issues: outcome.issues,
        };
        self.report_completion(
            path,
            result.overwrite_state,
            result.write_check_status,
            outcome.passes_completed,
            result.bytes_shredded,
        );
        Ok(result)
    }
}

/// Query the hard-link count of an already-open Windows handle for the M6
/// execution-time recheck. Never opens by path.
#[cfg(windows)]
fn windows_link_count(handle: std::os::windows::io::RawHandle) -> Result<u64, ShredError> {
    use std::mem::MaybeUninit;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: `information` is writable storage for the documented structure
    // and the handle is owned by the caller for the duration of the call.
    if unsafe { GetFileInformationByHandle(handle as _, information.as_mut_ptr()) } == 0 {
        return Err(ShredError::from_io_error(
            PathBuf::from("<open-handle>"),
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: GetFileInformationByHandle returned success and initialized the
    // complete structure.
    let information = unsafe { information.assume_init() };
    Ok(information.nNumberOfLinks as u64)
}
