// src-tauri/src/paths.rs
//
// Centralized portable path resolver. All app data lives in a
// "KnockKnock-data" folder next to the executable — never in
// OS-managed config/data directories.
//
// No fallback. If the exe directory is unwritable, the caller
// receives an Err with a user-facing message.

use std::path::PathBuf;

/// Directory containing the app executable on Windows/Linux, or the
/// .app bundle on macOS.
///
/// - Windows/Linux:  parent of the exe  (knockknock.exe  → <folder>/)
/// - macOS:          parent of .app      (…/MacOS/knockknock → 4 levels up → <folder>/)
fn app_root_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate executable: {e}"))?;

    let exe_dir = exe.parent().ok_or("Executable has no parent directory")?;

    #[cfg(target_os = "macos")]
    {
        // Binary is: <app_root>/KnockKnock.app/Contents/MacOS/knockknock
        // Walk up 3 more levels:
        //   MacOS/   → Contents/
        //   Contents → KnockKnock.app/
        //   .app/    → app_root/
        let root = exe_dir
            .parent() // Contents/
            .and_then(|p| p.parent()) // KnockKnock.app/
            .and_then(|p| p.parent()) // <app_root>/
            .ok_or("Cannot resolve .app bundle root")?;
        Ok(root.to_path_buf())
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Linux: if running as AppImage, current_exe() points to
        // /tmp/.mount_*/usr/bin/knockknock — ephemeral. Prefer
        // APPIMAGE env var to find the actual file location.
        #[cfg(target_os = "linux")]
        {
            if let Some(appimage) = std::env::var_os("APPIMAGE") {
                let p = PathBuf::from(appimage);
                if let Some(parent) = p.parent() {
                    return Ok(parent.to_path_buf());
                }
            }

            // APPIMAGE not set — guard against the /tmp/.mount_*
            // case (manually extracted AppImage, custom wrapper, CI).
            // Data written here is destroyed on app exit.
            let path_str = exe_dir.to_string_lossy();
            if path_str.starts_with("/tmp/.mount_") {
                return Err(format!(
                    "AppImage is not running from a mounted location.\n\
                     Set APPIMAGE=/path/to/file.AppImage and relaunch."
                ));
            }
        }
        Ok(exe_dir.to_path_buf())
    }
}

/// `KnockKnock-data/` next to the app — the single root for all
/// persisted state (PIN, vault, journal, settings, WebView data).
///
/// Creates the directory on first call.
///
/// # Errors
/// Returns a user-facing message if the directory cannot be created
/// (e.g. app placed in a read-only location).
pub fn portable_data_dir() -> Result<PathBuf, String> {
    let root = app_root_dir()?;
    let data_dir = root.join("KnockKnock-data");

    std::fs::create_dir_all(&data_dir).map_err(|e| {
        format!(
            "KnockKnock is portable — it must be placed in a writable folder.\n\
             Current location: {}\nError: {e}",
            root.display()
        )
    })?;

    Ok(data_dir)
}
