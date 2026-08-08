// src-tauri/src/shredder/mod.rs

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

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::shredder::engine::OverwriteState;
use crate::shredder::types::{DeletionPolicy, WriteCheckOutcome};

pub use errors::ShredError;
pub use traits::ProgressReporter;
pub use types::ShredResult;

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

/// Validate an execution-time hard-link count queried from an already-open
/// file handle. Query failures propagate so destructive writes fail closed.
pub(crate) fn validate_open_handle_link_count(
    path: &Path,
    link_count: Result<u64, ShredError>,
) -> Result<(), ShredError> {
    let count = link_count?;
    if count > 1 {
        return Err(ShredError::HardLinkBlocked {
            path: path.to_path_buf(),
            count,
        });
    }

    Ok(())
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
        // reopening by path. A query error hard-blocks because it cannot prove
        // the already-open handle is not hard linked.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            validate_open_handle_link_count(
                request.diagnostic_path(),
                file.metadata()
                    .map(|metadata| metadata.nlink())
                    .map_err(|error| {
                        ShredError::from_io_error(request.diagnostic_path().to_path_buf(), error)
                    }),
            )?;
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            validate_open_handle_link_count(
                request.diagnostic_path(),
                windows_link_count(file.as_raw_handle()),
            )?;
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
