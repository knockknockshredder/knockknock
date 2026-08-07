// src-tauri/src/commands/shred.rs

use crate::drive::{self, DriveInfo};
use crate::shredder::algorithms::all_algorithms;
use crate::shredder::cancel::CancellationToken;
use crate::shredder::journal::JournalStore;
use crate::shredder::logging::LogObfuscation;
use crate::shredder::progress::TauriProgressReporter;
use crate::shredder::root_execution::types::{BatchRootResult, ExecuteRootsRequest, TargetKind};
use crate::shredder::root_execution::{execute_roots as run_roots, SecureTreeIo};
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::*;
use crate::shredder::validation::{
    classify_path, is_network_drive, validate_path, PathClassification,
};
use crate::shredder::{LegacyOpenFileShredder, ShredAlgorithm, VerificationLevel};
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn execute_roots(
    app: AppHandle,
    request: ExecuteRootsRequest,
    algorithm_index: usize,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    log_obfuscation: String,
) -> Result<BatchRootResult, String> {
    let obfuscation = match log_obfuscation.as_str() {
        "numbered" => LogObfuscation::Numbered,
        "partial_mask" => LogObfuscation::PartialMask,
        _ => LogObfuscation::None,
    };

    let algorithms = all_algorithms();
    let algorithm = algorithms
        .get(algorithm_index)
        .ok_or_else(|| format!("Invalid algorithm index: {}", algorithm_index))?
        .clone();

    if passes > algorithm.max_passes() {
        return Err(format!(
            "Passes {} exceeds maximum {}",
            passes,
            algorithm.max_passes()
        ));
    }

    let policy = policy_from_legacy_args(algorithm_index, verification_level)?;

    // Reset cancellation token for fresh operation
    crate::shredder::cancel::reset_global();
    let cancel = crate::shredder::cancel::get_global_token();

    let progress: Arc<dyn ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app, obfuscation));
    let journal = JournalStore::portable().map_err(|error| error.to_string())?;

    tokio::task::spawn_blocking(move || {
        execute_roots_core(
            request,
            algorithm,
            passes,
            pattern,
            verification_level,
            policy,
            progress,
            &cancel,
            &journal,
        )
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))
}

/// Map the legacy IPC argument surface to the v2 deletion policy
/// (transitional shim, ORACLE-0 M4; removed in Phase 4). Legacy algorithm
/// index 1 (DoD 5220.22-M) maps to the fixed 3-pass method; indexes 0
/// (NIST 800-88 Clear) and 2 (Random Only) both map to Automatic (single
/// random pass). Unknown indexes are rejected fail-closed.
fn policy_from_legacy_args(
    algorithm_index: usize,
    verification_level: VerificationLevel,
) -> Result<DeletionPolicy, String> {
    let method = match algorithm_index {
        0 | 2 => DeletionMethod::Automatic,
        1 => DeletionMethod::LegacyThreePass,
        _ => return Err(format!("Invalid algorithm index: {algorithm_index}")),
    };
    let write_check = match verification_level {
        VerificationLevel::None => WriteCheck::Off,
        VerificationLevel::Sample => WriteCheck::Spot,
        VerificationLevel::Full => WriteCheck::Full,
    };
    Ok(DeletionPolicy { method, write_check })
}

/// Build the platform's secure tree adapter. Only Windows and Unix adapters
/// exist; KnockKnock is a desktop-only application.
fn platform_adapter() -> Arc<dyn SecureTreeIo> {
    #[cfg(windows)]
    {
        Arc::new(crate::shredder::root_execution::windows::WindowsSecureTreeIo::new())
    }
    #[cfg(unix)]
    {
        Arc::new(crate::shredder::root_execution::unix::UnixSecureTreeIo::new())
    }
    #[cfg(not(any(windows, unix)))]
    {
        compile_error!("KnockKnock supports Windows, macOS, and Linux only");
    }
}

/// Command core without the `AppHandle`: builds the platform adapter, the
/// open-file shredder, and runs the `execute_roots` seam against the given
/// policy, journal, and progress reporter. Kept separate so command behavior
/// is covered by tests that never construct a Tauri runtime.
pub(crate) fn execute_roots_core(
    request: ExecuteRootsRequest,
    algorithm: Arc<dyn ShredAlgorithm>,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    policy: DeletionPolicy,
    progress: Arc<dyn ProgressReporter>,
    cancel: &CancellationToken,
    journal: &JournalStore,
) -> BatchRootResult {
    let adapter = platform_adapter();
    let file_shredder = LegacyOpenFileShredder::new(
        algorithm,
        passes,
        pattern,
        verification_level,
        Arc::clone(&progress),
    );
    run_roots(
        request,
        policy,
        adapter.as_ref(),
        &file_shredder,
        journal,
        progress.as_ref(),
        cancel,
    )
}

