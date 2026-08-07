use super::plan::{
    ChildName, DirHandle, FileShredRequest, FileShredResult, NodeHandle, NodeIdentity, NodeKind,
    RenamedNode,
};
use super::{execute_roots, OpenFileShredder, SecureTreeIo};
use crate::shredder::cancel::CancellationToken;
use crate::shredder::engine::OverwriteState;
use crate::shredder::errors::JournalError;
use crate::shredder::errors::ShredError;
use crate::shredder::journal::{
    JournalEntry, JournalIo, JournalNodeIdentity, JournalNodeKind, JournalStore,
};
use crate::shredder::progress::NoopProgressReporter;
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::{
    BatchRootResult, DeletionPolicy, ExecuteRootRequest, ExecuteRootsRequest, ExecutionStage,
    RootStatus, TargetKind, WriteCheckOutcome,
};
use crate::shredder::PolicyFileShredder;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FakeNode {
    identity: NodeIdentity,
    kind: NodeKind,
    children: Vec<(OsString, u64)>,
}

#[derive(Default)]
struct FakeEvents {
    calls: Vec<String>,
    root_opens: usize,
    rename_calls: usize,
}

struct FakeIo {
    roots: HashMap<PathBuf, u64>,
    nodes: HashMap<u64, FakeNode>,
    fail_enumerate: HashSet<u64>,
    fail_rename: bool,
    fail_rollback: bool,
    fail_unlink: bool,
    fail_sync: bool,
    events: Arc<Mutex<FakeEvents>>,
}

impl FakeIo {
    fn new() -> Self {
        Self {
            roots: HashMap::new(),
            nodes: HashMap::new(),
            fail_enumerate: HashSet::new(),
            fail_rename: false,
            fail_rollback: false,
            fail_unlink: false,
            fail_sync: false,
            events: Arc::new(Mutex::new(FakeEvents::default())),
        }
    }

    fn root(mut self, path: PathBuf, handle: u64, node: FakeNode) -> Self {
        self.roots.insert(path, handle);
        self.nodes.insert(handle, node);
        self
    }

    fn add_node(mut self, handle: u64, node: FakeNode) -> Self {
        self.nodes.insert(handle, node);
        self
    }

    fn fail_directory(mut self, handle: u64) -> Self {
        self.fail_enumerate.insert(handle);
        self
    }

    fn fail_rename(mut self) -> Self {
        self.fail_rename = true;
        self
    }

    fn fail_unlink(mut self) -> Self {
        self.fail_unlink = true;
        self
    }

    fn fail_rollback(mut self) -> Self {
        self.fail_rollback = true;
        self
    }

    fn fail_sync(mut self) -> Self {
        self.fail_sync = true;
        self
    }

    fn record(&self, event: &str) {
        self.events.lock().unwrap().calls.push(event.to_string());
    }

    fn get_node(&self, handle: &NodeHandle) -> Result<&FakeNode, ShredError> {
        self.nodes.get(&handle.id()).ok_or_else(|| {
            ShredError::ValidationFailed(format!("unknown fake node {}", handle.id()))
        })
    }

    fn events(&self) -> Arc<Mutex<FakeEvents>> {
        Arc::clone(&self.events)
    }
}

impl SecureTreeIo for FakeIo {
    fn open_root_nofollow(&self, path: &Path) -> Result<DirHandle, ShredError> {
        self.events.lock().unwrap().root_opens += 1;
        self.record(&format!("open_root:{}", path.display()));
        self.roots
            .get(path)
            .copied()
            .map(DirHandle::new)
            .ok_or_else(|| ShredError::ValidationFailed("missing fake root".to_string()))
    }

    fn enumerate(&self, dir: &DirHandle) -> Result<Vec<ChildName>, ShredError> {
        self.record("enumerate");
        if self.fail_enumerate.contains(&dir.id()) {
            return Err(ShredError::ValidationFailed(
                "injected unreadable directory".to_string(),
            ));
        }
        self.nodes
            .get(&dir.id())
            .ok_or_else(|| ShredError::ValidationFailed("unknown fake directory".to_string()))
            .map(|node| {
                node.children
                    .iter()
                    .map(|(name, _)| ChildName::new(name.clone()))
                    .collect()
            })
    }

