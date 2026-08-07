// src-tauri/src/shredder/types.rs

use crate::shredder::errors::ShredError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use crate::shredder::root_execution::types::{
    BatchRootResult, ChildErrorDto, ExecuteRootRequest, ExecuteRootsRequest, ExecutionStage,
    RootResultDto, RootStatus, TargetAvailability, TargetKind, TargetMetadataDto, VaultError,
    VaultSchemaSource, VaultTarget,
};

/// Byte patterns for overwriting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    Random,
    Zeros,
    Ones,
}

impl PatternType {
    // PatternType is used for serialization/deserialization
}

/// Media type for SSD-aware shredding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Ssd,
    Hdd,
    Unknown,
}

/// Status of a shredding operation
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ShredStatus {
    Shredding,
    Complete,
    Warning { message: String },
    Error { message: String },
}

/// Result of a single file shredding operation
#[derive(Debug)]
pub struct ShredResult {
    pub success: bool,
    pub passes_completed: u32,
    pub bytes_written: u64,
    pub errors: Vec<ShredError>,
}

/// Result of verification
#[derive(Debug)]
pub struct VerificationResult {
    pub passed: bool,
}

/// Information about a hard link
#[derive(Debug)]
pub struct HardLinkInfo {
    pub link_count: u32,
}

/// Information about a process holding a file lock
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Summary report after batch shredding
#[derive(Debug, Serialize)]
pub struct ShredReport {
    pub total_files: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: Vec<ShredReportError>,
    pub total_bytes_shredded: u64,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct ShredReportError {
    pub path: String,
    pub error: String,
}

/// Progress event sent to frontend
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub file_path: String,
    pub file_size: u64,
    pub bytes_written: u64,
    pub current_pass: u32,
    pub total_passes: u32,
    pub speed_bytes_per_sec: u64,
    pub estimated_time_remaining_secs: u64,
    pub status: ShredStatus,
}

/// Verification levels (user-configurable)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationLevel {
    None,
    Sample,
    Full,
}

// ---------------------------------------------------------------------------
// v2 policy model (Phase 1, additive). The legacy VerificationLevel /
// PatternType / ShredResult types above stay live until the Phase 4 cutover.
// ---------------------------------------------------------------------------

/// Deletion method (v2): how many logical overwrite passes a file receives
/// before removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionMethod {
    /// Storage-aware local deletion: one logical random overwrite pass.
    Automatic,
    /// Legacy fixed zeros -> ones -> random sequence. Only permitted on
    /// confirmed magnetic HDD storage (see `validate_storage_for_method`).
    LegacyThreePass,
}

/// Final-state write check mode (v2): read-back verification performed once,
/// after the last overwrite pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteCheck {
    /// No read-back after the overwrite.
    Off,
    /// Deterministic distributed read-back (small files checked in full).
    Spot,
    /// Read-back of the entire final logical file range.
    Full,
}

/// Outcome of the final-state write check (v2 engine + later DTOs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteCheckOutcome {
    NotRun,
    Passed,
    Failed,
}

/// Policy-driven deletion configuration (v2). Replaces the legacy
/// algorithm/passes/pattern/verification-level combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionPolicy {
    pub method: DeletionMethod,
    pub write_check: WriteCheck,
}

impl Default for DeletionPolicy {
    fn default() -> Self {
        Self {
            method: DeletionMethod::Automatic,
            write_check: WriteCheck::Spot,
        }
    }
}

impl DeletionPolicy {
    /// Number of overwrite passes the method performs: Automatic -> 1,
    /// LegacyThreePass -> 3.
    pub fn total_passes(&self) -> u32 {
        match self.method {
            DeletionMethod::Automatic => 1,
            DeletionMethod::LegacyThreePass => 3,
        }
    }

    /// LegacyThreePass is only supported on confirmed magnetic HDD storage.
    pub fn requires_hdd(&self) -> bool {
        self.method == DeletionMethod::LegacyThreePass
    }
}

/// Storage validation rule (v2 preflight, M7): the legacy 3-pass method is
/// only permitted on confirmed HDD media; Automatic has no media restriction.
/// Pure rule — callers (root execution) enforce it before any mutation.
/// Consumed by the root-execution preflight in Phase 3 (Task 3.1); tests
/// exercise it from Phase 1.
#[allow(dead_code)]
pub(crate) fn validate_storage_for_method(
    method: DeletionMethod,
    media: MediaType,
) -> Result<(), ShredError> {
    if method == DeletionMethod::LegacyThreePass && media != MediaType::Hdd {
        return Err(ShredError::UnsupportedStorageForMethod {
            path: PathBuf::new(),
            method,
            media,
        });
    }
    Ok(())
}