#[tauri::command]
pub fn cancel_shred() {
    crate::shredder::cancel::cancel_global();
}

/// Re-launch the current executable with administrator privileges.
///
/// On Windows, this invokes `ShellExecuteW` with the `runas` verb, which
/// triggers the standard UAC elevation prompt. On a successful elevation
/// request the current process exits so the elevated instance can replace
/// it. On any failure (user cancelled UAC, no admin token available, etc.)
/// an error string is returned to the frontend.
///
/// On non-Windows platforms, returns an "unsupported" error so the UI can
/// hide the elevation control.
#[tauri::command]
pub fn request_elevation() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot determine executable path: {}", e))?;

        let exe_wide: Vec<u16> = exe
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = "runas\0".encode_utf16().collect();

        // ShellExecuteW returns an HINSTANCE. Values > 32 indicate success;
        // values <= 32 are predefined error codes (SE_ERR_*).
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                exe_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        if (result as isize) > 32 {
            // Exit the non-elevated instance so the elevated one takes over.
            std::process::exit(0);
        } else {
            Err(format!(
                "Elevation request failed (ShellExecuteW returned {})",
                result as isize
            ))
        }
    }

    #[cfg(not(windows))]
    {
        Err("Elevation is not supported on this platform".to_string())
    }
}

#[tauri::command]
pub fn cleanup_orphans() -> Result<Vec<String>, String> {
    let remaining =
        crate::shredder::journal::cleanup_orphans().map_err(|error| error.to_string())?;
    Ok(remaining
        .iter()
        .map(|e| format!("Orphaned: {:?}", e.renamed_path))
        .collect())
}

#[derive(serde::Serialize)]
pub struct AlgorithmInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub default_passes: u32,
    pub max_passes: u32,
    pub accepted_patterns: Vec<String>,
    pub has_fixed_pattern_sequence: bool,
}

#[tauri::command]
pub fn get_algorithms() -> Vec<AlgorithmInfo> {
    all_algorithms()
        .iter()
        .enumerate()
        .map(|(i, algo)| AlgorithmInfo {
            index: i,
            name: algo.name().to_string(),
            description: algo.description().to_string(),
            default_passes: algo.default_passes(),
            max_passes: algo.max_passes(),
            accepted_patterns: algo
                .accepted_patterns()
                .iter()
                .map(|p| format!("{:?}", p))
                .collect(),
            has_fixed_pattern_sequence: algo.has_fixed_pattern_sequence(),
        })
        .collect()
}

/// Collect metadata for a single file path.
///
/// `kind` is the classification surfaced to the frontend (see
/// `FileMetadata`). `is_shortcut` and `shortcut_target` are populated only
/// when the caller passed a non-`Normal` classification. Normal files pass
/// `false, None`.
fn collect_file_metadata(
    path: &std::path::Path,
    path_str: &str,
    kind: TargetKind,
    is_shortcut: bool,
    shortcut_target: Option<String>,
) -> Option<FileMetadata> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return None,
    };

    if !metadata.file_type().is_file() {
        return None;
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some(FileMetadata {
        path: path_str.to_string(),
        name,
        size: metadata.len(),
        kind,
        is_shortcut,
        shortcut_target,
    })
}