    fn open_child_nofollow(
        &self,
        parent: &DirHandle,
        name: &OsStr,
    ) -> Result<NodeHandle, ShredError> {
        self.record("open_child");
        let node = self
            .nodes
            .get(&parent.id())
            .ok_or_else(|| ShredError::ValidationFailed("unknown fake parent".to_string()))?;
        node.children
            .iter()
            .find(|(child_name, _)| child_name == name)
            .map(|(_, handle)| NodeHandle::new(*handle))
            .ok_or_else(|| ShredError::ValidationFailed("unknown fake child".to_string()))
    }

    fn identity(&self, node: &NodeHandle) -> Result<NodeIdentity, ShredError> {
        self.record("identity");
        Ok(self.get_node(node)?.identity)
    }

    fn open_regular_for_shred(&self, node: &NodeHandle) -> Result<File, ShredError> {
        self.record("open_regular");
        if self.get_node(node)?.kind != NodeKind::RegularFile {
            return Err(ShredError::ValidationFailed(
                "fake adapter opened a non-regular node".to_string(),
            ));
        }
        tempfile::tempfile()
            .map_err(|error| ShredError::from_io_error(PathBuf::from("fake"), error))
    }

    fn rename_noreplace(
        &self,
        _parent: &DirHandle,
        _node: &NodeHandle,
        _new_name: &OsStr,
    ) -> Result<RenamedNode, ShredError> {
        let mut events = self.events.lock().unwrap();
        events.calls.push("rename".to_string());
        events.rename_calls += 1;
        if self.fail_rename || (self.fail_rollback && events.rename_calls > 1) {
            return Err(ShredError::ValidationFailed(
                "injected rename failure".to_string(),
            ));
        }
        Ok(RenamedNode::new())
    }

    fn unlink_leaf(&self, _parent: &DirHandle, _node: &NodeHandle) -> Result<(), ShredError> {
        self.record("unlink");
        if self.fail_unlink {
            return Err(ShredError::ValidationFailed(
                "injected deletion failure".to_string(),
            ));
        }
        Ok(())
    }

    fn remove_empty_dir(&self, _parent: &DirHandle, _node: &NodeHandle) -> Result<(), ShredError> {
        self.record("remove_dir");
        Ok(())
    }

    fn sync_parent(&self, _parent: &DirHandle) -> Result<(), ShredError> {
        self.record("sync");
        if self.fail_sync {
            return Err(ShredError::ValidationFailed(
                "injected parent sync failure".to_string(),
            ));
        }
        Ok(())
    }
}

/// Outcome a fake shredder reports for one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FakeShredOutcome {
    /// Completed overwrite with a passed write check (clean removal).
    Completed,
    /// Completed overwrite with a failed final write check: removal still
    /// proceeds, but the root must never be a clean Destroyed.
    CompletedCheckFailed,
    /// Partial overwrite with a cancelled issue: cleanup still proceeds.
    Partial,
    /// Zero-length vacuous completion or Off write check: nothing to report.
    CompletedNoCheck,
    /// Hard shredder error before any byte was written (NotStarted
    /// semantics): the target stays intact and must not be renamed.
    NotStarted,
}

struct FakeShredder {
    calls: Arc<Mutex<Vec<FileShredRequest>>>,
    outcomes: Mutex<VecDeque<FakeShredOutcome>>,
}

impl FakeShredder {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            outcomes: Mutex::new(VecDeque::new()),
        }
    }

    /// Every file answered with `outcome` (queue is consumed per call and
    /// defaults to `Completed` once exhausted).
    fn outcome(self, outcome: FakeShredOutcome) -> Self {
        self.outcomes.lock().unwrap().push_back(outcome);
        self
    }

    fn outcomes(self, outcomes: Vec<FakeShredOutcome>) -> Self {
        self.outcomes.lock().unwrap().extend(outcomes);
        self
    }

    fn next_outcome(&self) -> FakeShredOutcome {
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(FakeShredOutcome::Completed)
    }
}

