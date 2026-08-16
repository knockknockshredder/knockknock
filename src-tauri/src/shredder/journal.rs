// src-tauri/src/shredder/journal.rs

pub use crate::shredder::errors::JournalError;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalOperation {
    RootFileRename,
    LegacyPathOnly,
}

impl Default for JournalOperation {
    fn default() -> Self {
        Self::LegacyPathOnly
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalNodeKind {
    RegularFile,
    Directory,
    Link,
    Special,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalNodeIdentity {
    pub id: u128,
    pub mount_id: u64,
}

impl JournalNodeIdentity {
    pub const fn new(id: u128, mount_id: u64) -> Self {
        Self { id, mount_id }
    }
}

/// Durable cleanup record. `renamed_path` is diagnostic only and is never used
/// as an execution input. Recovery reconstructs an operational child name from
/// `trusted_parent_path` and `generated_basename` after identity validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    #[serde(default)]
    pub operation: JournalOperation,
    #[serde(default)]
    pub trusted_parent_path: PathBuf,
    #[serde(default)]
    pub trusted_parent_identity: Option<JournalNodeIdentity>,
    #[serde(default)]
    pub generated_basename: Option<String>,
    #[serde(default)]
    pub node_identity: Option<JournalNodeIdentity>,
    #[serde(default)]
    pub node_kind: Option<JournalNodeKind>,
    #[serde(default)]
    pub original_path_hash: Option<String>,
    #[serde(default)]
    pub renamed_path: PathBuf,
    #[serde(default)]
    pub timestamp: u64,
}

impl JournalEntry {
    pub fn identity_bound(
        trusted_parent_path: PathBuf,
        trusted_parent_identity: JournalNodeIdentity,
        generated_basename: impl Into<String>,
        node_identity: JournalNodeIdentity,
        node_kind: JournalNodeKind,
    ) -> Self {
        let generated_basename = generated_basename.into();
        Self {
            operation: JournalOperation::RootFileRename,
            renamed_path: trusted_parent_path.join(&generated_basename),
            trusted_parent_path,
            trusted_parent_identity: Some(trusted_parent_identity),
            generated_basename: Some(generated_basename),
            node_identity: Some(node_identity),
            node_kind: Some(node_kind),
            original_path_hash: None,
            timestamp: now_secs(),
        }
    }

    pub(crate) fn for_root_rename(
        parent_path: &Path,
        parent_identity: crate::shredder::root_execution::NodeIdentity,
        generated_basename: &OsStr,
        node_identity: crate::shredder::root_execution::NodeIdentity,
        node_kind: crate::shredder::root_execution::NodeKind,
    ) -> Result<Self, JournalError> {
        let basename =
            generated_basename
                .to_str()
                .ok_or_else(|| JournalError::IdentityMismatch {
                    path: parent_path.to_path_buf(),
                    reason: "generated basename is not valid UTF-8".to_string(),
                })?;
        Ok(Self::identity_bound(
            parent_path.to_path_buf(),
            JournalNodeIdentity::new(parent_identity.id() as u128, parent_identity.mount_id()),
            basename,
            JournalNodeIdentity::new(node_identity.id(), node_identity.mount_id()),
            match node_kind {
                crate::shredder::root_execution::NodeKind::RegularFile => {
                    JournalNodeKind::RegularFile
                }
                crate::shredder::root_execution::NodeKind::Directory => JournalNodeKind::Directory,
                crate::shredder::root_execution::NodeKind::Link => JournalNodeKind::Link,
                crate::shredder::root_execution::NodeKind::Special => JournalNodeKind::Special,
            },
        ))
    }

    fn is_identity_bound(&self) -> bool {
        self.operation == JournalOperation::RootFileRename
            && self.trusted_parent_identity.is_some()
            && self.generated_basename.is_some()
            && self.node_identity.is_some()
            && self.node_kind.is_some()
            && valid_basename(self.generated_basename.as_deref().unwrap_or_default())
    }

    fn operational_path(&self) -> Result<PathBuf, JournalError> {
        if !self.is_identity_bound() {
            return Err(JournalError::LegacyRecord {
                path: self.renamed_path.clone(),
            });
        }
        Ok(self
            .trusted_parent_path
            .join(self.generated_basename.as_deref().unwrap_or_default()))
    }
}

pub trait JournalIo: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>>;
    fn write_temp(&self, path: &Path, contents: &[u8]) -> io::Result<PathBuf>;
    fn sync(&self, path: &Path) -> io::Result<()>;
    fn sync_parent(&self, path: &Path) -> io::Result<()>;
    fn atomic_replace(&self, temporary: &Path, destination: &Path) -> io::Result<()>;
    fn delete(&self, path: &Path) -> io::Result<()>;
}

struct FsJournalIo;

impl JournalIo for FsJournalIo {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn write_temp(&self, path: &Path, contents: &[u8]) -> io::Result<PathBuf> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        match std::fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        // On Unix the journal is created owner-only (0o600) AT CREATION so the
        // rename below carries the mode to the final journal without a
        // world-readable window; `create_new` is already set, so there is no
        // symlink-follow risk. On Windows the KnockKnock data directory ACLs
        // restrict access to the owning user (documented in Phase 5).
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(contents)?;
        file.flush()?;
        Ok(temporary)
    }

