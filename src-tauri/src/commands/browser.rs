// src-tauri/src/commands/browser.rs

use crate::browser;
use crate::browser::types::*;
use crate::shredder::types::ShredReport;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn detect_browsers() -> Result<Vec<DetectedBrowser>, String> {
    eprintln!("[detect_browsers command] called");
    let result = tokio::task::spawn_blocking(|| browser::detection::detect_browsers())
        .await
        .map_err(|e| format!("Detection panicked: {:?}", e))?;
    eprintln!(
        "[detect_browsers command] returning {} browsers",
        result.len()
    );
    Ok(result)
}

#[tauri::command]
pub async fn shred_browser_data(
    app: AppHandle,
    request: BrowserShredRequest,
) -> Result<ShredReport, String> {
    use crate::shredder::algorithms::all_algorithms;
    use crate::shredder::logging::LogObfuscation;
    use crate::shredder::progress::TauriProgressReporter;

    eprintln!(
        "[shred_browser_data] called for browser={}, profile={}",
        request.browser_name, request.profile_path
    );

    let profile_path = PathBuf::from(&request.profile_path);
    if !profile_path.exists() {
        return Err(format!(
            "Profile path does not exist: {}",
            request.profile_path
        ));
    }

    // Safety: refuse to shred browser data while the browser is running
    // unless the user has explicitly acknowledged the warning.
    if check_browser_lock_file(&profile_path) && !request.explicit_consent {
        return Err(format!(
            "Browser {} is currently running. Close it first or acknowledge the warning.",
            request.browser_name
        ));
    }

    // Collect files to shred based on selected data types. Collection
    // failures are surfaced, never silently swallowed (M9).
    let mut files_to_shred = Vec::new();
    for data_type in &request.data_types {
        collect_browser_data_files(&profile_path, data_type, &mut files_to_shred)
            .map_err(|error| error.to_string())?;
    }

    if files_to_shred.is_empty() {
        return Err("No browser data files found to shred".to_string());
    }

    eprintln!(
        "[shred_browser_data] found {} files to shred",
        files_to_shred.len()
    );

    // Use the SAME algorithm settings as file shredding
    let algorithms = all_algorithms();
    let algorithm = algorithms
        .get(request.algorithm_index)
        .ok_or_else(|| format!("Invalid algorithm index: {}", request.algorithm_index))?
        .clone();

    if request.passes > algorithm.max_passes() {
        return Err(format!(
            "Passes {} exceeds maximum {}",
            request.passes,
            algorithm.max_passes()
        ));
    }

    let progress: Arc<dyn crate::shredder::traits::ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app, LogObfuscation::None));

    let report = tokio::task::spawn_blocking(move || {
        // Browser profile data is plain files inside the profile directory;
        // shortcuts are not expected there and we never want to follow them
        // (a stray .lnk in a profile should not drag in user files outside it).
        crate::shredder::shred_files(
            files_to_shred,
            algorithm,
            request.passes,
            request.pattern,
            request.verification_level,
            progress,
        )
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    eprintln!(
        "[shred_browser_data] complete: {} successful, {} failed",
        report.successful, report.failed
    );

    Ok(report)
}

/// Collect files for a specific browser data type into `files`.
///
/// Returns `Err(BrowserCollectionFailed)` on any inspection failure — an
/// unreadable profile must never silently become a "successful" cleanup
/// (M9). Filesystem links are never followed anywhere in collection.
fn collect_browser_data_files(
    profile_path: &std::path::Path,
    data_type: &BrowserDataType,
    files: &mut Vec<PathBuf>,
) -> Result<(), crate::shredder::ShredError> {
    match data_type {
        BrowserDataType::Cache => {
            let cache_dirs = ["Cache", "cache2", "Code Cache", "GPUCache", "OfflineCache"];
            for dir in &cache_dirs {
                let cache_path = profile_path.join(dir);
                if is_dir_nofollow(&cache_path) {
                    collect_files_recursive_nofollow(&cache_path, files)?;
                }
            }
        }
        BrowserDataType::Cookies => {
            let cookie_files = [
                "Cookies",
                "cookies.sqlite",
                "cookies.txt",
                "Network/Cookies",
                "Cookies-journal",
            ];
            for name in &cookie_files {
                let path = profile_path.join(name);
                if is_regular_file_nofollow(&path) {
                    files.push(path);
                }
            }
        }
        BrowserDataType::History => {
            let history_files = [
                "History",
                "History-journal",
                "places.sqlite",
                "places.sqlite-wal",
                "places.sqlite-shm",
                "Favicons",
            ];
            for name in &history_files {
                let path = profile_path.join(name);
                if is_regular_file_nofollow(&path) {
                    files.push(path);
                }
            }
        }
        BrowserDataType::Passwords => {
            let password_files = [
                "Login Data",
                "Login Data-journal",
                "logins.json",
                "signons.sqlite",
                "key4.db",
            ];
            for name in &password_files {
                let path = profile_path.join(name);
                if is_regular_file_nofollow(&path) {
                    files.push(path);
                }
            }
        }
        BrowserDataType::Extensions => {
            let ext_path = profile_path.join("Extensions");
            if is_dir_nofollow(&ext_path) {
                collect_files_recursive_nofollow(&ext_path, files)?;
            }
        }
        BrowserDataType::Profile => {
            // Shred entire profile (except Extensions which are re-downloadable)
            collect_files_recursive_excluding_nofollow(profile_path, files, &["Extensions"])?;
        }
    }

    Ok(())
}

/// Non-followed existence check for directory starts: the path exists,
/// is a real directory, and is not a filesystem link.
fn is_dir_nofollow(path: &std::path::Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.is_dir() && !browser::paths::is_link_metadata(&metadata),
        Err(_) => false,
    }
}

