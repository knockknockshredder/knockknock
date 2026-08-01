use super::crypto;
use super::storage::{FaultInjectingVaultIo, VaultIo, VaultIoFailure, VaultPayloadV2, VaultStore};
use crate::commands::shred::validate_targets;
use crate::shredder::root_execution::types::{
    TargetAvailability, TargetKind, VaultError, VaultSchemaSource, VaultTarget,
};
use serde::Serialize;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn store() -> (TempDir, VaultStore) {
    let tempdir = tempfile::tempdir().expect("temporary vault directory");
    let path = tempdir.path().join("vault.json");
    (tempdir, VaultStore::at(path))
}

fn write_encrypted<T: Serialize>(store: &VaultStore, pin: &str, payload: &T) {
    let plaintext = serde_json::to_vec(payload).expect("serialize payload");
    write_encrypted_bytes(store, pin, &plaintext);
}

fn write_encrypted_bytes(store: &VaultStore, pin: &str, plaintext: &[u8]) {
    let encrypted = crypto::encrypt(&plaintext, pin).expect("encrypt payload");
    let file = json!({
        "version": encrypted.version,
        "salt": encrypted.salt,
        "nonce": encrypted.nonce,
        "ciphertext": encrypted.ciphertext,
    });
    fs::write(
        store.path(),
        serde_json::to_vec(&file).expect("serialize vault file"),
    )
    .expect("write vault fixture");
}

fn v2_target(path: impl Into<String>, kind: TargetKind) -> VaultTarget {
    VaultTarget {
        path: path.into(),
        kind,
    }
}

#[test]
fn decodes_v1_strings_as_unknown_legacy_and_requires_migration() {
    let (_tempdir, store) = store();
    let legacy = vec!["legacy-file.txt".to_string(), "legacy-dir".to_string()];
    write_encrypted(&store, "123456", &legacy);

    let loaded = store.load("123456").expect("decode V1 vault");

    assert_eq!(loaded.source_schema, VaultSchemaSource::V1);
    assert!(loaded.migration_required);
    assert_eq!(
        loaded.targets,
        legacy
            .into_iter()
            .map(|path| v2_target(path, TargetKind::UnknownLegacy))
            .collect::<Vec<_>>()
    );
}

#[test]
fn decodes_v2_payload_without_migration() {
    let (_tempdir, store) = store();
    let payload = VaultPayloadV2 {
        schema_version: 2,
        targets: vec![v2_target("known-file.txt", TargetKind::File)],
    };
    write_encrypted(&store, "123456", &payload);
    let before = fs::read(store.path()).expect("read V2 ciphertext before load");

    let loaded = store.load("123456").expect("decode V2 vault");
    let after = fs::read(store.path()).expect("read V2 ciphertext after load");

    assert_eq!(loaded.source_schema, VaultSchemaSource::V2);
    assert!(!loaded.migration_required);
    assert_eq!(loaded.targets, payload.targets);
    assert_eq!(before, after);
}

#[test]
fn wrong_pin_returns_crypto_error_without_writing() {
    let (_tempdir, store) = store();
    write_encrypted(&store, "correct", &vec!["legacy.txt".to_string()]);
    let before = fs::read(store.path()).expect("read ciphertext before load");

    let error = store.load("wrong").expect_err("wrong PIN must fail");
    let after = fs::read(store.path()).expect("read ciphertext after load");

    assert!(matches!(error, VaultError::Crypto(_)));
    assert_eq!(before, after);
}

#[test]
fn corrupt_ciphertext_returns_crypto_error_without_writing() {
    let (_tempdir, store) = store();
    write_encrypted(&store, "123456", &vec!["legacy.txt".to_string()]);
    let mut file: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).expect("read vault fixture"))
            .expect("parse vault fixture");
    file["ciphertext"] = json!([0, 1, 2, 3]);
    fs::write(
        store.path(),
        serde_json::to_vec(&file).expect("serialize corrupt fixture"),
    )
    .expect("write corrupt fixture");
    let before = fs::read(store.path()).expect("read corrupt ciphertext before load");

    let error = store
        .load("123456")
        .expect_err("corrupt ciphertext must fail");
    let after = fs::read(store.path()).expect("read corrupt ciphertext after load");

    assert!(matches!(error, VaultError::Crypto(_)));
    assert_eq!(before, after);
}

