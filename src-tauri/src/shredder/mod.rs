// src-tauri/src/shredder/mod.rs

pub mod algorithms;
pub mod cancel;
pub mod errors;
pub mod journal;
pub mod logging;
pub mod platform;
pub mod progress;
pub mod traits;
pub mod types;
pub mod validation;
pub mod verification;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::shredder::validation::{classify_path, PathClassification};

pub use cancel::CancellationToken;
pub use errors::ShredError;
pub use traits::{PlatformIo, ProgressReporter, ShredAlgorithm, VerificationStrategy};
pub use types::{
    MediaType, PatternType, ShredReport, ShredReportError, ShredResult, VerificationLevel,
};

/// Shred a single file with full pipeline, including shortcut/symlink dispatch.
///
/// `shred_targets` controls whether shortcut targets are also shredded after the
/// link file itself. `visited` is the recursion guard — a `HashSet` of paths
/// already processed in this batch. If `path` is already present, the call
/// returns a successful no-op result (the path was handled by an earlier
/// invocation, possibly via a circular shortcut chain).
pub fn shred_file(
    path: &std::path::Path,
    algorithm: &dyn ShredAlgorithm,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: &dyn ProgressReporter,
    shred_targets: bool,
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
            // through the standard pipeline.
            let link_result = shred_file_inner(
                path,
                algorithm,
                passes,
                pattern,
                verification_level,
                progress,
                cancel,
            )?;

            if !shred_targets {
                // User did NOT opt in to shredding targets. Surface this loudly
                // so the operator is aware the target survived.
                eprintln!(
                    "[KnockKnock] Shortcut shredded. Target {} was NOT shredded.",
                    target.display()
                );
                return Ok(link_result);
            }

            // User opted in. Enforce depth limit 1: if the target is itself
            // a shortcut, stop. We refuse to follow shortcut chains because
            // each hop multiplies the surface area for unintended shreds.
            let target_class = classify_path(&target)?;
            if matches!(target_class, PathClassification::Shortcut { .. }) {
                eprintln!(
                    "[KnockKnock] Target {} is itself a shortcut or symlink; \
                     refusing to follow (depth limit 1). Enable 'Also shred linked targets' \
                     and run again to destroy the chain manually.",
                    target.display()
                );
                return Ok(link_result);
            }

            // Target is a Normal file/dir. Run the full validation pipeline on
            // it (allow_shortcut: false — if the TOCTOU window revealed a
            // symlink we still refuse).
            crate::shredder::validation::validate_path(&target, false)?;

            // Recurse into the target. The visited set already contains `path`,
            // so the target gets inserted normally and is shredded in full.
            let target_result = shred_file(
                &target,
                algorithm,
                passes,
                pattern,
                verification_level,
                progress,
                shred_targets,
                visited,
                cancel,
            )?;

            // Combine the two results. Success requires BOTH halves to succeed;
            // errors from either side propagate in the merged vector.
            let mut combined_errors = link_result.errors;
            combined_errors.extend(target_result.errors);
            let combined_success = link_result.success && target_result.success;
            let combined_passes = link_result.passes_completed + target_result.passes_completed;
            let combined_bytes = link_result.bytes_written + target_result.bytes_written;

            Ok(ShredResult {
                success: combined_success,
                passes_completed: combined_passes,
                bytes_written: combined_bytes,
                errors: combined_errors,
            })
        }
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

    // 6. Handle empty files — skip to rename/delete
    if file_size == 0 {
        let renamed = platform_io.rename_random(path)?;
        platform_io.delete(&renamed)?;
        let result = ShredResult {
            success: true,
            passes_completed: 0,
            bytes_written: 0,
            errors: vec![],
        };
        progress.on_file_complete(path, &result);
        return Ok(result);
    }

    // 7. Open file for shredding
    let mut file = platform_io.open_for_shred(path)?;

    // 8. Generate PRNG seed for deterministic Random verification.
    //    Only Random pattern needs a seed; fixed patterns (Zeros, Ones) use
    //    direct byte comparison and don't benefit from seeding.
    let prng_seed = if pattern == PatternType::Random {
        Some(verification::PrngSeed::generate()?)
    } else {
        None
    };

    // 9. Shred with per-pass verification
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
        );
        match shred_res {
            Ok(r) => {
                bytes_written_total += r.bytes_written;
                platform_io.sync_to_disk(&mut file)?;
                // Verify against the algorithm's final-pass pattern, not the user's
                // selected pattern (fixed-sequence algorithms may differ).
                let verify_pattern = algorithm.final_pattern(pattern);
                let verification_result =
                    verifier.verify(&mut file, &verify_pattern, file_size, prng_seed.as_ref())?;
                if !verification_result.passed {
                    errors.push(ShredError::VerificationFailed {
                        path: path.to_path_buf(),
                        pass: passes,
                    });
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
            Err(e) => return Err(e),
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
            )?;
            bytes_written_total += result.bytes_written;

            // Flush to disk
            platform_io.sync_to_disk(&mut file)?;

            // Verify after each pass
            let verification_result =
                verifier.verify(&mut file, &pattern, file_size, prng_seed.as_ref())?;
            if !verification_result.passed {
                errors.push(ShredError::VerificationFailed {
                    path: path.to_path_buf(),
                    pass: pass + 1,
                });
            }

            progress.on_pass_complete(pass + 1, passes);
        }
    }

    // 9. Close file handle before rename/delete
    drop(file);

    // 10. Rename to random name. Always run the cleanup pipeline even if the
    //     shredding pass was cancelled — leaving the original-named partially-
    //     overwritten file on disk is the catastrophic failure mode this
    //     stage is here to prevent.
    let was_cancelled = cancel.is_cancelled();
    let renamed_path = platform_io.rename_random(path)?;
    // Record orphan so a partial-failure recovery can clean it up later
    crate::shredder::journal::write_orphan(path, &renamed_path);

    // 11. Truncate (re-open briefly)
    {
        let mut f = platform_io.open_for_shred(&renamed_path)?;
        platform_io.truncate_to_zero(&mut f)?;
    }

    // 12. Issue TRIM for SSDs BEFORE delete (file must still exist on the
    //     volume for TRIM to apply). This is the correct ordering for SSD
    //     wear-leveling: the FTL needs the LBA range before it's freed.
    if media_type == MediaType::Ssd {
        if let Err(e) = platform_io.issue_trim(&renamed_path) {
            progress.on_warning(path, &format!("TRIM failed: {}", e));
        }
    }

    // 13. Delete
    platform_io.delete(&renamed_path)?;
    // Deletion succeeded — clear the orphan record
    crate::shredder::journal::clear_orphan(&renamed_path);

    // Surface cancellation in the final result, alongside any errors that
    // were already collected. Cleanup ran, but the user must still see the
    // operation as unsuccessful.
    let result = ShredResult {
        success: errors.is_empty() && !was_cancelled,
        passes_completed: passes,
        bytes_written: bytes_written_total,
        errors,
    };

    progress.on_file_complete(path, &result);
    Ok(result)
}

/// Shred multiple files, continuing on error.
///
/// `shred_targets` propagates to each individual `shred_file` call. A fresh
/// `visited` set is created per batch — cross-batch deduplication is not
/// required (each user-initiated shred is a distinct operation).
pub fn shred_files(
    paths: Vec<std::path::PathBuf>,
    algorithm: std::sync::Arc<dyn ShredAlgorithm>,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: std::sync::Arc<dyn ProgressReporter>,
    shred_targets: bool,
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
            shred_targets,
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