impl OpenFileShredder for FakeShredder {
    fn shred_open_file(
        &self,
        _file: File,
        _identity: NodeIdentity,
        request: &FileShredRequest,
    ) -> Result<FileShredResult, ShredError> {
        self.calls.lock().unwrap().push(request.clone());
        let path = request.diagnostic_path().to_path_buf();
        match self.next_outcome() {
            FakeShredOutcome::Completed => Ok(FileShredResult {
                overwrite_state: OverwriteState::Completed,
                write_check_status: WriteCheckOutcome::Passed,
                bytes_shredded: 7,
                issues: Vec::new(),
            }),
            FakeShredOutcome::CompletedCheckFailed => Ok(FileShredResult {
                overwrite_state: OverwriteState::Completed,
                write_check_status: WriteCheckOutcome::Failed,
                bytes_shredded: 7,
                issues: vec![ShredError::WriteCheckFailed { path }],
            }),
            FakeShredOutcome::Partial => Ok(FileShredResult {
                overwrite_state: OverwriteState::Partial,
                write_check_status: WriteCheckOutcome::NotRun,
                bytes_shredded: 7,
                issues: vec![ShredError::IoError {
                    path,
                    kind: "Cancelled".to_string(),
                    message: "injected partial shredding".to_string(),
                }],
            }),
            FakeShredOutcome::CompletedNoCheck => Ok(FileShredResult {
                overwrite_state: OverwriteState::Completed,
                write_check_status: WriteCheckOutcome::NotRun,
                bytes_shredded: 0,
                issues: Vec::new(),
            }),
            FakeShredOutcome::NotStarted => Err(ShredError::ValidationFailed(
                "injected child failure".to_string(),
            )),
        }
    }
}

fn home_child(name: &str) -> PathBuf {
    std::env::home_dir().expect("home directory").join(name)
}

/// A temporary directory under the real home directory (root execution
/// refuses roots outside home), removed on drop.
struct TempHomeDir(PathBuf);