/// Non-followed regular-file check for the fixed file-name patterns
/// (Cookies/History/Passwords). A symlink pointing at a real file is NOT a
/// collection candidate (M9).
fn is_regular_file_nofollow(path: &std::path::Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_file() && !browser::paths::is_link_metadata(&metadata),
        Err(_) => false,
    }
}

/// Recursively collect regular files from a directory without following any
/// filesystem link (M9).
///
/// Rules:
/// - `read_dir` failure, `DirEntry` error, or metadata inspection failure →
///   `Err(BrowserCollectionFailed)` — never a silent skip;
/// - Unix symlink / Windows reparse point (junction) → skipped, never
///   recursed into;
/// - normal nested directory → recursed into;
/// - regular file → collected.
/// A root that does not exist yields an empty collection (callers gate the
/// root with `is_dir_nofollow`).
fn collect_files_recursive_nofollow(
    dir: &std::path::Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), crate::shredder::ShredError> {
    let root_metadata = match std::fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(collection_error(dir, &error));
        }
    };
    if browser::paths::is_link_metadata(&root_metadata) {
        return Ok(());
    }
    if !root_metadata.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|error| collection_error(dir, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| collection_error(dir, &error))?;
        let path = entry.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            // Entry vanished between enumeration and inspection — a benign
            // race with the browser; treat it as absent.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(collection_error(&path, &error)),
        };
        if browser::paths::is_link_metadata(&metadata) {
            continue;
        }
        if metadata.file_type().is_file() {
            files.push(path);
        } else if metadata.is_dir() {
            collect_files_recursive_nofollow(&path, files)?;
        }
    }

    Ok(())
}

/// Recursively collect files excluding specified directories, with the same
/// no-follow and fail-loud rules as `collect_files_recursive_nofollow`.
fn collect_files_recursive_excluding_nofollow(
    dir: &std::path::Path,
    files: &mut Vec<PathBuf>,
    exclude_dirs: &[&str],
) -> Result<(), crate::shredder::ShredError> {
    let root_metadata = match std::fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(collection_error(dir, &error));
        }
    };
    if browser::paths::is_link_metadata(&root_metadata) {
        return Ok(());
    }
    if !root_metadata.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|error| collection_error(dir, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| collection_error(dir, &error))?;
        let path = entry.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(collection_error(&path, &error)),
        };
        if browser::paths::is_link_metadata(&metadata) {
            continue;
        }
        if metadata.file_type().is_file() {
            files.push(path);
        } else if metadata.is_dir() {
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !exclude_dirs.contains(&dir_name.as_str()) {
                collect_files_recursive_excluding_nofollow(&path, files, exclude_dirs)?;
            }
        }
    }

    Ok(())
}

fn collection_error(
    path: &std::path::Path,
    error: &std::io::Error,
) -> crate::shredder::ShredError {
    crate::shredder::ShredError::BrowserCollectionFailed {
        path: path.to_path_buf(),
        detail: error.to_string(),
    }
}