    fn sync(&self, path: &Path) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?
            .sync_all()
    }

    fn sync_parent(&self, path: &Path) -> io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::Storage::FileSystem::{
                CreateFileW, FlushFileBuffers, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_READ,
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            };

            let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(path.as_ptr()),
                    FILE_GENERIC_READ.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    None,
                )
            }
            .map_err(|error| io::Error::other(error.to_string()))?;
            let flush_result = match unsafe { FlushFileBuffers(handle) } {
                Ok(()) => Ok(()),
                // Windows may reject FlushFileBuffers for a directory handle
                // even when the directory was opened successfully.
                Err(error) if (error.code().0 as u32 & 0xffff) == 5 => Ok(()),
                Err(error) => Err(io::Error::other(error.to_string())),
            };
            let close_result =
                unsafe { CloseHandle(handle) }.map_err(|error| io::Error::other(error.to_string()));
            flush_result.and(close_result)
        }
        #[cfg(not(windows))]
        {
            std::fs::File::open(path)?.sync_all()
        }
    }

    fn atomic_replace(&self, temporary: &Path, destination: &Path) -> io::Result<()> {
        std::fs::rename(temporary, destination)
    }

    fn delete(&self, path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub struct JournalStore {
    path: PathBuf,
    io: Arc<dyn JournalIo>,
}

impl JournalStore {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            io: Arc::new(FsJournalIo),
        }
    }

    pub(crate) fn with_io(path: impl Into<PathBuf>, io: Arc<dyn JournalIo>) -> Self {
        Self {
            path: path.into(),
            io,
        }
    }

    pub(crate) fn portable() -> Result<Self, JournalError> {
        journal_path()
            .map(Self::at)
            .map_err(|message| JournalError::Io {
                operation: "resolve journal path",
                path: PathBuf::new(),
                message,
            })
    }

    pub fn append(&self, entry: JournalEntry) -> Result<(), JournalError> {
        if !entry.is_identity_bound() {
            return Err(JournalError::LegacyRecord {
                path: entry.renamed_path,
            });
        }
        let mut entries = self.read()?;
        let previous = entries.clone();
        entries.push(entry);
        self.write_entries(&entries, &previous)
    }

    pub fn read(&self) -> Result<Vec<JournalEntry>, JournalError> {
        let Some(bytes) = self
            .io
            .read(&self.path)
            .map_err(|error| io_error("read", &self.path, error))?
        else {
            return Ok(Vec::new());
        };
        serde_json::from_slice(&bytes).map_err(|error| JournalError::Decode {
            path: self.path.clone(),
            message: error.to_string(),
        })
    }

    pub fn clear(&self, entry: &JournalEntry) -> Result<(), JournalError> {
        if !entry.is_identity_bound() {
            return Err(JournalError::LegacyRecord {
                path: entry.renamed_path.clone(),
            });
        }
        let mut entries = self.read()?;
        let previous = entries.clone();
        let original_len = entries.len();
        entries.retain(|candidate| candidate != entry);
        if entries.len() == original_len {
            return Err(JournalError::RecordNotFound {
                path: entry.renamed_path.clone(),
            });
        }
        self.write_entries(&entries, &previous)
    }

    /// Validate and remove only identity-bound records. Legacy records stop
    /// recovery and remain on disk for an explicit migration decision.
    pub fn recover(&self) -> Result<Vec<JournalEntry>, JournalError> {
        let entries = self.read()?;
        for entry in &entries {
            if !entry.is_identity_bound() {
                return Err(JournalError::LegacyRecord {
                    path: entry.renamed_path.clone(),
                });
            }
            self.recover_entry(entry)?;
        }
        Ok(self.read()?)
    }

    fn recover_entry(&self, entry: &JournalEntry) -> Result<(), JournalError> {
        validate_recovery_parent(entry)?;
        let operational_path = entry.operational_path()?;
        let metadata = std::fs::symlink_metadata(&operational_path).map_err(|error| {
            JournalError::IdentityMismatch {
                path: entry.renamed_path.clone(),
                reason: format!("generated target cannot be validated: {error}"),
            }
        })?;
        let actual_identity = metadata_identity(&operational_path, &metadata).ok_or_else(|| {
            JournalError::IdentityMismatch {
                path: entry.renamed_path.clone(),
                reason: "filesystem identity is unavailable".to_string(),
            }
        })?;
        if Some(actual_identity) != entry.node_identity {
            return Err(JournalError::IdentityMismatch {
                path: entry.renamed_path.clone(),
                reason: "generated target identity does not match the journal".to_string(),
            });
        }
        let actual_kind = metadata_kind(&metadata);
        if Some(actual_kind) != entry.node_kind {
            return Err(JournalError::IdentityMismatch {
                path: entry.renamed_path.clone(),
                reason: "generated target kind does not match the journal".to_string(),
            });
        }
        if actual_kind != JournalNodeKind::RegularFile {
            return Err(JournalError::IdentityMismatch {
                path: entry.renamed_path.clone(),
                reason: "only regular files are valid recovery targets".to_string(),
            });
        }

        self.io
            .delete(&operational_path)
            .map_err(|error| io_error("recover generated file", &operational_path, error))?;
        self.io
            .sync_parent(&entry.trusted_parent_path)
            .map_err(|error| {
                io_error("sync recovered parent", &entry.trusted_parent_path, error)
            })?;
        self.clear(entry)
    }

    fn write_entries(
        &self,
        entries: &[JournalEntry],
        previous: &[JournalEntry],
    ) -> Result<(), JournalError> {
        match self.write_entries_once(entries) {
            Err(
                error @ JournalError::Io {
                    operation: "sync journal",
                    ..
                },
            ) => match self.write_entries_once(previous) {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(JournalError::Io {
                    operation: "restore journal after sync failure",
                    path: self.path.clone(),
                    message: format!("{error}; restore failed: {rollback_error}"),
                }),
            },
            result => result,
        }
    }

    fn write_entries_once(&self, entries: &[JournalEntry]) -> Result<(), JournalError> {
        let json = serde_json::to_vec_pretty(entries)
            .map_err(|error| JournalError::Serialize(error.to_string()))?;
        let temporary = self
            .io
            .write_temp(&self.path, &json)
            .map_err(|error| io_error("write temporary journal", &self.path, error))?;
        if let Err(error) = self.io.sync(&temporary) {
            return Err(self.cleanup_after_failure(
                temporary,
                io_error("sync temporary journal", &self.path, error),
            ));
        }
        if let Err(error) = self.io.atomic_replace(&temporary, &self.path) {
            return Err(self.cleanup_after_failure(
                temporary,
                io_error("atomically replace journal", &self.path, error),
            ));
        }
        self.io
            .sync(&self.path)
            .map_err(|error| io_error("sync journal", &self.path, error))
    }

    fn cleanup_after_failure(&self, temporary: PathBuf, error: JournalError) -> JournalError {
        match self.io.delete(&temporary) {
            Ok(()) => error,
            Err(cleanup_error) => JournalError::Io {
                operation: "clean up temporary journal",
                path: temporary,
                message: format!("{error}; cleanup failed: {cleanup_error}"),
            },
        }
    }
}