impl TempHomeDir {
    fn new(label: &str) -> Self {
        let home = std::env::home_dir().expect("home directory");
        let unique = format!(
            ".knockknock-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let path = home.join(unique);
        std::fs::create_dir(&path).expect("create home fixture");
        TempHomeDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempHomeDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn root_request(id: &str, path: &Path, kind: TargetKind) -> ExecuteRootRequest {
    ExecuteRootRequest {
        target_id: id.to_string(),
        path: path.to_string_lossy().into_owned(),
        kind,
    }
}

fn regular(identity: u128) -> FakeNode {
    FakeNode {
        identity: NodeIdentity::regular(identity, 1),
        kind: NodeKind::RegularFile,
        children: Vec::new(),
    }
}

fn directory(identity: u128, children: Vec<(u64, &str)>) -> FakeNode {
    FakeNode {
        identity: NodeIdentity::directory(identity, 1),
        kind: NodeKind::Directory,
        children: children
            .into_iter()
            .map(|(handle, name)| (OsString::from(name), handle))
            .collect(),
    }
}

fn run(
    request: ExecuteRootsRequest,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
) -> BatchRootResult {
    let progress = NoopProgressReporter;
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    run_full(
        request,
        DeletionPolicy::default(),
        io,
        shredder,
        &journal,
        &progress,
        &CancellationToken::new(),
    )
}

#[test]
fn journal_write_failure_prevents_root_rename() {
    let root = home_child("task8-journal-write-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2));
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::new(FailingJournalIo::write()),
    );

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("write-failure", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|call| call == "rename"));
    assert!(result.roots[0]
        .errors
        .iter()
        .any(|error| error.stage == ExecutionStage::Journal));
}

#[test]
fn journal_sync_failure_prevents_root_rename() {
    let root = home_child("task8-journal-sync-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2));
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::new(FailingJournalIo::sync()),
    );

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("sync-failure", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|call| call == "rename"));
}

#[test]
fn executes_file_root_using_its_containing_directory() {
    let parent = home_child("task8-file-root-parent");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![root_request("file-root", &root, TargetKind::File)],
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert!(result.roots[0].root_removed);
    assert_eq!(result.roots[0].files_destroyed, 1);

    let events = io.events();
    let events = events.lock().unwrap();
    let first_mutation = events
        .calls
        .iter()
        .position(|call| call == "open_regular")
        .expect("file mutation must open the regular file");
    assert!(events.calls[..first_mutation]
        .iter()
        .any(|call| call == &format!("open_root:{}", parent.display())));
    assert!(
        events.calls[..first_mutation]
            .iter()
            .filter(|call| call.starts_with("open_root:"))
            .count()
            >= 2
    );
    assert!(events.calls[first_mutation..]
        .iter()
        .all(|call| !call.starts_with("open_root:")));
}

#[test]
fn rename_failure_retains_identity_bound_journal_record() {
    let root = home_child("task8-rename-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .fail_rename();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("rename-failure", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!journal.read().expect("journal read").is_empty());
}

#[test]
fn parent_sync_failure_rolls_back_without_deleting() {
    let root = home_child("task8-sync-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .fail_sync();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("sync-failure", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|call| call == "unlink"));
    assert!(!journal.read().expect("journal read").is_empty());
}

#[test]
fn deletion_failure_rolls_back_and_retains_journal_record() {
    let root = home_child("task8-delete-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .fail_unlink();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("delete-failure", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!result.roots[0].root_removed);
    assert_eq!(io.events().lock().unwrap().rename_calls, 2);
    assert!(!journal.read().expect("journal read").is_empty());
}

#[test]
fn rollback_failure_is_reported_and_never_widens_scope() {
    let root = home_child("task8-rollback-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .fail_unlink()
        .fail_rollback();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request(
                "rollback-failure",
                &root,
                TargetKind::Directory,
            )],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(result.roots[0]
        .errors
        .iter()
        .any(|error| error.message.contains("rollback to original name failed")));
    assert!(!result.roots[0].root_removed);
    assert!(!journal.read().expect("journal read").is_empty());
}

#[test]
fn recovery_identity_mismatch_retains_record_and_target() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join(".knockknock-generated");
    std::fs::write(&target, b"preserve").expect("target write");
    let store = JournalStore::at(directory.path().join("journal.json"));
    let entry = JournalEntry::identity_bound(
        directory.path().to_path_buf(),
        JournalNodeIdentity::new(0, 0),
        ".knockknock-generated",
        JournalNodeIdentity::new(0, 0),
        JournalNodeKind::RegularFile,
    );
    store.append(entry).expect("journal append");

    let error = store.recover().expect_err("identity mismatch must fail");

    assert!(matches!(error, JournalError::IdentityMismatch { .. }));
    assert!(target.exists());
    assert!(!store.read().expect("journal read").is_empty());
}

#[test]
fn journal_clear_failure_after_delete_is_reported_and_retained() {
    let root = home_child("task8-journal-clear-failure");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2));
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let state = Arc::new(Mutex::new(ClearFailState {
        fail_sync_at: Some(4),
        ..ClearFailState::default()
    }));
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::new(ClearFailJournalIo {
            state: Arc::clone(&state),
        }),
    );

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request(
                "journal-clear-failure",
                &root,
                TargetKind::Directory,
            )],
        },
        &io,
        &shredder,
        &journal,
    );

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert_eq!(result.roots[0].files_destroyed, 1);
    assert!(result.roots[0]
        .errors
        .iter()
        .any(|error| error.stage == ExecutionStage::Journal));
    assert!(!journal.read().expect("journal read").is_empty());
}

struct FailingJournalIo {
    fail_write: bool,
    fail_sync: bool,
}

impl FailingJournalIo {
    fn write() -> Self {
        Self {
            fail_write: true,
            fail_sync: false,
        }
    }

    fn sync() -> Self {
        Self {
            fail_write: false,
            fail_sync: true,
        }
    }
}