#[tauri::command]
pub fn validate_paths(paths: Vec<String>) -> Result<(Vec<FileMetadata>, Vec<String>), String> {
    let mut valid = Vec::new();
    let mut errors = Vec::new();
    for path_str in paths {
        let path = std::path::Path::new(&path_str);

        // Classify via the same logic the shredder uses, so the metadata
        // surfaced to the UI matches what the shredder will see. A
        // classification error (e.g. file disappeared between selection and
        // validation) is silently skipped — `validate_path` already reports
        // hard failures during shred.
        let classification = match classify_path(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        match classification {
            PathClassification::Shortcut { target } => {
                // Surface the shortcut with its resolved target. Link policy:
                // `.lnk` shell shortcuts are ordinary file data (kind `File`),
                // while real filesystem links (Unix symlinks, NTFS symlinks,
                // junctions) stay non-executable (kind `Link`). The frontend
                // uses `is_shortcut` to render the warning badge and
                // `shortcut_target` for the tooltip.
                let kind = match std::fs::symlink_metadata(path) {
                    Ok(meta) if meta.file_type().is_symlink() => TargetKind::Link,
                    _ => TargetKind::File,
                };
                if let Some(meta) = collect_file_metadata(
                    path,
                    &path_str,
                    kind,
                    true,
                    Some(target.to_string_lossy().to_string()),
                ) {
                    valid.push(meta);
                }
            }
            PathClassification::Normal => {
                // `Normal` covers both files and directories. Files become
                // shred candidates; a selected directory root is preserved as
                // a single `Directory` record so the frontend can render it as
                // a folder. Recursion happens only inside `execute_roots` with
                // the trusted adapters.
                let sym_meta = match std::fs::symlink_metadata(path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if sym_meta.file_type().is_file() {
                    if let Some(meta) =
                        collect_file_metadata(path, &path_str, TargetKind::File, false, None)
                    {
                        valid.push(meta);
                    }
                } else if sym_meta.file_type().is_dir() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    valid.push(FileMetadata {
                        path: path_str,
                        name,
                        size: sym_meta.len(),
                        kind: TargetKind::Directory,
                        is_shortcut: false,
                        shortcut_target: None,
                    });
                }
            }
        }
    }
    Ok((valid, errors))
}

fn target_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string())
}

fn metadata_for_target(
    target: &VaultTarget,
    kind: TargetKind,
    availability: TargetAvailability,
    reason: Option<String>,
    size: u64,
) -> TargetMetadataDto {
    let path = std::path::Path::new(&target.path);
    TargetMetadataDto {
        path: target.path.clone(),
        kind,
        availability,
        reason,
        name: target_name(path),
        size,
    }
}

fn actual_target_kind(metadata: &std::fs::Metadata) -> Option<TargetKind> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        Some(TargetKind::Link)
    } else if file_type.is_file() {
        Some(TargetKind::File)
    } else if file_type.is_dir() {
        Some(TargetKind::Directory)
    } else {
        None
    }
}

fn validate_target(target: &VaultTarget) -> TargetMetadataDto {
    let path = std::path::Path::new(&target.path);
    if target.path.trim().is_empty() {
        return metadata_for_target(
            target,
            target.kind,
            TargetAvailability::Blocked,
            Some("Target path is empty".to_string()),
            0,
        );
    }

    if !path.is_absolute() {
        return metadata_for_target(
            target,
            target.kind,
            TargetAvailability::Blocked,
            Some("Relative paths are not safe execution roots".to_string()),
            0,
        );
    }

    if is_network_drive(path) {
        return metadata_for_target(
            target,
            target.kind,
            TargetAvailability::Blocked,
            Some("Network roots are not safe execution roots".to_string()),
            0,
        );
    }

    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return match target.kind {
                TargetKind::UnknownLegacy => metadata_for_target(
                    target,
                    TargetKind::UnknownLegacy,
                    TargetAvailability::Blocked,
                    Some("Legacy target is missing".to_string()),
                    0,
                ),
                known_kind => {
                    metadata_for_target(target, known_kind, TargetAvailability::Missing, None, 0)
                }
            };
        }
        Err(error) => {
            return metadata_for_target(
                target,
                target.kind,
                TargetAvailability::Blocked,
                Some(format!("Cannot inspect target: {error}")),
                0,
            );
        }
    };

    let size = metadata.len();
    let Some(actual_kind) = actual_target_kind(&metadata) else {
        return metadata_for_target(
            target,
            target.kind,
            TargetAvailability::Blocked,
            Some("Target is not a regular file, directory, or link".to_string()),
            size,
        );
    };

    if let Err(error) = validate_path(path, false) {
        return metadata_for_target(
            target,
            if target.kind == TargetKind::UnknownLegacy {
                actual_kind
            } else {
                target.kind
            },
            TargetAvailability::Blocked,
            Some(error.to_string()),
            size,
        );
    }

    if actual_kind == TargetKind::Link {
        return metadata_for_target(
            target,
            if target.kind == TargetKind::UnknownLegacy {
                actual_kind
            } else {
                target.kind
            },
            TargetAvailability::Blocked,
            Some("Symbolic links are not safe execution roots".to_string()),
            size,
        );
    }

    match target.kind {
        TargetKind::UnknownLegacy => {
            metadata_for_target(target, actual_kind, TargetAvailability::Ready, None, size)
        }
        expected_kind if expected_kind == actual_kind => {
            metadata_for_target(target, expected_kind, TargetAvailability::Ready, None, size)
        }
        expected_kind => metadata_for_target(
            target,
            expected_kind,
            TargetAvailability::Blocked,
            Some(format!(
                "Target kind mismatch: expected {:?}, found {:?}",
                expected_kind, actual_kind
            )),
            size,
        ),
    }
}