#[test]
fn malformed_outer_json_preserves_ciphertext_without_writing() {
    let (_tempdir, store) = store();
    fs::write(store.path(), b"{ malformed outer json").expect("write malformed vault");
    let before = fs::read(store.path()).expect("read malformed vault before load");

    let error = store
        .load("123456")
        .expect_err("malformed outer JSON must fail");
    let after = fs::read(store.path()).expect("read malformed vault after load");

    assert!(matches!(error, VaultError::Decode(_)));
    assert_eq!(before, after);
}

#[test]
fn authenticated_invalid_payload_preserves_ciphertext_without_writing() {
    let (_tempdir, store) = store();
    write_encrypted_bytes(&store, "123456", b"authenticated but not JSON");
    let before = fs::read(store.path()).expect("read invalid payload before load");

    let error = store
        .load("123456")
        .expect_err("authenticated invalid payload must fail");
    let after = fs::read(store.path()).expect("read invalid payload after load");

    assert!(matches!(error, VaultError::Decode(_)));
    assert_eq!(before, after);
}

#[test]
fn unknown_schema_returns_error_without_writing() {
    let (_tempdir, store) = store();
    let payload = json!({ "schema_version": 99, "targets": [] });
    write_encrypted(&store, "123456", &payload);
    let before = fs::read(store.path()).expect("read ciphertext before load");

    let error = store.load("123456").expect_err("unknown schema must fail");
    let after = fs::read(store.path()).expect("read ciphertext after load");

    assert!(matches!(error, VaultError::UnsupportedSchema(99)));
    assert_eq!(before, after);
}

#[test]
fn validates_existing_and_missing_legacy_roots_without_following_links() {
    let (_tempdir, store) = store();
    let existing_file = store.path().with_file_name("existing.txt");
    let existing_dir = store.path().with_file_name("existing-dir");
    let missing = store.path().with_file_name("missing.txt");
    fs::write(&existing_file, b"data").expect("existing file");
    fs::create_dir(&existing_dir).expect("existing directory");

    let legacy = vec![
        existing_file.to_string_lossy().into_owned(),
        existing_dir.to_string_lossy().into_owned(),
        missing.to_string_lossy().into_owned(),
    ];
    write_encrypted(&store, "123456", &legacy);
    let loaded = store.load("123456").expect("decode legacy roots");
    assert_eq!(loaded.source_schema, VaultSchemaSource::V1);
    assert!(loaded.migration_required);

    let metadata = validate_targets(loaded.targets).expect("validate legacy roots");

    assert_eq!(metadata.len(), 3);
    assert_eq!(metadata[0].kind, TargetKind::File);
    assert_eq!(metadata[0].availability, TargetAvailability::Ready);
    assert_eq!(metadata[1].kind, TargetKind::Directory);
    assert_eq!(metadata[1].availability, TargetAvailability::Ready);
    assert_eq!(metadata[2].kind, TargetKind::UnknownLegacy);
    assert_eq!(metadata[2].availability, TargetAvailability::Blocked);
}

#[test]
fn preserves_known_v2_kinds_for_missing_roots() {
    let (_tempdir, store) = store();
    let targets = vec![
        v2_target(
            store
                .path()
                .with_file_name("missing-file")
                .to_string_lossy()
                .into_owned(),
            TargetKind::File,
        ),
        v2_target(
            store
                .path()
                .with_file_name("missing-directory")
                .to_string_lossy()
                .into_owned(),
            TargetKind::Directory,
        ),
        v2_target(
            store
                .path()
                .with_file_name("missing-link")
                .to_string_lossy()
                .into_owned(),
            TargetKind::Link,
        ),
    ];

    let metadata = validate_targets(targets).expect("validate missing V2 roots");

    assert_eq!(metadata.len(), 3);
    assert_eq!(metadata[0].kind, TargetKind::File);
    assert_eq!(metadata[0].availability, TargetAvailability::Missing);
    assert_eq!(metadata[1].kind, TargetKind::Directory);
    assert_eq!(metadata[1].availability, TargetAvailability::Missing);
    assert_eq!(metadata[2].kind, TargetKind::Link);
    assert_eq!(metadata[2].availability, TargetAvailability::Missing);
}