impl JournalIo for FailingJournalIo {
    fn read(&self, _path: &Path) -> std::io::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn write_temp(&self, _path: &Path, _contents: &[u8]) -> std::io::Result<PathBuf> {
        if self.fail_write {
            Err(std::io::Error::other("injected journal write failure"))
        } else {
            Ok(PathBuf::from("journal.tmp"))
        }
    }

    fn sync(&self, _path: &Path) -> std::io::Result<()> {
        if self.fail_sync {
            return Err(std::io::Error::other("injected journal sync failure"));
        }
        Ok(())
    }

    fn sync_parent(&self, _path: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn atomic_replace(&self, _temporary: &Path, _destination: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn delete(&self, _path: &Path) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct ClearFailState {
    current: Option<Vec<u8>>,
    temporary: Option<Vec<u8>>,
    syncs: usize,
    fail_sync_at: Option<usize>,
}

struct ClearFailJournalIo {
    state: Arc<Mutex<ClearFailState>>,
}

impl JournalIo for ClearFailJournalIo {
    fn read(&self, _path: &Path) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.state.lock().unwrap().current.clone())
    }

    fn write_temp(&self, _path: &Path, contents: &[u8]) -> std::io::Result<PathBuf> {
        self.state.lock().unwrap().temporary = Some(contents.to_vec());
        Ok(PathBuf::from("journal.tmp"))
    }

    fn sync(&self, _path: &Path) -> std::io::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.syncs += 1;
        if state.fail_sync_at == Some(state.syncs) {
            return Err(std::io::Error::other("injected journal clear sync failure"));
        }
        Ok(())
    }

    fn sync_parent(&self, _path: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn atomic_replace(&self, _temporary: &Path, _destination: &Path) -> std::io::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.current = state.temporary.take();
        Ok(())
    }

    fn delete(&self, _path: &Path) -> std::io::Result<()> {
        self.state.lock().unwrap().temporary = None;
        Ok(())
    }
}

fn run_with_journal(
    request: ExecuteRootsRequest,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
    journal: &JournalStore,
) -> BatchRootResult {
    let progress = NoopProgressReporter;
    run_full(
        request,
        DeletionPolicy::default(),
        io,
        shredder,
        journal,
        &progress,
        &CancellationToken::new(),
    )
}

fn run_full(
    request: ExecuteRootsRequest,
    policy: DeletionPolicy,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
    journal: &JournalStore,
    progress: &dyn ProgressReporter,
    cancel: &CancellationToken,
) -> BatchRootResult {
    execute_roots(request, policy, io, shredder, journal, progress, cancel)
}

#[test]
fn rejects_unsafe_roots_before_opening_or_mutation() {
    let io = FakeIo::new();
    let shredder = FakeShredder::new();
    let roots = [
        root_request("relative", Path::new("relative"), TargetKind::Directory),
        root_request("filesystem", Path::new("/"), TargetKind::Directory),
        root_request(
            "home",
            &std::env::home_dir().unwrap(),
            TargetKind::Directory,
        ),
        root_request(
            "outside",
            Path::new("/definitely/outside/home"),
            TargetKind::Directory,
        ),
    ];

    let result = run(
        ExecuteRootsRequest {
            roots: roots.to_vec(),
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots.len(), roots.len());
    assert!(result
        .roots
        .iter()
        .all(|root| root.status == RootStatus::Failed));
    assert_eq!(io.events().lock().unwrap().root_opens, 0);
    assert!(shredder.calls.lock().unwrap().is_empty());
}

#[test]
fn completes_batch_preflight_before_any_mutation() {
    let first = home_child("task7-first");
    let second = home_child("task7-second");
    let io = FakeIo::new()
        .root(first.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .root(second.clone(), 3, directory(3, vec![]))
        .fail_directory(3);
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("first", &first, TargetKind::Directory),
                root_request("second", &second, TargetKind::Directory),
            ],
        },
        &io,
        &shredder,
    );

    assert!(result
        .roots
        .iter()
        .any(|root| root.status == RootStatus::Failed));
    assert!(shredder.calls.lock().unwrap().is_empty());
    assert!(io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .all(|call| call != "rename" && call != "unlink" && call != "remove_dir"));
}