fn io_error(operation: &'static str, path: &Path, error: io::Error) -> JournalError {
    JournalError::Io {
        operation,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(crate::paths::portable_data_dir()?.join(".knockknock-journal.json"))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn valid_basename(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn validate_recovery_parent(entry: &JournalEntry) -> Result<(), JournalError> {
    let parent = &entry.trusted_parent_path;
    if !parent.is_absolute()
        || parent
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return Err(JournalError::UnsafeParent {
            path: parent.clone(),
            reason: "parent is not an absolute, normalized path".to_string(),
        });
    }
    if parent.parent().is_none() || crate::shredder::validation::is_network_drive(parent) {
        return Err(JournalError::UnsafeParent {
            path: parent.clone(),
            reason: "filesystem roots and network locations are not allowed".to_string(),
        });
    }
    let metadata =
        std::fs::symlink_metadata(parent).map_err(|error| JournalError::UnsafeParent {
            path: parent.clone(),
            reason: format!("trusted parent cannot be inspected: {error}"),
        })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(JournalError::UnsafeParent {
            path: parent.clone(),
            reason: "trusted parent is not a real directory".to_string(),
        });
    }
    let identity =
        metadata_identity(parent, &metadata).ok_or_else(|| JournalError::UnsafeParent {
            path: parent.clone(),
            reason: "trusted parent identity is unavailable".to_string(),
        })?;
    if Some(identity) != entry.trusted_parent_identity {
        return Err(JournalError::IdentityMismatch {
            path: parent.clone(),
            reason: "trusted parent identity does not match the journal".to_string(),
        });
    }
    Ok(())
}

fn metadata_kind(metadata: &std::fs::Metadata) -> JournalNodeKind {
    if metadata.file_type().is_symlink() {
        JournalNodeKind::Link
    } else if metadata.is_file() {
        JournalNodeKind::RegularFile
    } else if metadata.is_dir() {
        JournalNodeKind::Directory
    } else {
        JournalNodeKind::Special
    }
}

fn metadata_identity(path: &Path, metadata: &std::fs::Metadata) -> Option<JournalNodeIdentity> {
    #[cfg(windows)]
    let _ = metadata;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Some(JournalNodeIdentity::new(
            metadata.ino() as u128,
            metadata.dev(),
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };

        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // Windows' stable Rust metadata API does not expose file identity. The
        // handle query is required to bind recovery to the original node.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )
        }
        .ok()?;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        let queried = unsafe { GetFileInformationByHandle(handle, &mut information) }.is_ok();
        let closed = unsafe { CloseHandle(handle) }.is_ok();
        if !queried || !closed {
            return None;
        }
        return Some(JournalNodeIdentity::new(
            ((information.nFileIndexHigh as u128) << 32) | information.nFileIndexLow as u128,
            information.dwVolumeSerialNumber as u64,
        ));
    }
    #[allow(unreachable_code)]
    let _ = (path, metadata);
    None
}