#[tauri::command]
pub fn validate_targets(targets: Vec<VaultTarget>) -> Result<Vec<TargetMetadataDto>, String> {
    Ok(targets.iter().map(validate_target).collect())
}

/// Open a multi-select file dialog that returns raw `.lnk` paths without
/// resolving shortcut targets.
///
/// The bug being fixed: `@tauri-apps/plugin-dialog` invokes the standard
/// `IFileOpenDialog` without `FOS_NODEREFERENCELINKS`, so when a user picks a
/// `.lnk` file, the OS resolves it to the target `.exe` and the backend
/// shreds the wrong file. This command calls `IFileOpenDialog` directly with
/// the flag set, so the returned paths are the link files themselves.
///
/// Drag-drop already passes raw paths (no resolution), so this command is
/// only used by the explicit "Add Files" button on Windows.
#[cfg(windows)]
#[tauri::command]
pub fn open_files_windows() -> Result<Vec<String>, String> {
    run_windows_picker(crate::shredder::root_execution::windows::file_picker_options())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn open_files_windows() -> Result<Vec<String>, String> {
    Err("This command is only available on Windows".to_string())
}

#[cfg(windows)]
#[tauri::command]
pub fn open_folders_windows() -> Result<Vec<String>, String> {
    run_windows_picker(crate::shredder::root_execution::windows::folder_picker_options())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn open_folders_windows() -> Result<Vec<String>, String> {
    Err("This command is only available on Windows".to_string())
}

#[cfg(windows)]
fn run_windows_picker(options: u32) -> Result<Vec<String>, String> {
    let thread = std::thread::Builder::new()
        .name("knockknock-file-picker".to_string())
        .spawn(move || run_windows_picker_sta(options))
        .map_err(|error| format!("Failed to start file picker STA thread: {error}"))?;
    thread
        .join()
        .map_err(|_| "File picker STA thread panicked".to_string())?
}

#[cfg(windows)]
fn run_windows_picker_sta(options: u32) -> Result<Vec<String>, String> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FILEOPENDIALOGOPTIONS, SIGDN,
    };

    // SAFETY: this function runs only on the dedicated picker thread, which has
    // no other COM apartment. The matching CoUninitialize is below.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.0 < 0 {
        return Err(format!(
            "Failed to initialize picker STA: 0x{:08X}",
            initialized.0 as u32
        ));
    }

    let result = unsafe {
        (|| -> Result<Vec<String>, String> {
            // CoCreateInstance returns a COM pointer; bind it to the
            // IFileOpenDialog interface so we can use the high-level methods.
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("Failed to create file dialog: {}", e))?;

            dialog
                .SetOptions(FILEOPENDIALOGOPTIONS(options))
                .map_err(|e| format!("Failed to set dialog options: {}", e))?;

            // `None` here means no parent HWND â€” fine for a modeless top-level
            // dialog. Tauri commands run on their own thread and we do not have
            // access to the window handle here.
            dialog
                .Show(None)
                .map_err(|e| format!("Failed to show dialog: {}", e))?;

            let results = dialog
                .GetResults()
                .map_err(|e| format!("Failed to get dialog results: {}", e))?;
            let count = results
                .GetCount()
                .map_err(|e| format!("Failed to get result count: {}", e))?;
            let mut paths = Vec::with_capacity(count as usize);

            for i in 0..count {
                let item = results
                    .GetItemAt(i)
                    .map_err(|e| format!("Failed to get item at index {}: {}", i, e))?;
                // SIGDN_FILESYSPATH (= 0x80058000) returns the filesystem path
                // verbatim. The spec example showed `GetDisplayName(0)` which is
                // SIGDN_NORMALDISPLAY â€” that returns a human-friendly display
                // name like "Notepad.lnk", NOT a filesystem path. We need the
                // path so the shredder receives the raw `.lnk` file, not its
                // display label. SIGDN_FILESYSPATH is the correct constant.
                let display_name = item
                    .GetDisplayName(SIGDN(0x80058000u32 as i32))
                    .map_err(|e| format!("Failed to get display name for item {}: {}", i, e))?;
                // PWSTR -> String. `to_string()` is unsafe (reads the raw wide
                // pointer); in windows-rs 0.59 it returns `Result<String,
                // windows_core::Error>`. We are already inside an unsafe block
                // for COM, so this is the right scope.
                let path_str = display_name
                    .to_string()
                    .map_err(|e| format!("Failed to convert display name for item {}: {}", i, e))?;
                paths.push(path_str);
            }

            Ok(paths)
        })()
    };

    // SAFETY: the preceding CoInitializeEx succeeded on this same thread.
    unsafe { CoUninitialize() };
    result
}

