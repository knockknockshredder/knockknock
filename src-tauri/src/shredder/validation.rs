// src-tauri/src/shredder/validation.rs

use crate::shredder::errors::ShredError;
use crate::shredder::types::HardLinkInfo;
use std::path::{Path, PathBuf};

const SYSTEM_PATHS: &[&str] = &[
    // Windows
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\ProgramData",
    // macOS
    "/System",
    "/System/Library",
    "/Applications",
    "/Library",
    // Linux
    "/usr",
    "/etc",
    "/bin",
    "/sbin",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/lib",
    "/lib64",
    "/opt",
    "/var",
];

/// Result of classifying a path before shredding.
///
/// `Normal` covers regular files and directories (including broken or malformed
/// `.lnk` files — see `classify_path`). `Shortcut` covers any link type whose
/// target would be exposed if the link were shredded in place: Unix symlinks,
/// Windows NTFS symlinks/junctions, and Windows `.lnk` shell shortcuts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathClassification {
    Normal,
    Shortcut { target: PathBuf },
}

pub fn validate_path(path: &Path, allow_shortcut: bool) -> Result<(), ShredError> {
    if path.as_os_str().is_empty() {
        return Err(ShredError::EmptyPath);
    }

    let classification = classify_path(path)?;
    match classification {
        PathClassification::Normal => validate_path_inner(path),
        PathClassification::Shortcut { target } => {
            if allow_shortcut {
                Ok(())
            } else {
                Err(ShredError::ShortcutDetected {
                    path: path.to_path_buf(),
                    target: target.to_string_lossy().to_string(),
                })
            }
        }
    }
}

/// Classify a path as either a regular file/directory or a shortcut/symlink.
///
/// A shortcut is any of:
/// - Unix symlink (`ln -s`)
/// - Windows NTFS symlink (`mklink`)
/// - Windows directory junction (`mklink /J`)
/// - Windows `.lnk` shell shortcut
///
/// Broken or malformed `.lnk` files are classified as `Normal` with a warning
/// logged to stderr — the user's intent (shred this file) is preserved.
pub fn classify_path(path: &Path) -> Result<PathClassification, ShredError> {
    // 1. Check if symlink (Unix symlinks, Windows NTFS symlinks, junctions).
    let sym_meta = std::fs::symlink_metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ShredError::FileNotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => ShredError::PermissionDenied(path.to_path_buf()),
        _ => ShredError::from_io_error(path.to_path_buf(), e),
    })?;

    if sym_meta.file_type().is_symlink() {
        let target = std::fs::read_link(path)
            .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;
        return Ok(PathClassification::Shortcut { target });
    }

    // 2. Check if Windows `.lnk` (separate — not a reparse point).
    #[cfg(windows)]
    {
        if is_windows_lnk(path) {
            match lnks::Shortcut::load(path) {
                Ok(shortcut) => {
                    if let Some(target) = shortcut.target_path {
                        return Ok(PathClassification::Shortcut { target });
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[KnockKnock] Warning: Failed to parse .lnk {:?}: {}",
                        path, e
                    );
                    // Fall through — treat as normal file.
                }
            }
        }
    }

    Ok(PathClassification::Normal)
}

#[cfg(windows)]
fn is_windows_lnk(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false)
}

