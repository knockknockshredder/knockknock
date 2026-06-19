// src-tauri/src/shredder/validation.rs

use crate::shredder::errors::ShredError;
use crate::shredder::types::HardLinkInfo;
use std::path::{Path, PathBuf};

const SYSTEM_PATHS: &[&str] = &[
    "C:\\Windows",
    "/System",
    "/usr",
    "/etc",
    "/bin",
    "/sbin",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
];

pub fn validate_path(path: &Path) -> Result<(), ShredError> {
    if path.as_os_str().is_empty() {
        return Err(ShredError::EmptyPath);
    }

    // Use symlink_metadata to NOT follow symlinks
    let metadata = std::fs::symlink_metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ShredError::FileNotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => ShredError::PermissionDenied(path.to_path_buf()),
        _ => ShredError::from_io_error(path.to_path_buf(), e),
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ShredError::SymlinkDetected(path.to_path_buf()));
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

    if !metadata.file_type().is_file() && !metadata.file_type().is_dir() {
        return Err(ShredError::InvalidPathType(canonical));
    }

    Ok(())
}

pub fn check_hard_links(path: &Path) -> Result<HardLinkInfo, ShredError> {
    let metadata =
        std::fs::metadata(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

    #[cfg(unix)]
    let link_count = {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() as u32
    };

    #[cfg(windows)]
    let link_count = 1u32; // TODO: GetFileInformationByHandle

    Ok(HardLinkInfo {
        path: path.to_path_buf(),
        link_count,
        warning: if link_count > 1 {
            Some(format!("File has {} hard links.", link_count))
        } else {
            None
        },
    })
}

pub fn is_network_drive(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        s.starts_with("\\\\") || s.starts_with("//")
    }
    #[cfg(unix)]
    {
        false
    } // TODO: Check /proc/mounts
}