pub fn cleanup_orphans() -> Result<Vec<JournalEntry>, JournalError> {
    JournalStore::portable()?.recover()
}

#[cfg(test)]
mod tests {
    use super::{
        JournalEntry, JournalError, JournalIo, JournalNodeIdentity, JournalNodeKind, JournalStore,
    };
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Faults {
        fail_write: bool,
        fail_sync: bool,
        fail_replace: bool,
        fail_delete: bool,
    }

    struct FakeJournalIo {
        faults: Arc<Mutex<Faults>>,
    }

    #[derive(Default)]
    struct RecordingState {
        current: Option<Vec<u8>>,
        temporary: Option<Vec<u8>>,
        syncs: usize,
        fail_sync_at: Option<usize>,
    }

    struct RecordingJournalIo {
        state: Arc<Mutex<RecordingState>>,
    }

    impl JournalIo for FakeJournalIo {
        fn read(&self, _path: &Path) -> io::Result<Option<Vec<u8>>> {
            Ok(None)
        }

        fn write_temp(&self, _path: &Path, _contents: &[u8]) -> io::Result<PathBuf> {
            if self.faults.lock().unwrap().fail_write {
                return Err(io::Error::other("injected journal write failure"));
            }
            Ok(PathBuf::from("journal.tmp"))
        }

