// src-tauri/src/shredder/validation.rs

use crate::shredder::errors::ShredError;
use crate::shredder::types::HardLinkInfo;
use std::path::Path;

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
    let metadata =
        std::fs::metadata(path).map_err(|e| ShredError::from_io_error(path.to_path_buf(), e))?;

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
                    1
                }
            }
            Err(_) => 1,
        }
    };

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