fn validate_path_inner(path: &Path) -> Result<(), ShredError> {
    if path.as_os_str().is_empty() {
        return Err(ShredError::EmptyPath);
    }

    // Reject network drives BEFORE canonicalization. Canonicalize on Windows
    // prepends `\\?\` to UNC paths (e.g. `\\?\UNC\server\share`), so checking
    // the original input here ensures we never accidentally canonicalize a
    // network path before refusing it.
    if is_network_drive(path) {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }

    // Use symlink_metadata to NOT follow symlinks. The shortcut/symlink check
    // has already been handled by `classify_path`, so any symlink that reaches
    // here indicates a TOCTOU race — fail closed as InvalidPathType.
    let metadata = std::fs::symlink_metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ShredError::FileNotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => ShredError::PermissionDenied(path.to_path_buf()),
        _ => ShredError::from_io_error(path.to_path_buf(), e),
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ShredError::InvalidPathType(path.to_path_buf()));
    }

    let canonical = std::fs::canonicalize(path)
        .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

    // Strip UNC prefix on Windows for system path comparison
    let canonical_str = canonical.to_string_lossy();
    let canonical_normalized = canonical_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&canonical_str);

    #[cfg(windows)]
    let canonical_lower = canonical_normalized.to_lowercase();
    #[cfg(not(windows))]
    let canonical_lower = canonical_normalized.to_string();

    for sys_path in SYSTEM_PATHS {
        #[cfg(windows)]
        let matches = canonical_lower.starts_with(&sys_path.to_lowercase());
        #[cfg(not(windows))]
        let matches = canonical_lower.starts_with(sys_path);

        if matches {
            return Err(ShredError::SystemFile(canonical));
        }
    }

    // Protect app's own binary
    if let Ok(app_exe) = std::env::current_exe() {
        if let Ok(app_dir) = app_exe.parent().map(|p| p.canonicalize()).transpose() {
            if let Some(app_dir) = app_dir {
                if canonical.starts_with(&app_dir) {
                    return Err(ShredError::SystemFile(canonical));
                }
            }
        }
    }

    if !metadata.file_type().is_file() && !metadata.file_type().is_dir() {
        return Err(ShredError::InvalidPathType(canonical));
    }

    Ok(())
}

pub fn check_hard_links(path: &Path) -> Result<HardLinkInfo, ShredError> {
    #[cfg(unix)]
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

    #[cfg(unix)]
    let link_count = {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() as u32
    };

    #[cfg(windows)]
    let link_count = {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
        use windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION;

        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x00000001) // FILE_SHARE_READ
            .open(path);

        match file {
            Ok(f) => {
                let handle = f.as_raw_handle() as *mut _;
                let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                let result = unsafe { GetFileInformationByHandle(handle, &mut info) };
                if result != 0 {
                    info.nNumberOfLinks
                } else {
                    eprintln!(
                        "[KnockKnock] Warning: GetFileInformationByHandle failed for {:?}; assuming single link.",
                        path
                    );
                    1
                }
            }
            Err(e) => {
                eprintln!(
                    "[KnockKnock] Warning: Could not check hard links for {:?}: {}. Assuming single link.",
                    path, e
                );
                1
            }
        }
    };

    Ok(HardLinkInfo { link_count })
}

/// Windows mapped-drive (DRIVE_REMOTE) check via `GetDriveTypeW`.
///
/// Drive letters like `Z:` mounted as `net use Z: \\server\share` are NOT
/// detected by path-prefix checks against `\\` — they look like ordinary
/// drive-letter paths until the OS is consulted. `GetDriveTypeW` returns
/// `DRIVE_REMOTE (4)` for these so we can refuse them before shredding.
#[cfg(windows)]
fn is_windows_remote_drive(path: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    let path_str = path.to_string_lossy();
    let drive = if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
        format!("{}:\\", &path_str[..1])
    } else {
        return false;
    };

    let drive_wide: Vec<u16> = drive.encode_utf16().chain(std::iter::once(0)).collect();
    let drive_type = unsafe { GetDriveTypeW(drive_wide.as_ptr()) };

    // DRIVE_REMOTE = 4
    drive_type == 4
}

pub fn is_network_drive(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        // Regular UNC: \\server\share or //server/share
        // Extended-length UNC: \\?\UNC\server\share. canonicalize() prepends
        // this prefix on Windows; callers must catch it before stripping.
        if s.starts_with("\\\\") || s.starts_with("//") || s.starts_with(r"\\?\UNC\") {
            return true;
        }
        // Also check mapped drives via GetDriveTypeW (e.g., `net use Z:` shares)
        if is_windows_remote_drive(path) {
            return true;
        }
        false
    }
    #[cfg(unix)]
    {
        let mounts = match std::fs::read_to_string("/proc/mounts") {
            Ok(m) => m,
            Err(_) => return false,
        };

        let path_str = path.to_string_lossy();
        let mut best_match = "";
        let mut best_fs_type = "";

        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let mount_point = parts[1];
                let fs_type = parts[2];
                if path_str.starts_with(mount_point) && mount_point.len() > best_match.len() {
                    best_match = mount_point;
                    best_fs_type = fs_type;
                }
            }
        }

        matches!(
            best_fs_type,
            "nfs" | "nfs4" | "cifs" | "smbfs" | "sshfs" | "afs" | "ncp" | "ncpfs"
        )
    }
}