        fn sync(&self, _path: &Path) -> io::Result<()> {
            if self.faults.lock().unwrap().fail_sync {
                return Err(io::Error::other("injected journal sync failure"));
            }
            Ok(())
        }

        fn sync_parent(&self, _path: &Path) -> io::Result<()> {
            Ok(())
        }

        fn atomic_replace(&self, _temporary: &Path, _destination: &Path) -> io::Result<()> {
            if self.faults.lock().unwrap().fail_replace {
                return Err(io::Error::other("injected journal replacement failure"));
            }
            Ok(())
        }

        fn delete(&self, _path: &Path) -> io::Result<()> {
            if self.faults.lock().unwrap().fail_delete {
                return Err(io::Error::other("injected journal cleanup failure"));
            }
            Ok(())
        }
    }

    impl JournalIo for RecordingJournalIo {
        fn read(&self, _path: &Path) -> io::Result<Option<Vec<u8>>> {
            Ok(self.state.lock().unwrap().current.clone())
        }

        fn write_temp(&self, _path: &Path, contents: &[u8]) -> io::Result<PathBuf> {
            self.state.lock().unwrap().temporary = Some(contents.to_vec());
            Ok(PathBuf::from("journal.tmp"))
        }

        fn sync(&self, _path: &Path) -> io::Result<()> {
            let mut state = self.state.lock().unwrap();
            state.syncs += 1;
            if state.fail_sync_at == Some(state.syncs) {
                return Err(io::Error::other("injected journal sync failure"));
            }
            Ok(())
        }

        fn sync_parent(&self, _path: &Path) -> io::Result<()> {
            Ok(())
        }

        fn atomic_replace(&self, _temporary: &Path, _destination: &Path) -> io::Result<()> {
            let mut state = self.state.lock().unwrap();
            state.current = state.temporary.take();
            Ok(())
        }

        fn delete(&self, _path: &Path) -> io::Result<()> {
            self.state.lock().unwrap().temporary = None;
            Ok(())
        }
    }

    fn entry() -> JournalEntry {
        JournalEntry::identity_bound(
            PathBuf::from("/tmp/trusted-parent"),
            JournalNodeIdentity::new(10, 20),
            ".knockknock-generated",
            JournalNodeIdentity::new(30, 40),
            JournalNodeKind::RegularFile,
        )
    }

    #[test]
    fn temporary_store_round_trips_identity_bound_entries() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = JournalStore::at(directory.path().join("journal.json"));
        let expected = entry();

        store.append(expected.clone()).expect("journal append");