/// Metadata returned to the frontend for each path discovered during
/// `validate_paths`.
///
/// `kind` is the same classification the shredder uses: `Directory` for a
/// selected directory root (preserved as one record, never flattened),
/// `Link` for real filesystem links (Unix symlinks, NTFS symlinks, junctions),
/// and `File` for regular files and `.lnk` shell shortcuts (file data).
/// `is_shortcut` flags `.lnk` shell shortcuts, NTFS symlinks, junctions, and
/// Unix symlinks — any path whose target would survive the link's destruction.
/// `shortcut_target` is the resolved target path when classification found one.
#[derive(Debug, Clone, Serialize)]
pub struct FileMetadata {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub kind: TargetKind,
    pub is_shortcut: bool,
    pub shortcut_target: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shredder::types::{
        DeletionMethod, DeletionPolicy, MediaType, WriteCheck, WriteCheckOutcome,
    };

    fn assert_snake_case_round_trip<T>(value: T, expected: &str)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn deletion_method_serializes_snake_case_and_round_trips() {
        assert_snake_case_round_trip(DeletionMethod::Automatic, "automatic");
        assert_snake_case_round_trip(DeletionMethod::LegacyThreePass, "legacy_three_pass");
    }

    #[test]
    fn write_check_serializes_snake_case_and_round_trips() {
        assert_snake_case_round_trip(WriteCheck::Off, "off");
        assert_snake_case_round_trip(WriteCheck::Spot, "spot");
        assert_snake_case_round_trip(WriteCheck::Full, "full");
    }

    #[test]
    fn write_check_outcome_serializes_snake_case_and_round_trips() {
        assert_snake_case_round_trip(WriteCheckOutcome::NotRun, "not_run");
        assert_snake_case_round_trip(WriteCheckOutcome::Passed, "passed");
        assert_snake_case_round_trip(WriteCheckOutcome::Failed, "failed");
    }

    #[test]
    fn deletion_policy_round_trips_with_serde() {
        let policy = DeletionPolicy {
            method: DeletionMethod::LegacyThreePass,
            write_check: WriteCheck::Full,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: DeletionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, policy);
    }

    #[test]
    fn default_policy_is_automatic_with_spot_check() {
        let default = DeletionPolicy::default();
        assert_eq!(
            default,
            DeletionPolicy {
                method: DeletionMethod::Automatic,
                write_check: WriteCheck::Spot,
            }
        );
    }

    #[test]
    fn total_passes_matches_method() {
        let automatic = DeletionPolicy {
            method: DeletionMethod::Automatic,
            write_check: WriteCheck::Off,
        };
        let legacy = DeletionPolicy {
            method: DeletionMethod::LegacyThreePass,
            write_check: WriteCheck::Off,
        };
        assert_eq!(automatic.total_passes(), 1);
        assert_eq!(legacy.total_passes(), 3);
        assert!(!automatic.requires_hdd());
        assert!(legacy.requires_hdd());
    }

    #[test]
    fn validate_storage_for_method_allows_legacy_only_on_hdd() {
        // LegacyThreePass: Hdd allowed.
        assert!(
            validate_storage_for_method(DeletionMethod::LegacyThreePass, MediaType::Hdd).is_ok()
        );

        // LegacyThreePass: every other media type rejected with the
        // structured variant carrying method + media (M7 fail closed).
        for media in [MediaType::Ssd, MediaType::Unknown] {
            match validate_storage_for_method(DeletionMethod::LegacyThreePass, media) {
                Err(ShredError::UnsupportedStorageForMethod {
                    path,
                    method,
                    media: got,
                }) => {
                    assert!(path.as_os_str().is_empty());
                    assert_eq!(method, DeletionMethod::LegacyThreePass);
                    assert_eq!(got, media);
                }
                other => panic!("expected UnsupportedStorageForMethod, got {other:?}"),
            }
        }

        // Automatic: no media restriction.
        for media in [MediaType::Ssd, MediaType::Hdd, MediaType::Unknown] {
            assert!(validate_storage_for_method(DeletionMethod::Automatic, media).is_ok());
        }
    }
}
