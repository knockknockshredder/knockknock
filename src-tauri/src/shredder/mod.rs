// src-tauri/src/shredder/mod.rs

pub mod algorithms;
pub mod errors;
pub mod platform;
pub mod progress;
pub mod traits;
pub mod types;
pub mod validation;
pub mod verification;

#[cfg(test)]
mod tests;

pub use errors::ShredError;
pub use traits::{PlatformIo, ProgressReporter, ShredAlgorithm, VerificationStrategy};
pub use types::{
    HardLinkInfo, MediaType, PatternType, ProcessInfo, ProgressEvent, ShredReport,
    ShredReportError, ShredResult, ShredStatus, VerificationLevel, VerificationResult,
};

/// Shred a single file with full pipeline
pub fn shred_file(
    path: &std::path::Path,
    algorithm: &dyn ShredAlgorithm,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: &dyn ProgressReporter,
) -> Result<ShredResult, ShredError> {
    let start = std::time::Instant::now();

    // 1. Validate path
    validation::validate_path(path)?;

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
            verification_passed: true,
            errors: vec![],
            duration: start.elapsed(),
        };
        progress.on_file_complete(path, &result);
        return Ok(result);
    }

    // 7. Open file for shredding
    let mut file = platform_io.open_for_shred(path)?;

    // 8. Shred with per-pass verification
    let verifier = verification::create_verifier(verification_level);
    let mut bytes_written_total = 0u64;
    let mut errors = Vec::new();

    if algorithm.has_fixed_pattern_sequence() {
        // Let algorithm handle multi-pass with its fixed sequence
        progress.on_pass_start(1, passes);
        let result = algorithm.shred(&mut file, file_size, passes, pattern, progress)?;
        bytes_written_total += result.bytes_written;
        platform_io.sync_to_disk(&mut file)?;
        // Verify after all passes
        let verification_result = verifier.verify(&mut file, &pattern, file_size)?;
        if !verification_result.passed {
            errors.push(ShredError::VerificationFailed {
                path: path.to_path_buf(),
                pass: passes,
            });
        }
        progress.on_pass_complete(passes, passes);
    } else {
        for pass in 0..passes {
            progress.on_pass_start(pass + 1, passes);

            // Write pattern
            let result = algorithm.shred(&mut file, file_size, 1, pattern, progress)?;
            bytes_written_total += result.bytes_written;

            // Flush to disk
            platform_io.sync_to_disk(&mut file)?;

            // Verify after each pass
            let verification_result = verifier.verify(&mut file, &pattern, file_size)?;
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

    // 10. Rename to random name
    let renamed_path = platform_io.rename_random(path)?;

    // 11. Truncate (re-open briefly)
    {
        let mut f = platform_io.open_for_shred(&renamed_path)?;
        platform_io.truncate_to_zero(&mut f)?;
    }

    // 12. Delete
    platform_io.delete(&renamed_path)?;

    // 13. Issue TRIM for SSDs
    if media_type == MediaType::Ssd {
        let _ = platform_io.issue_trim(path);
    }

    let result = ShredResult {
        success: errors.is_empty(),
        passes_completed: passes,
        bytes_written: bytes_written_total,
        verification_passed: errors.is_empty(),
        errors,
        duration: start.elapsed(),
    };

    progress.on_file_complete(path, &result);
    Ok(result)
}

/// Shred multiple files, continuing on error
pub fn shred_files(
    paths: Vec<std::path::PathBuf>,
    algorithm: std::sync::Arc<dyn ShredAlgorithm>,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    progress: std::sync::Arc<dyn ProgressReporter>,
) -> ShredReport {
    let start = std::time::Instant::now();
    let mut successful = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    let mut total_bytes = 0u64;

    for path in &paths {
        match shred_file(
            path,
            algorithm.as_ref(),
            passes,
            pattern,
            verification_level,
            progress.as_ref(),
        ) {
            Ok(result) => {
                if result.success {
                    successful += 1;
                    total_bytes += result.bytes_written;
                } else {
                    failed += 1;
                    // Copy verification errors to report
                    for err in result.errors {
                        errors.push(ShredReportError {
                            path: path.to_string_lossy().to_string(),
                            error: err.to_string(),
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