/// Detect if a browser is running by looking for lock files in the profile directory.
/// Chromium-based browsers create `SingletonLock` (or `lock`) while running;
/// Firefox creates `.parentlock`. Returns true if any lock file is present.
fn check_browser_lock_file(profile_path: &std::path::Path) -> bool {
    const LOCK_FILES: &[&str] = &["SingletonLock", "lock", ".parentlock"];
    for lock_name in LOCK_FILES {
        if profile_path.join(lock_name).exists() {
            return true;
        }
    }
    if let Some(parent) = profile_path.parent() {
        if parent.join("SingletonLock").exists() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Collects the given data types from `profile_path` into a sorted list.
    fn collect(profile_path: &Path, data_types: &[BrowserDataType]) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        for data_type in data_types {
            collect_browser_data_files(profile_path, data_type, &mut files)
                .map_err(|error| error.to_string())?;
        }
        files.sort();
        Ok(files)
    }

    /// A fake browser profile on disk. `link_dir` (when created) points at
    /// `outside`, which is intentionally NOT part of the profile.
    fn profile_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("temporary directory");
        let profile = tmp.path().join("profile");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&profile).expect("create profile");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        (tmp, profile, outside)
    }

    fn write(path: &Path, content: &[u8]) {
        std::fs::write(path, content).expect("write fixture");
    }

    #[test]
    fn collector_processes_nested_directories_and_skips_extensions() {
        let (tmp, profile, _outside) = profile_fixture();
        write(&profile.join("real.txt"), b"real");
        std::fs::create_dir_all(profile.join("sub/deeper")).expect("create nested dirs");
        write(&profile.join("sub/nested.txt"), b"nested");
        write(&profile.join("sub/deeper/deep.txt"), b"deep");
        std::fs::create_dir(profile.join("Extensions")).expect("create extensions dir");
        write(&profile.join("Extensions/extension.txt"), b"extension");

        let files = collect(&profile, &[BrowserDataType::Profile]).expect("collect");

        let expected = [
            profile.join("real.txt"),
            profile.join("sub/deeper/deep.txt"),
            profile.join("sub/nested.txt"),
        ];
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(files, expected);
        drop(tmp);
    }

    /// M9: a symlink directory inside the profile must not be traversed, and
    /// its target content must not be collected.
    #[cfg(unix)]
    #[test]
    fn collector_never_escapes_profile_through_link_dirs() {
        let (tmp, profile, outside) = profile_fixture();
        write(&profile.join("real.txt"), b"real");
        write(&outside.join("secret.txt"), b"secret payload");
        std::os::unix::fs::symlink(&outside, profile.join("link_dir")).expect("create symlink");

        let files = collect(&profile, &[BrowserDataType::Profile]).expect("collect");

        assert_eq!(files, vec![profile.join("real.txt")]);
        assert!(
            profile.join("link_dir").symlink_metadata().is_ok(),
            "the link itself must remain"
        );
        assert_eq!(
            std::fs::read(&outside.join("secret.txt")).expect("outside readable"),
            b"secret payload",
            "the link target must be untouched"
        );
        drop(tmp);
    }

    /// M9: an unreadable directory inside the profile surfaces as a
    /// `BrowserCollectionFailed` error — never a silent skip.
    #[cfg(unix)]
    #[test]
    fn collector_surfaces_inspection_errors_instead_of_skipping() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, profile, _outside) = profile_fixture();
        write(&profile.join("readable.txt"), b"readable");
        let locked = profile.join("locked");
        std::fs::create_dir(&locked).expect("create locked dir");
        write(&locked.join("inside.txt"), b"inside");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
            .expect("chmod 0");

        let error = collect(&profile, &[BrowserDataType::Profile]).expect_err("must fail");
        assert!(
            error.contains("browser data collection failed"),
            "unexpected error: {error}"
        );

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
            .expect("restore permissions");
        drop(tmp);
    }

    /// M9: fixed file-name patterns (Cookies/History/Passwords) require a
    /// non-followed regular file — a symlink pointing at a real file is not
    /// a candidate.
    #[cfg(unix)]
    #[test]
    fn collector_requires_non_link_regular_files_for_pattern_names() {
        let (tmp, profile, outside) = profile_fixture();
        write(&profile.join("Cookies"), b"real cookies");
        std::fs::create_dir_all(profile.join("Network")).expect("create network dir");
        write(&profile.join("Network/Cookies"), b"network cookies");
        write(&outside.join("secret.txt"), b"secret payload");
        std::os::unix::fs::symlink(&outside.join("secret.txt"), profile.join("cookies.txt"))
            .expect("create file symlink");

        let files = collect(&profile, &[BrowserDataType::Cookies]).expect("collect");

        let mut expected = vec![
            profile.join("Cookies"),
            profile.join("Network/Cookies"),
        ];
        expected.sort();
        assert_eq!(files, expected, "the cookies.txt symlink must not be collected");
        assert_eq!(
            std::fs::read(&outside.join("secret.txt")).expect("outside readable"),
            b"secret payload",
            "the link target must be untouched"
        );
        drop(tmp);
    }

    /// M9: profile discovery must not accept symlink/reparse profile
    /// directories.
    #[cfg(unix)]
    #[test]
    fn find_browser_profiles_skips_link_profiles() {
        use crate::browser::paths::find_browser_profiles;

        let tmp = tempfile::tempdir().expect("temporary directory");
        let base = tmp.path().join("user-data");
        std::fs::create_dir_all(base.join("Default")).expect("create default profile");
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("create elsewhere dir");
        std::os::unix::fs::symlink(&elsewhere, base.join("Profile 1")).expect("create symlink");

        let profiles = find_browser_profiles(&base, "Default");

        assert_eq!(profiles, vec![base.join("Default")]);
        drop(tmp);
    }
}
