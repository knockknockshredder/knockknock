// src-tauri/src/commands/shred.rs

use crate::drive::{self, DriveInfo};
use crate::shredder::algorithms::all_algorithms;
use crate::shredder::logging::LogObfuscation;
use crate::shredder::progress::TauriProgressReporter;
use crate::shredder::types::*;
use crate::shredder::validation::{PathClassification, classify_path};
use crate::shredder::VerificationLevel;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn shred_files(
    app: AppHandle,
    paths: Vec<String>,
    algorithm_index: usize,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
    log_obfuscation: String,
    shred_targets: bool,
) -> Result<ShredReport, String> {
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
        return Err(format!("Passes {} exceeds maximum {}", passes, algorithm.max_passes()));
    }

    // Reset cancellation token for fresh operation
    crate::shredder::cancel::reset_global();

    let progress: Arc<dyn crate::shredder::traits::ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app, obfuscation));

    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

    let report = tokio::task::spawn_blocking(move || {
        crate::shredder::shred_files(
            path_bufs,
            algorithm,
            passes,
            pattern,
            verification_level,
            progress,
            shred_targets,
        )
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    Ok(report)
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
pub fn cleanup_orphans() -> Vec<String> {
    let remaining = crate::shredder::journal::cleanup_orphans();
    remaining
        .iter()
        .map(|e| format!("Orphaned: {:?}", e.renamed_path))
        .collect()
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
            accepted_patterns: algo.accepted_patterns().iter().map(|p| format!("{:?}", p)).collect(),
            has_fixed_pattern_sequence: algo.has_fixed_pattern_sequence(),
        })
        .collect()
}

/// Collect metadata for a single path.
///
/// `is_shortcut` and `shortcut_target` are populated only when the caller
/// passed a non-`Normal` classification. Normal files pass `false, None`.
fn collect_file_metadata(
    path: &std::path::Path,
    path_str: &str,
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
        is_shortcut,
        shortcut_target,
    })
}

/// Recursively collect all files from a directory, classifying each entry.
fn collect_files_from_dir(
    dir: &std::path::Path,
    valid: &mut Vec<FileMetadata>,
    errors: &mut Vec<String>,
    depth: usize,
) {
    const MAX_DEPTH: usize = 50;
    if depth > MAX_DEPTH {
        errors.push(format!(
            "Max directory depth ({}) exceeded at: {:?}",
            MAX_DEPTH, dir
        ));
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Cannot read directory {:?}: {}", dir, e));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Classify first — shortcuts are surfaced in `valid` with their
        // resolved target rather than silently skipped. A symlink-to-directory
        // would short-circuit here and never recurse, which is the desired
        // safety behaviour for the no-`shred_targets` mode.
        let classification = match classify_path(&path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("Cannot classify {:?}: {}", path, e));
                continue;
            }
        };

        match classification {
            PathClassification::Shortcut { target } => {
                let path_str = path.to_string_lossy().to_string();
                if let Some(meta) = collect_file_metadata(
                    &path,
                    &path_str,
                    true,
                    Some(target.to_string_lossy().to_string()),
                ) {
                    valid.push(meta);
                }
            }
            PathClassification::Normal => {
                let metadata = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(e) => {
                        errors.push(format!("Cannot stat {:?}: {}", path, e));
                        continue;
                    }
                };

                if metadata.file_type().is_file() {
                    let path_str = path.to_string_lossy().to_string();
                    if let Some(meta) = collect_file_metadata(&path, &path_str, false, None) {
                        valid.push(meta);
                    }
                } else if metadata.file_type().is_dir() {
                    collect_files_from_dir(&path, valid, errors, depth + 1);
                }
            }
        }
    }
}

#[tauri::command]
pub fn validate_paths(
    paths: Vec<String>,
) -> Result<(Vec<FileMetadata>, Vec<String>), String> {
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
                // Surface the shortcut with its resolved target. The frontend
                // uses `is_shortcut` to render the warning badge and
                // `shortcut_target` for the tooltip.
                if let Some(meta) = collect_file_metadata(
                    path,
                    &path_str,
                    true,
                    Some(target.to_string_lossy().to_string()),
                ) {
                    valid.push(meta);
                }
            }
            PathClassification::Normal => {
                // `Normal` covers both files and directories. Recurse into
                // directories; treat plain files as shred candidates.
                let sym_meta = match std::fs::symlink_metadata(path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if sym_meta.file_type().is_file() {
                    if let Some(meta) = collect_file_metadata(path, &path_str, false, None) {
                        valid.push(meta);
                    }
                } else if sym_meta.file_type().is_dir() {
                    collect_files_from_dir(path, &mut valid, &mut errors, 0);
                }
            }
        }
    }
    Ok((valid, errors))
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
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::Shell::{
        FOS_FILEMUSTEXIST, FOS_NODEREFERENCELINKS, FOS_PATHMUSTEXIST, FileOpenDialog,
        FILEOPENDIALOGOPTIONS, IFileOpenDialog, SIGDN,
    };

    unsafe {
        // CoCreateInstance returns a COM pointer; bind it to the
        // IFileOpenDialog interface so we can use the high-level methods.
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Failed to create file dialog: {}", e))?;

        // Combine the three required flags into the FILEOPENDIALOGOPTIONS
        // bitfield. The `.0` accessors strip the newtype wrappers.
        let options = FILEOPENDIALOGOPTIONS(
            FOS_FILEMUSTEXIST.0 | FOS_PATHMUSTEXIST.0 | FOS_NODEREFERENCELINKS.0,
        );
        dialog
            .SetOptions(options)
            .map_err(|e| format!("Failed to set dialog options: {}", e))?;

        // `None` here means no parent HWND — fine for a modeless top-level
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
            // SIGDN_NORMALDISPLAY — that returns a human-friendly display
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
    }
}

#[cfg(not(windows))]
#[tauri::command]
pub fn open_files_windows() -> Result<Vec<String>, String> {
    Err("This command is only available on Windows".to_string())
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