#[test]
fn discovered_links_are_unlink_only() {
    let root = home_child("task7-links");
    let io = FakeIo::new()
        .root(
            root.clone(),
            1,
            directory(1, vec![(2, "link"), (3, "file")]),
        )
        .add_node(
            2,
            FakeNode {
                identity: NodeIdentity::link(2, 1),
                kind: NodeKind::Link,
                children: Vec::new(),
            },
        )
        .add_node(3, regular(3));
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![root_request("links", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(shredder.calls.lock().unwrap().len(), 1);
    let events = io.events();
    let events = events.lock().unwrap();
    assert_eq!(
        events.calls.iter().filter(|call| *call == "unlink").count(),
        2
    );
    assert_eq!(
        events
            .calls
            .iter()
            .filter(|call| *call == "open_regular")
            .count(),
        1
    );
}

#[test]
fn child_failure_reports_issue_and_later_roots_still_process() {
    let first = home_child("task7-failing");
    let second = home_child("task7-later");
    let io = FakeIo::new()
        .root(first.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .root(second.clone(), 3, directory(3, vec![(4, "file")]))
        .add_node(4, regular(4));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::NotStarted);

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("failing", &first, TargetKind::Directory),
                root_request("later", &second, TargetKind::Directory),
            ],
        },
        &io,
        &shredder,
    );

    // Per-file outcome issues never stop the batch (ora-2 amendment 2): the
    // first root reports the intact-target issue, and the later root is
    // still processed to completion.
    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert!(result.roots[1].root_removed);
    assert!(result.roots[0]
        .errors
        .iter()
        .any(|error| error.message.contains("injected child failure")));
    assert!(result.roots[0]
        .errors
        .iter()
        .all(|error| error.stage == ExecutionStage::Overwrite));
    assert!(result.roots[0]
        .errors
        .iter()
        .all(|error| !error.message.contains("irreversible partial destruction")));
    let events = io.events();
    let events = events.lock().unwrap();
    // The intact target is never renamed or unlinked; only the second root's
    // file goes through the cleanup lifecycle. The walk continued, so the
    // second root's file was opened and destroyed.
    assert_eq!(
        events.calls.iter().filter(|call| *call == "rename").count(),
        1
    );
    assert_eq!(
        events.calls.iter().filter(|call| *call == "unlink").count(),
        1
    );
    assert_eq!(
        events
            .calls
            .iter()
            .filter(|call| *call == "open_regular")
            .count(),
        2
    );
}

#[test]
fn rejects_parent_child_root_overlap_before_mutation() {
    let parent = home_child("task7-parent");
    let child = parent.join("child");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(child.clone(), 2, regular(2));
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("parent", &parent, TargetKind::Directory),
                root_request("child", &child, TargetKind::File),
            ],
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots[1].status, RootStatus::Failed);
    assert!(shredder.calls.lock().unwrap().is_empty());
    assert!(!io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink" || call == "remove_dir"));
}

#[test]
fn rejects_duplicate_node_identities_before_mutation() {
    let first = home_child("task7-duplicate-first");
    let second = home_child("task7-duplicate-second");
    let io = FakeIo::new()
        .root(first.clone(), 1, regular(42))
        .root(second.clone(), 2, regular(42));
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("first", &first, TargetKind::File),
                root_request("second", &second, TargetKind::File),
            ],
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots[1].status, RootStatus::Failed);
    assert!(shredder.calls.lock().unwrap().is_empty());
    assert!(!io
        .events()
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink"));
}

