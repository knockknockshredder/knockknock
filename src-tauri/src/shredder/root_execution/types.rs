use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    File,
    Directory,
    Link,
    UnknownLegacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetAvailability {
    Ready,
    Missing,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootStatus {
    Destroyed,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStage {
    Preflight,
    Overwrite,
    Verify,
    Rename,
    Truncate,
    Delete,
    DirectoryRemove,
    Journal,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultSchemaSource {
    V1,
    V2,
}

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("I/O error while {action}: {source}")]
    Io {
        action: &'static str,
        source: std::io::Error,
    },
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Unsupported vault schema: {0}")]
    UnsupportedSchema(u32),
    #[error("Failed to replace vault: {source}")]
    Replace { source: std::io::Error },
    #[error("Failed to sync vault path {path}: {source}")]
    Sync {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl From<VaultError> for String {
    fn from(error: VaultError) -> Self {
        error.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultTarget {
    pub path: String,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetMetadataDto {
    pub path: String,
    pub kind: TargetKind,
    pub availability: TargetAvailability,
    pub reason: Option<String>,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteRootRequest {
    pub target_id: String,
    pub path: String,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteRootsRequest {
    pub roots: Vec<ExecuteRootRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildErrorDto {
    pub path: String,
    pub stage: ExecutionStage,
    pub error_type: String,
    pub message: String,
    pub actionable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootResultDto {
    pub target_id: String,
    pub requested_path: String,
    pub kind: TargetKind,
    pub status: RootStatus,
    pub root_removed: bool,
    pub files_destroyed: u64,
    pub directories_removed: u64,
    pub bytes_shredded: u64,
    pub errors: Vec<ChildErrorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRootResult {
    pub roots: Vec<RootResultDto>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::path::PathBuf;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + DeserializeOwned,
    {
        serde_json::from_value(serde_json::to_value(value).expect("serialize"))
            .expect("deserialize")
    }

    #[test]
    fn serializes_every_enum_value_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&TargetKind::File).unwrap(),
            "\"file\""
        );
        assert_eq!(
            serde_json::to_string(&TargetKind::Directory).unwrap(),
            "\"directory\""
        );
        assert_eq!(
            serde_json::to_string(&TargetKind::Link).unwrap(),
            "\"link\""
        );
        assert_eq!(
            serde_json::to_string(&TargetKind::UnknownLegacy).unwrap(),
            "\"unknown_legacy\""
        );

        assert_eq!(
            serde_json::to_string(&TargetAvailability::Ready).unwrap(),
            "\"ready\""
        );
        assert_eq!(
            serde_json::to_string(&TargetAvailability::Missing).unwrap(),
            "\"missing\""
        );
        assert_eq!(
            serde_json::to_string(&TargetAvailability::Blocked).unwrap(),
            "\"blocked\""
        );

        assert_eq!(
            serde_json::to_string(&RootStatus::Destroyed).unwrap(),
            "\"destroyed\""
        );
        assert_eq!(
            serde_json::to_string(&RootStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&RootStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
        assert_eq!(
            serde_json::to_string(&RootStatus::Skipped).unwrap(),
            "\"skipped\""
        );

        for (stage, expected) in [
            (ExecutionStage::Preflight, "preflight"),
            (ExecutionStage::Overwrite, "overwrite"),
            (ExecutionStage::Verify, "verify"),
            (ExecutionStage::Rename, "rename"),
            (ExecutionStage::Truncate, "truncate"),
            (ExecutionStage::Delete, "delete"),
            (ExecutionStage::DirectoryRemove, "directory_remove"),
            (ExecutionStage::Journal, "journal"),
            (ExecutionStage::Sync, "sync"),
        ] {
            assert_eq!(
                serde_json::to_string(&stage).unwrap(),
                format!("\"{expected}\"")
            );
        }

        assert_eq!(
            serde_json::to_string(&VaultSchemaSource::V1).unwrap(),
            "\"v1\""
        );
        assert_eq!(
            serde_json::to_string(&VaultSchemaSource::V2).unwrap(),
            "\"v2\""
        );
    }

    #[test]
    fn rejects_unknown_enum_values() {
        assert!(serde_json::from_str::<TargetKind>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<TargetAvailability>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<RootStatus>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<ExecutionStage>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<VaultSchemaSource>("\"v3\"").is_err());
    }

    #[test]
    fn round_trips_each_dto() {
        let target = VaultTarget {
            path: "C:\\selected\\root".to_string(),
            kind: TargetKind::Directory,
        };
        assert_eq!(round_trip(&target), target);

        let metadata = TargetMetadataDto {
            path: target.path.clone(),
            kind: target.kind,
            availability: TargetAvailability::Ready,
            reason: None,
            name: "root".to_string(),
            size: 42,
        };
        assert_eq!(round_trip(&metadata), metadata);

        let request = ExecuteRootRequest {
            target_id: "target-1".to_string(),
            path: target.path,
            kind: target.kind,
        };
        assert_eq!(round_trip(&request), request);

        let requests = ExecuteRootsRequest {
            roots: vec![request.clone()],
        };
        assert_eq!(round_trip(&requests), requests);

        let error = ChildErrorDto {
            path: "C:\\selected\\root\\child".to_string(),
            stage: ExecutionStage::Verify,
            error_type: "verification_failed".to_string(),
            message: "verification failed".to_string(),
            actionable: "Retry the operation".to_string(),
        };
        assert_eq!(round_trip(&error), error);

        let result = RootResultDto {
            target_id: "target-1".to_string(),
            requested_path: "C:\\selected\\root".to_string(),
            kind: TargetKind::Directory,
            status: RootStatus::Failed,
            root_removed: false,
            files_destroyed: 1,
            directories_removed: 0,
            bytes_shredded: 42,
            errors: vec![error],
        };
        assert_eq!(round_trip(&result), result);

        let batch = BatchRootResult {
            roots: vec![result],
        };
        assert_eq!(round_trip(&batch), batch);
    }

    #[test]
    fn displays_and_maps_every_vault_error() {
        let errors = [
            VaultError::Io {
                action: "read vault",
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
            },
            VaultError::Crypto("authentication failed".to_string()),
            VaultError::Decode("invalid payload".to_string()),
            VaultError::UnsupportedSchema(3),
            VaultError::Replace {
                source: std::io::Error::new(std::io::ErrorKind::Other, "replace failed"),
            },
            VaultError::Sync {
                path: PathBuf::from("C:\\vault.json"),
                source: std::io::Error::new(std::io::ErrorKind::Other, "sync failed"),
            },
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
            let ipc: String = error.into();
            assert_eq!(ipc, display);
        }
    }
}
