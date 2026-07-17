// src-tauri/src/commands/error.rs

use crate::shredder::errors::ShredError;
use serde::Serialize;

/// IPC-safe error payload returned to the React frontend.
///
/// `path` is preserved as-is for now; obfuscation will be applied in Task 19.
#[derive(Debug, Serialize)]
pub struct ShredErrorDto {
    /// Stable machine-readable error category (e.g. "FileNotFound").
    pub error_type: String,
    /// Human-readable error description (from `ShredError::Display`).
    pub message: String,
    /// Optional file path associated with the error.
    pub path: Option<String>,
    /// User-facing remediation guidance.
    pub actionable: String,
}

impl From<&ShredError> for ShredErrorDto {
    fn from(err: &ShredError) -> Self {
        let message = err.to_string();
        let (path, actionable) = extract_path_and_action(err);

        ShredErrorDto {
            error_type: error_type_name(err),
            message,
            path,
            actionable,
        }
    }
}

/// Map each `ShredError` variant to user-actionable guidance, and recover
/// the associated path where one is present.
fn extract_path_and_action(err: &ShredError) -> (Option<String>, String) {
    match err {
        ShredError::FileNotFound(p) => (
            Some(p.to_string_lossy().into_owned()),
            "Verify the file exists and is accessible. It may have been moved or deleted.".to_string(),
        ),
        ShredError::PermissionDenied(p) => (
            Some(p.to_string_lossy().into_owned()),
            "Run KnockKnock as administrator, or check the file's ownership and permissions.".to_string(),
        ),
        ShredError::FileLocked { path, .. } => (
            Some(path.to_string_lossy().into_owned()),
            "Close the application using this file, then retry. On Windows, schedule deletion at next reboot if needed.".to_string(),
        ),
        ShredError::IoError { path, kind, message } => {
            // Surface the OS error kind + message as actionable context for advanced users.
            let detail = format!("OS reported {}: {}", kind, message);
            (
                Some(path.to_string_lossy().into_owned()),
                format!("Check disk health, free space, and file integrity. ({})", detail),
            )
        }
        ShredError::VerificationFailed { path, .. } => (
            Some(path.to_string_lossy().into_owned()),
            "Overwrite verification failed. Retry the operation; if it persists, the disk may be failing.".to_string(),
        ),
        ShredError::NetworkDrive(p) => (
            Some(p.to_string_lossy().into_owned()),
            "Copy the file to a local drive and retry. Network drives cannot be reliably shredded.".to_string(),
        ),
        ShredError::SystemFile(p) => (
            Some(p.to_string_lossy().into_owned()),
            "This is a protected system file. Do not shred it — this operation has been refused for your safety.".to_string(),
        ),
        ShredError::SymlinkDetected(p) => (
            Some(p.to_string_lossy().into_owned()),
            "Symlinks are refused to prevent shredding unintended targets. Resolve the link and retry with the real file.".to_string(),
        ),
        ShredError::InvalidPathType(p) => (
            Some(p.to_string_lossy().into_owned()),
            "Select a regular file or directory. Special filesystem entries (sockets, pipes, devices) are not supported.".to_string(),
        ),
        ShredError::EmptyPath => (
            None,
            "Provide a valid file or directory path.".to_string(),
        ),
    }
}

/// Return a stable, machine-readable category name for the error variant.
fn error_type_name(err: &ShredError) -> String {
    match err {
        ShredError::FileNotFound(_) => "FileNotFound",
        ShredError::PermissionDenied(_) => "PermissionDenied",
        ShredError::FileLocked { .. } => "FileLocked",
        ShredError::IoError { .. } => "IoError",
        ShredError::VerificationFailed { .. } => "VerificationFailed",
        ShredError::NetworkDrive(_) => "NetworkDrive",
        ShredError::SystemFile(_) => "SystemFile",
        ShredError::SymlinkDetected(_) => "SymlinkDetected",
        ShredError::InvalidPathType(_) => "InvalidPathType",
        ShredError::EmptyPath => "EmptyPath",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn dto_from_file_not_found() {
        let err = ShredError::FileNotFound(PathBuf::from("/tmp/missing.txt"));
        let dto: ShredErrorDto = (&err).into();
        assert_eq!(dto.error_type, "FileNotFound");
        assert!(dto.path.is_some());
        assert!(!dto.actionable.is_empty());
    }

    #[test]
    fn dto_from_empty_path_has_no_path() {
        let err = ShredError::EmptyPath;
        let dto: ShredErrorDto = (&err).into();
        assert_eq!(dto.error_type, "EmptyPath");
        assert!(dto.path.is_none());
        assert!(!dto.actionable.is_empty());
    }

    #[test]
    fn dto_serializes_to_json() {
        let err = ShredError::PermissionDenied(PathBuf::from("C:\\secret.txt"));
        let dto: ShredErrorDto = (&err).into();
        let json = serde_json::to_string(&dto).expect("DTO should serialize");
        assert!(json.contains("\"error_type\":\"PermissionDenied\""));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"actionable\""));
        assert!(json.contains("\"path\""));
    }

    #[test]
    fn every_variant_has_an_actionable_message() {
        let cases = vec![
            ShredError::FileNotFound(PathBuf::from("a")),
            ShredError::PermissionDenied(PathBuf::from("a")),
            ShredError::FileLocked {
                path: PathBuf::from("a"),
                process: "x".into(),
            },
            ShredError::IoError {
                path: PathBuf::from("a"),
                kind: "Other".into(),
                message: "boom".into(),
            },
            ShredError::VerificationFailed {
                path: PathBuf::from("a"),
                pass: 1,
            },
            ShredError::NetworkDrive(PathBuf::from("a")),
            ShredError::SystemFile(PathBuf::from("a")),
            ShredError::SymlinkDetected(PathBuf::from("a")),
            ShredError::InvalidPathType(PathBuf::from("a")),
            ShredError::EmptyPath,
        ];
        for err in cases {
            let dto: ShredErrorDto = (&err).into();
            assert!(
                !dto.actionable.is_empty(),
                "Missing actionable message for variant producing error_type={}",
                dto.error_type
            );
        }
    }
}