#[test]
fn rejects_special_files_mount_crossings_and_depth_overflow() {
    let special = home_child("task7-special");
    let mount = home_child("task7-mount");
    let deep = home_child("task7-deep");
    let mut io = FakeIo::new()
        .root(
            special.clone(),
            1,
            FakeNode {
                identity: NodeIdentity::special(1, 1),
                kind: NodeKind::Special,
                children: Vec::new(),
            },
        )
        .root(mount.clone(), 100, directory(100, vec![(101, "mounted")]))
        .add_node(
            101,
            FakeNode {
                identity: NodeIdentity::directory(101, 2),
                kind: NodeKind::Directory,
                children: Vec::new(),
            },
        )
        .root(deep.clone(), 200, directory(200, vec![(201, "level")]))
        .add_node(201, directory(201, vec![(202, "level")]));
    for handle in 202..=251 {
        io = io.add_node(
            handle,
            directory(handle as u128, vec![(handle + 1, "level")]),
        );
    }
    io = io.add_node(252, regular(252));

    let shredder = FakeShredder::new();
    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("special", &special, TargetKind::UnknownLegacy),
                root_request("mount", &mount, TargetKind::Directory),
                root_request("deep", &deep, TargetKind::Directory),
            ],
        },
        &io,
        &shredder,
    );

    assert!(result
        .roots
        .iter()
        .all(|root| root.status == RootStatus::Failed));
    assert!(shredder.calls.lock().unwrap().is_empty());
}

#[test]
fn policy_file_shredder_rejects_directory_identity() {
    let adapter =
        PolicyFileShredder::new(DeletionPolicy::default(), Arc::new(NoopProgressReporter));
    let request = FileShredRequest::new(
        home_child("task7-directory-handle"),
        DeletionPolicy::default(),
    );
    let error = adapter
        .shred_open_file(
            tempfile::tempfile().unwrap(),
            NodeIdentity::directory(1, 1),
            &request,
        )
        .expect_err("directory identity must be rejected");

    assert!(matches!(error, ShredError::ValidationFailed(_)));
}

/// The M6 execution-time recheck must refuse an already-open handle whose
/// link count grew past 1 (a link created after preflight), without ever
/// reopening by path, and must leave both names byte-for-byte untouched.
#[test]
fn policy_file_shredder_rechecks_hard_links_on_open_handle() {
    let fixture = TempHomeDir::new("openhandle");
    let target = fixture.path().join("target.txt");
    std::fs::write(&target, b"payload").expect("write fixture");
    let sibling = fixture.path().join("sibling.txt");
    if let Err(error) = std::fs::hard_link(&target, &sibling) {
        // Filesystems without hard-link support skip cleanly.
        eprintln!("skipping hard-link fixture: {error}");
        return;
    }

    let file = File::open(&target).expect("open target");
    let adapter =
        PolicyFileShredder::new(DeletionPolicy::default(), Arc::new(NoopProgressReporter));
    let request = FileShredRequest::new(target.clone(), DeletionPolicy::default());
    let error = adapter
        .shred_open_file(file, NodeIdentity::regular(1, 1), &request)
        .expect_err("hard-linked open handle must be blocked");

    assert!(matches!(error, ShredError::HardLinkBlocked { .. }));
    assert_eq!(
        std::fs::read(&target).expect("target readable"),
        b"payload",
        "selected name must be untouched"
    );
    assert_eq!(
        std::fs::read(&sibling).expect("sibling readable"),
        b"payload",
        "sibling name must be untouched"
    );
}

#[test]
fn write_check_failure_does_not_prevent_removal() {
    let parent = home_child("task22-check-failure");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::CompletedCheckFailed);
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("check-failure", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &journal,
    );

    // Removal continues after a failed write check (M2 rule 3): the entry is
    // gone, but the root is not a clean Destroyed and the failed check is
    // surfaced with a Verify-stage error.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.write_check, WriteCheckOutcome::Failed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Verify && error.error_type == "write_check_failed"
    }));
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(events.calls.iter().any(|call| call == "rename"));
    assert!(events.calls.iter().any(|call| call == "unlink"));
    assert!(journal.read().expect("journal read").is_empty());
}

#[test]
fn not_started_failure_leaves_target_intact() {
    let parent = home_child("task22-not-started");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::NotStarted);
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("not-started", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &journal,
    );

    // A NotStarted failure means no byte was written (M2 rule 1): no
    // journal/rename/unlink, the root reports the Overwrite-stage issue, and
    // the entry is not removed.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 0);
    assert_eq!(root_result.write_check, WriteCheckOutcome::NotRun);
    assert_eq!(shredder.calls.lock().unwrap().len(), 1);
    assert!(root_result
        .errors
        .iter()
        .any(|error| error.stage == ExecutionStage::Overwrite));
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "rename"));
    assert!(!events.calls.iter().any(|call| call == "unlink"));
    assert!(journal.read().expect("journal read").is_empty());
}