#[cfg(windows)]
#[test]
fn blocks_relative_protected_and_network_roots() {
    let (_tempdir, store) = store();
    let targets = vec![
        v2_target("relative-root", TargetKind::File),
        v2_target(r"C:\Windows", TargetKind::Directory),
        v2_target(r"\\server\share\root", TargetKind::File),
    ];
    let payload = VaultPayloadV2 {
        schema_version: 2,
        targets,
    };
    write_encrypted(&store, "123456", &payload);
    let before = fs::read(store.path()).expect("read V2 ciphertext before validation");
    let loaded = store.load("123456").expect("decode V2 roots");
    let metadata = validate_targets(loaded.targets).expect("validate mismatched roots");
    let after = fs::read(store.path()).expect("read V2 ciphertext after validation");

    assert_eq!(metadata.len(), 3);
    assert_eq!(metadata[0].availability, TargetAvailability::Blocked);
    assert_eq!(metadata[0].kind, TargetKind::File);
    assert_eq!(
        metadata[0].reason.as_deref(),
        Some("Relative paths are not safe execution roots")
    );
    assert_eq!(metadata[1].availability, TargetAvailability::Blocked);
    assert_eq!(metadata[1].kind, TargetKind::Directory);
    assert!(metadata[1]
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("System file protected")));
    assert_eq!(metadata[2].availability, TargetAvailability::Blocked);
    assert_eq!(metadata[2].kind, TargetKind::File);
    assert_eq!(
        metadata[2].reason.as_deref(),
        Some("Network roots are not safe execution roots")
    );
    assert_eq!(before, after);
}

#[test]
fn returns_one_metadata_record_per_input_root() {
    let (_tempdir, _store) = store();
    let targets = vec![
        v2_target("one", TargetKind::File),
        v2_target("two", TargetKind::Directory),
        v2_target("three", TargetKind::Link),
        v2_target("four", TargetKind::UnknownLegacy),
    ];

    let metadata = validate_targets(targets.clone()).expect("validate roots");

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
}

#[test]
fn fault_injecting_vault_io_covers_atomic_write_operations() {
    let (_tempdir, store) = store();
    let temporary_path = store.path().with_extension("tmp");
    let operations = [
        VaultIoFailure::CreateTemp,
        VaultIoFailure::WriteTemp,
        VaultIoFailure::SyncTemp,
        VaultIoFailure::Replace,
        VaultIoFailure::SyncParent,
        VaultIoFailure::CleanupTemp,
    ];

    for operation in operations {
        let adapter = FaultInjectingVaultIo::failing_at(operation);
        let error = match operation {
            VaultIoFailure::CreateTemp => adapter
                .create_temp(&temporary_path)
                .expect_err("create fault should fail"),
            VaultIoFailure::WriteTemp => {
                let mut file = fs::File::create(&temporary_path).expect("temporary fixture");
                adapter
                    .write_temp(&mut file, &temporary_path, b"data")
                    .expect_err("write fault should fail")
            }
            VaultIoFailure::SyncTemp => {
                let file = fs::File::create(&temporary_path).expect("temporary fixture");
                adapter
                    .sync_temp(&file, &temporary_path)
                    .expect_err("sync fault should fail")
            }
            VaultIoFailure::Replace => adapter
                .replace(&temporary_path, store.path())
                .expect_err("replace fault should fail"),
            VaultIoFailure::SyncParent => adapter
                .sync_parent(store.path())
                .expect_err("parent sync fault should fail"),
            VaultIoFailure::CleanupTemp => adapter
                .cleanup_temp(&temporary_path)
                .expect_err("cleanup fault should fail"),
        };
        assert!(matches!(error, VaultError::Io { .. }));
        let _ = fs::remove_file(&temporary_path);
    }
}