/// Detect drive info for a single path.
#[tauri::command]
pub fn get_drive_info(path: String) -> Result<DriveInfo, String> {
    drive::detect_drive_info(std::path::Path::new(&path))
}

/// Detect drive info for every unique drive represented by the given paths.
///
/// Returns one `DriveInfo` per distinct drive key (e.g. `"C:"`, `"D:"`),
/// preserving first-seen order.
#[tauri::command]
pub fn get_all_drive_info(paths: Vec<String>) -> Result<Vec<DriveInfo>, String> {
    let mut drives: Vec<DriveInfo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path_str in paths {
        let path = std::path::Path::new(&path_str);
        let info = drive::detect_drive_info(path)?;
        if seen.insert(info.drive_letter.clone()) {
            drives.push(info);
        }
    }

    Ok(drives)
}

#[cfg(test)]
mod tests {
    use super::execute_roots_core;
    use super::validate_targets;
    use crate::shredder::algorithms::nist_clear::NistClear;
    use crate::shredder::cancel::CancellationToken;
    use crate::shredder::journal::JournalStore;
    use crate::shredder::progress::NoopProgressReporter;
    use crate::shredder::root_execution::types::{
        BatchRootResult, ExecuteRootRequest, ExecuteRootsRequest, ExecutionStage, RootStatus,
        TargetAvailability, TargetKind, VaultTarget,
    };
    use crate::shredder::types::{DeletionMethod, DeletionPolicy, PatternType, VerificationLevel, WriteCheck};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A real directory under the real home directory (root execution refuses
    /// roots outside the home directory), removed on drop.
    ///
    /// The fixture lives at depth 5 under home (`home/.knockknock-task11-*/
    /// inner/`): the Windows adapter opens the parent-of-parent of a file root
    /// with DELETE access, and sessions where the shell holds the profile
    /// directory without FILE_SHARE_DELETE reject that open with
    /// STATUS_SHARING_VIOLATION. A child-of-home directory at depth 2 avoids
    /// ever mutation-opening the profile directory itself.
    struct TempHome(PathBuf);

    impl TempHome {
        /// The depth-5 fixture directory (see the struct doc comment).
        fn inner(&self) -> PathBuf {
            self.0.join("inner")
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_home() -> TempHome {
        let home = std::env::home_dir().expect("home directory");
        let unique = format!(
            ".knockknock-task11-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let unique_dir = home.join(unique);
        std::fs::create_dir_all(unique_dir.join("inner")).expect("create temp home child");
        TempHome(unique_dir)
    }

    fn root_request(id: &str, path: &std::path::Path, kind: TargetKind) -> ExecuteRootRequest {
        ExecuteRootRequest {
            target_id: id.to_string(),
            path: path.to_string_lossy().into_owned(),
            kind,
        }
    }

    fn run(request: ExecuteRootsRequest) -> BatchRootResult {
        let journal_directory = tempfile::tempdir().expect("temporary journal directory");
        let journal = JournalStore::at(journal_directory.path().join("journal.json"));
        execute_roots_core(
            request,
            Arc::new(NistClear),
            1,
            PatternType::Zeros,
            VerificationLevel::None,
            DeletionPolicy {
                method: DeletionMethod::Automatic,
                write_check: WriteCheck::Off,
            },
            Arc::new(NoopProgressReporter),
            &CancellationToken::new(),
            &journal,
        )
    }

    #[test]
    fn validate_targets_command_returns_one_record_per_target() {
        let targets = vec![
            VaultTarget {
                path: "relative-file".to_string(),
                kind: TargetKind::File,
            },
            VaultTarget {
                path: "relative-directory".to_string(),
                kind: TargetKind::Directory,
            },
            VaultTarget {
                path: "relative-link".to_string(),
                kind: TargetKind::Link,
            },
        ];

        let metadata = validate_targets(targets.clone()).expect("validate target command");

        assert_eq!(metadata.len(), targets.len());
        assert_eq!(
            metadata
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            targets
                .iter()
                .map(|target| target.path.as_str())
                .collect::<Vec<_>>()
        );
        assert!(metadata
            .iter()
            .all(|entry| entry.availability == TargetAvailability::Blocked));
    }

    #[test]
    fn validate_paths_preserves_directory_roots_without_flattening() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let dir = temp.path().join("folder");
        std::fs::create_dir(&dir).expect("create directory");
        std::fs::write(dir.join("nested.txt"), b"nested").expect("write nested fixture");
        std::fs::write(dir.join("deep.txt"), b"deep").expect("write deep fixture");
        let file = temp.path().join("file.txt");
        std::fs::write(&file, b"data").expect("write file fixture");

        let (valid, errors) = super::validate_paths(vec![
            dir.to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
        ])
        .expect("validate paths command");

        assert!(
            errors.is_empty(),
            "unexpected validation errors: {errors:?}"
        );
        assert_eq!(
            valid.len(),
            2,
            "a selected directory must yield one root record, not its files"
        );

        let dir_meta = valid
            .iter()
            .find(|entry| entry.path == dir.to_string_lossy().as_ref())
            .expect("directory root record");
        assert_eq!(dir_meta.kind, TargetKind::Directory);
        assert_eq!(dir_meta.name, "folder");
        assert!(!dir_meta.is_shortcut);
        assert!(dir_meta.shortcut_target.is_none());

        let file_meta = valid
            .iter()
            .find(|entry| entry.path == file.to_string_lossy().as_ref())
            .expect("file record");
        assert_eq!(file_meta.kind, TargetKind::File);
        assert_eq!(file_meta.size, 4);
    }

    #[test]
    fn execute_roots_core_destroys_a_file_root_on_the_real_adapter() {
        let home = temp_home();
        let file = home.inner().join("secret.txt");
        std::fs::write(&file, b"top secret data").expect("write fixture");

        let result = run(ExecuteRootsRequest {
            roots: vec![root_request("file-1", &file, TargetKind::File)],
        });

        assert_eq!(result.roots.len(), 1);
        assert_eq!(result.roots[0].target_id, "file-1");
        assert_eq!(
            result.roots[0].status,
            RootStatus::Destroyed,
            "child errors: {:?}",
            result.roots[0].errors
        );
        assert!(result.roots[0].root_removed);
        assert_eq!(result.roots[0].files_destroyed, 1);
        assert!(
            !file.exists(),
            "destroyed file root must be removed from disk"
        );
    }

    #[test]
    fn execute_roots_core_destroys_a_directory_root_on_the_real_adapter() {
        let home = temp_home();
        let root_dir = home.inner().join("rootdir");
        std::fs::create_dir(&root_dir).expect("create root directory");
        let child = root_dir.join("nested.txt");
        std::fs::write(&child, b"nested data").expect("write fixture");

        let result = run(ExecuteRootsRequest {
            roots: vec![root_request("dir-1", &root_dir, TargetKind::Directory)],
        });

        assert_eq!(result.roots.len(), 1);
        assert_eq!(result.roots[0].target_id, "dir-1");
        assert_eq!(result.roots[0].status, RootStatus::Destroyed);
        assert!(result.roots[0].root_removed);
        assert_eq!(result.roots[0].files_destroyed, 1);
        assert_eq!(result.roots[0].directories_removed, 1);
        assert!(!child.exists(), "nested file must be removed");
        assert!(!root_dir.exists(), "directory root must be removed");
    }

    #[test]
    fn execute_roots_core_blocks_missing_and_kind_mismatched_roots_without_mutation() {
        let home = temp_home();
        let missing = home.inner().join("missing.txt");
        let file = home.inner().join("real.txt");
        std::fs::write(&file, b"keep this data").expect("write fixture");

        let result = run(ExecuteRootsRequest {
            roots: vec![
                root_request("missing", &missing, TargetKind::File),
                root_request("mismatch", &file, TargetKind::Directory),
            ],
        });

        assert_eq!(result.roots.len(), 2);
        assert!(result
            .roots
            .iter()
            .all(|root| root.status == RootStatus::Failed));
        assert!(result.roots.iter().all(|root| !root.root_removed));
        assert!(result.roots.iter().all(|root| {
            root.errors
                .iter()
                .all(|error| error.stage == ExecutionStage::Preflight)
        }));
        assert_eq!(
            std::fs::read(&file).expect("file must remain readable"),
            b"keep this data"
        );
        assert!(!missing.exists());
    }
}