#[test]
fn partial_failure_cleans_up_current_file() {
    let parent = home_child("task22-partial");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::Partial);
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("partial", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &journal,
    );

    // The partially overwritten file still completes its destructive
    // lifecycle (M2 rule 2): rename/unlink run, the partial-destruction
    // issue is preserved with the Overwrite stage, and the root is never a
    // clean Destroyed.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.write_check, WriteCheckOutcome::NotRun);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Overwrite
            && error.message.contains("injected partial shredding")
    }));
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(events.calls.iter().any(|call| call == "rename"));
    assert!(events.calls.iter().any(|call| call == "unlink"));
}

#[test]
fn root_with_file_issues_is_failed_not_destroyed_but_removed_and_batch_continues() {
    let first_parent = home_child("task22-issue-root-parent");
    let first = first_parent.join("file");
    let second_parent = home_child("task22-later-root-parent");
    let second = second_parent.join("file");
    let io = FakeIo::new()
        .root(first_parent.clone(), 1, directory(1, vec![]))
        .root(first.clone(), 2, regular(2))
        .root(second_parent.clone(), 3, directory(3, vec![]))
        .root(second.clone(), 4, regular(4));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::CompletedCheckFailed);

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("issue", &first, TargetKind::File),
                root_request("later", &second, TargetKind::File),
            ],
        },
        &io,
        &shredder,
    );

    // The first root's entry was removed (write-check failure does not stop
    // removal) but the root is Failed, not Destroyed (ora-2 amendment 2) —
    // and the batch continues: the later root still executes.
    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(result.roots[0].root_removed);
    assert_eq!(result.roots[0].write_check, WriteCheckOutcome::Failed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert!(result.roots[1].root_removed);
    assert_eq!(result.roots[1].write_check, WriteCheckOutcome::Passed);
    assert_eq!(
        io.events()
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|call| *call == "open_regular")
            .count(),
        2
    );
}

#[test]
fn root_result_aggregates_write_check() {
    let alpha = home_child("task22-aggregate-alpha");
    let beta = home_child("task22-aggregate-beta");
    let gamma = home_child("task22-aggregate-gamma");
    let io = FakeIo::new()
        .root(
            alpha.clone(),
            1,
            directory(1, vec![(2, "failed"), (3, "passed")]),
        )
        .add_node(2, regular(2))
        .add_node(3, regular(3))
        .root(
            beta.clone(),
            4,
            directory(4, vec![(5, "passed-a"), (6, "passed-b")]),
        )
        .add_node(5, regular(5))
        .add_node(6, regular(6))
        .root(
            gamma.clone(),
            7,
            directory(7, vec![(8, "nocheck-a"), (9, "nocheck-b")]),
        )
        .add_node(8, regular(8))
        .add_node(9, regular(9));
    let shredder = FakeShredder::new().outcomes(vec![
        FakeShredOutcome::CompletedCheckFailed,
        FakeShredOutcome::Completed,
        FakeShredOutcome::Completed,
        FakeShredOutcome::Completed,
        FakeShredOutcome::CompletedNoCheck,
        FakeShredOutcome::CompletedNoCheck,
    ]);

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("alpha", &alpha, TargetKind::Directory),
                root_request("beta", &beta, TargetKind::Directory),
                root_request("gamma", &gamma, TargetKind::Directory),
            ],
        },
        &io,
        &shredder,
    );

    // Aggregate rule: Failed if any file failed, else Passed if any file
    // passed, else NotRun.
    assert_eq!(result.roots[0].write_check, WriteCheckOutcome::Failed);
    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert_eq!(result.roots[1].write_check, WriteCheckOutcome::Passed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert_eq!(result.roots[2].write_check, WriteCheckOutcome::NotRun);
    assert_eq!(result.roots[2].status, RootStatus::Destroyed);
}