        let entries = store.read().expect("journal read");
        assert_eq!(entries, vec![expected]);
    }

    #[cfg(unix)]
    #[test]
    fn journal_files_are_created_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let journal_path = directory.path().join(".journal.json");
        let store = JournalStore::at(&journal_path);

        store.append(entry()).expect("journal append");

        let mode = std::fs::metadata(&journal_path)
            .expect("journal metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "journal must not be group- or world-accessible"
        );
        assert!(
            !journal_path.with_extension("tmp").exists(),
            "temporary journal must not remain after the write"
        );
    }

    #[test]
    fn journal_write_fault_is_returned_and_does_not_create_a_record() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let faults = Arc::new(Mutex::new(Faults {
            fail_write: true,
            ..Faults::default()
        }));
        let store = JournalStore::with_io(
            directory.path().join("journal.json"),
            Arc::new(FakeJournalIo {
                faults: Arc::clone(&faults),
            }),
        );

        let error = store.append(entry()).expect_err("write fault must fail");
        assert!(matches!(error, JournalError::Io { .. }));
    }

    #[test]
    fn clear_sync_fault_is_returned_and_record_remains_recoverable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = Arc::new(Mutex::new(RecordingState {
            fail_sync_at: Some(4),
            ..RecordingState::default()
        }));
        let store = JournalStore::with_io(
            directory.path().join("journal.json"),
            Arc::new(RecordingJournalIo {
                state: Arc::clone(&state),
            }),
        );
        let expected = entry();
        store.append(expected.clone()).expect("journal append");

        let error = store
            .clear(&expected)
            .expect_err("clear sync fault must fail");
        assert!(matches!(error, JournalError::Io { .. }));
        assert_eq!(store.read().expect("journal read"), vec![expected]);
    }

    #[test]
    fn legacy_path_only_records_are_retained_and_reported() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("journal.json");
        std::fs::write(
            &path,
            r#"[{"original_path_hash":"legacy","renamed_path":"/tmp/legacy","timestamp":1}]"#,
        )
        .expect("legacy journal write");
        let store = JournalStore::at(&path);

        let error = store.recover().expect_err("legacy records are untrusted");
        assert!(matches!(error, JournalError::LegacyRecord { .. }));
        assert!(!store.read().expect("legacy record remains").is_empty());
    }

    #[test]
    fn matching_identity_recovery_deletes_target_and_clears_record() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join(".knockknock-generated");
        std::fs::write(&target, b"destroy").expect("target write");
        let parent_metadata = std::fs::symlink_metadata(directory.path()).expect("parent metadata");
        let target_metadata = std::fs::symlink_metadata(&target).expect("target metadata");
        let parent_identity =
            super::metadata_identity(directory.path(), &parent_metadata).expect("parent identity");
        let target_identity =
            super::metadata_identity(&target, &target_metadata).expect("target identity");
        let store = JournalStore::at(directory.path().join("journal.json"));
        store
            .append(JournalEntry::identity_bound(
                directory.path().to_path_buf(),
                parent_identity,
                ".knockknock-generated",
                target_identity,
                JournalNodeKind::RegularFile,
            ))
            .expect("journal append");

        assert!(store.recover().expect("matching recovery").is_empty());
        assert!(!target.exists());
        assert!(store.read().expect("journal read").is_empty());
    }

    #[test]
    fn unsafe_recovery_parent_is_rejected_without_mutating_target() {
        #[cfg(windows)]
        let filesystem_root = PathBuf::from(r"C:\");
        #[cfg(not(windows))]
        let filesystem_root = PathBuf::from("/");

        for (name, parent) in [
            ("relative", PathBuf::from("relative-parent")),
            ("filesystem-root", filesystem_root),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let target = directory.path().join(format!("{name}-target"));
            std::fs::write(&target, b"preserve").expect("target write");
            let store = JournalStore::at(directory.path().join("journal.json"));
            store
                .append(JournalEntry::identity_bound(
                    parent,
                    JournalNodeIdentity::new(1, 1),
                    format!("{name}-target"),
                    JournalNodeIdentity::new(2, 2),
                    JournalNodeKind::RegularFile,
                ))
                .expect("journal append");

            let error = store.recover().expect_err("unsafe parent must be rejected");
            assert!(matches!(error, JournalError::UnsafeParent { .. }));
            assert!(target.exists());
            assert!(!store.read().expect("journal read").is_empty());
        }
    }
}
