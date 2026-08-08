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
    BatchRootResult, DeletionMethod, DeletionPolicy, ExecuteRootRequest, ExecuteRootsRequest,
    ExecutionStage, MediaType, RootStatus, ShredResult, TargetKind, WriteCheck, WriteCheckOutcome,
};
use crate::shredder::PolicyFileShredder;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    link_counts: HashMap<u64, u64>,
    regular_file_size: Option<u64>,
    fail_enumerate: HashSet<u64>,
    fail_open: HashSet<u64>,
    fail_rename: bool,
    fail_rollback: bool,
    fail_unlink: bool,
    fail_sync: bool,
    media_by_path: HashMap<PathBuf, MediaType>,
    classify_error: bool,
    classify_calls: Arc<AtomicUsize>,
    cancel_token: Option<CancellationToken>,
    cancel_on_nth_event: Option<(String, usize)>,
    events: Arc<Mutex<FakeEvents>>,
}

impl FakeIo {
    fn new() -> Self {
        Self {
            roots: HashMap::new(),
            nodes: HashMap::new(),
            link_counts: HashMap::new(),
            regular_file_size: None,
            fail_enumerate: HashSet::new(),
            fail_open: HashSet::new(),
            fail_rename: false,
            fail_rollback: false,
            fail_unlink: false,
            fail_sync: false,
            media_by_path: HashMap::new(),
            classify_error: false,
            classify_calls: Arc::new(AtomicUsize::new(0)),
            cancel_token: None,
            cancel_on_nth_event: None,
            events: Arc::new(Mutex::new(FakeEvents::default())),
        }
    }

    /// Cancel the operation when the `nth` occurrence of `event` is recorded
    /// (counted from 1, after the current call's bookkeeping starts). Used to
    /// inject stop-after-current-file cancellations at precise walk points.
    fn cancel_on_nth(mut self, event: &str, nth: usize, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self.cancel_on_nth_event = Some((event.to_string(), nth));
        self
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

    /// Hard-link count for a node handle (default 1).
    fn link_count(mut self, handle: u64, count: u64) -> Self {
        self.link_counts.insert(handle, count);
        self
    }

    /// Byte length of the real temp file handed to the shredder (default 0 —
    /// the vacuous zero-length path).
    fn regular_file_size(mut self, size: u64) -> Self {
        self.regular_file_size = Some(size);
        self
    }

    /// Media type the fake classifier reports for a path (default Unknown).
    fn media(mut self, path: PathBuf, media: MediaType) -> Self {
        self.media_by_path.insert(path, media);
        self
    }

    /// The fake classifier returns Err for every path (fail-closed tests).
    fn classify_error(mut self) -> Self {
        self.classify_error = true;
        self
    }

    /// Classifier matching this FakeIo's media map; increments a shared
    /// counter so tests can assert per-distinct-volume call counts (M7).
    fn classifier(&self) -> impl Fn(&Path) -> Result<MediaType, ShredError> {
        let classify_calls = Arc::clone(&self.classify_calls);
        let media = self.media_by_path.clone();
        let classify_error = self.classify_error;
        move |path: &Path| {
            classify_calls.fetch_add(1, Ordering::SeqCst);
            if classify_error {
                return Err(ShredError::ValidationFailed(format!(
                    "injected classifier failure at {}",
                    path.display()
                )));
            }
            Ok(media.get(path).copied().unwrap_or(MediaType::Unknown))
        }
    }

    fn classify_calls(&self) -> usize {
        self.classify_calls.load(Ordering::SeqCst)
    }

    fn fail_directory(mut self, handle: u64) -> Self {
        self.fail_enumerate.insert(handle);
        self
    }

    /// `open_regular_for_shred` returns Err for the given node handle only,
    /// so a single run can prove that one file's open failure does not stop
    /// later files.
    fn fail_open(mut self, handle: u64) -> Self {
        self.fail_open.insert(handle);
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
        let mut events = self.events.lock().unwrap();
        if let Some((target, nth)) = &self.cancel_on_nth_event {
            if target == event
                && events.calls.iter().filter(|call| *call == event).count() + 1 == *nth
            {
                if let Some(token) = &self.cancel_token {
                    token.cancel();
                }
            }
        }
        events.calls.push(event.to_string());
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

    fn link_count(&self, node: &NodeHandle) -> Result<u64, ShredError> {
        self.record("link_count");
        Ok(self.link_counts.get(&node.id()).copied().unwrap_or(1))
    }

    fn open_regular_for_shred(&self, node: &NodeHandle) -> Result<File, ShredError> {
        self.record("open_regular");
        if self.fail_open.contains(&node.id()) {
            return Err(ShredError::IoError {
                path: PathBuf::from("fake"),
                kind: "injected".to_string(),
                message: "injected open failure".to_string(),
            });
        }
        if self.get_node(node)?.kind != NodeKind::RegularFile {
            return Err(ShredError::ValidationFailed(
                "fake adapter opened a non-regular node".to_string(),
            ));
        }
        let file = tempfile::tempfile()
            .map_err(|error| ShredError::from_io_error(PathBuf::from("fake"), error))?;
        if let Some(size) = self.regular_file_size {
            file.set_len(size)
                .map_err(|error| ShredError::from_io_error(PathBuf::from("fake"), error))?;
        }
        Ok(file)
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
    /// Partial overwrite after a real post-write I/O error: cleanup still
    /// proceeds and the original issue remains visible.
    PartialPostWriteError,
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
            FakeShredOutcome::PartialPostWriteError => Ok(FileShredResult {
                overwrite_state: OverwriteState::Partial,
                write_check_status: WriteCheckOutcome::NotRun,
                bytes_shredded: 7,
                issues: vec![ShredError::IoError {
                    path,
                    kind: "injected_post_write".to_string(),
                    message: "injected post-write failure".to_string(),
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

/// Regular file on an explicit mount id, for multi-volume fixtures (M7).
fn regular_on(identity: u128, mount_id: u64) -> FakeNode {
    FakeNode {
        identity: NodeIdentity::regular(identity, mount_id),
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

/// Directory on an explicit mount id, for multi-volume fixtures (M7).
fn directory_on(identity: u128, mount_id: u64, children: Vec<(u64, &str)>) -> FakeNode {
    FakeNode {
        identity: NodeIdentity::directory(identity, mount_id),
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
    // Default policy is Automatic, which never classifies (M7).
    let classify = |_path: &Path| -> Result<MediaType, ShredError> { Ok(MediaType::Unknown) };
    run_full(
        request,
        DeletionPolicy::default(),
        io,
        shredder,
        &journal,
        &progress,
        &CancellationToken::new(),
        &classify,
    )
}

/// Run a batch with the Legacy 3-pass policy (HDD-only, M7) and the given
/// classifier.
fn run_legacy(
    request: ExecuteRootsRequest,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
    classify: &dyn Fn(&Path) -> Result<MediaType, ShredError>,
) -> BatchRootResult {
    let progress = NoopProgressReporter;
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    run_full(
        request,
        DeletionPolicy {
            method: DeletionMethod::LegacyThreePass,
            write_check: WriteCheck::Off,
        },
        io,
        shredder,
        &journal,
        &progress,
        &CancellationToken::new(),
        classify,
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

/// Journal IO that records every operation it performs while tracking the
/// journal contents like the real store, so tests can prove the append →
/// ... → clear lifecycle without depending on journal internals.
struct RecordingJournalState {
    ops: Vec<String>,
    current: Option<Vec<u8>>,
    temporary: Option<Vec<u8>>,
}

struct RecordingJournalIo {
    state: Arc<Mutex<RecordingJournalState>>,
}

impl RecordingJournalIo {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingJournalState {
                ops: Vec::new(),
                current: None,
                temporary: None,
            })),
        }
    }

    fn ops(&self) -> Vec<String> {
        self.state.lock().unwrap().ops.clone()
    }
}

impl JournalIo for RecordingJournalIo {
    fn read(&self, _path: &Path) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.state.lock().unwrap().current.clone())
    }

    fn write_temp(&self, _path: &Path, contents: &[u8]) -> std::io::Result<PathBuf> {
        let mut state = self.state.lock().unwrap();
        state.ops.push("write_temp".to_string());
        state.temporary = Some(contents.to_vec());
        Ok(PathBuf::from("journal.tmp"))
    }

    fn sync(&self, _path: &Path) -> std::io::Result<()> {
        self.state.lock().unwrap().ops.push("sync".to_string());
        Ok(())
    }

    fn sync_parent(&self, _path: &Path) -> std::io::Result<()> {
        self.state
            .lock()
            .unwrap()
            .ops
            .push("sync_parent".to_string());
        Ok(())
    }

    fn atomic_replace(&self, _temporary: &Path, _destination: &Path) -> std::io::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.ops.push("atomic_replace".to_string());
        state.current = state.temporary.take();
        Ok(())
    }

    fn delete(&self, _path: &Path) -> std::io::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.ops.push("delete".to_string());
        state.temporary = None;
        Ok(())
    }
}

/// Progress reporter that records every call, for pass-local progress
/// invariant assertions (M5/D8).
#[derive(Default)]
struct RecordingProgress {
    file_starts: Mutex<Vec<u64>>,
    pass_starts: Mutex<Vec<(u32, u32)>>,
    pass_completes: Mutex<Vec<(u32, u32)>>,
    progress_events: Mutex<Vec<(u64, u64)>>,
    /// (path, bytes_written, passes_completed, total_passes)
    file_completes: Mutex<Vec<(PathBuf, u64, u32, u32)>>,
}

impl ProgressReporter for RecordingProgress {
    fn on_file_start(&self, _path: &Path, file_size: u64) {
        self.file_starts.lock().unwrap().push(file_size);
    }

    fn on_pass_start(&self, pass: u32, total_passes: u32) {
        self.pass_starts.lock().unwrap().push((pass, total_passes));
    }

    fn on_progress(&self, bytes_written: u64, total: u64) {
        self.progress_events
            .lock()
            .unwrap()
            .push((bytes_written, total));
    }

    fn on_pass_complete(&self, pass: u32, total_passes: u32) {
        self.pass_completes
            .lock()
            .unwrap()
            .push((pass, total_passes));
    }

    fn on_file_complete(&self, path: &Path, result: &ShredResult, total_passes: u32) {
        self.file_completes.lock().unwrap().push((
            path.to_path_buf(),
            result.bytes_written,
            result.passes_completed,
            total_passes,
        ));
    }

    fn on_error(&self, _path: &Path, _error: &ShredError) {}

    fn on_warning(&self, _path: &Path, _message: &str) {}
}

fn run_with_journal(
    request: ExecuteRootsRequest,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
    journal: &JournalStore,
) -> BatchRootResult {
    let progress = NoopProgressReporter;
    // Default policy is Automatic, which never classifies (M7).
    let classify = |_path: &Path| -> Result<MediaType, ShredError> { Ok(MediaType::Unknown) };
    run_full(
        request,
        DeletionPolicy::default(),
        io,
        shredder,
        journal,
        &progress,
        &CancellationToken::new(),
        &classify,
    )
}

/// Classifier for Automatic-policy tests: never invoked (M7 — Automatic
/// performs zero media classification).
fn automatic_classifier() -> impl Fn(&Path) -> Result<MediaType, ShredError> {
    |_path: &Path| Ok(MediaType::Unknown)
}

/// Test harness wrapper over `execute_roots`; grew with the M7
/// `classify_media` callback.
#[allow(clippy::too_many_arguments)]
fn run_full(
    request: ExecuteRootsRequest,
    policy: DeletionPolicy,
    io: &dyn SecureTreeIo,
    shredder: &dyn OpenFileShredder,
    journal: &JournalStore,
    progress: &dyn ProgressReporter,
    cancel: &CancellationToken,
    classify: &dyn Fn(&Path) -> Result<MediaType, ShredError>,
) -> BatchRootResult {
    execute_roots(
        request, policy, io, shredder, journal, progress, cancel, classify,
    )
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
    let request = FileShredRequest::new(home_child("task7-directory-handle"));
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
    let request = FileShredRequest::new(target.clone());
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
fn open_failure_is_per_file_issue_and_batch_continues() {
    let first_parent = home_child("task30-open-failure-parent");
    let first = first_parent.join("file");
    let second_parent = home_child("task30-open-later-parent");
    let second = second_parent.join("file");
    let io = FakeIo::new()
        .root(first_parent.clone(), 1, directory(1, vec![]))
        .root(first.clone(), 2, regular(2))
        .root(second_parent.clone(), 3, directory(3, vec![]))
        .root(second.clone(), 4, regular(4))
        .fail_open(2);
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![
                root_request("open-failure", &first, TargetKind::File),
                root_request("later", &second, TargetKind::File),
            ],
        },
        &io,
        &shredder,
    );

    // An open failure is a per-file issue (ORACLE-2 SHOULD-FIX 1), not a
    // batch abort: file A could not be opened before any byte was written —
    // the Overwrite-stage issue is reported, A is not removed, the walk
    // continues, and file B is still processed. The root finish() downgrades
    // to Failed because the issue is visible.
    let first_result = &result.roots[0];
    assert_eq!(first_result.status, RootStatus::Failed);
    assert!(!first_result.root_removed);
    assert_eq!(first_result.files_destroyed, 0);
    assert!(first_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Overwrite && error.message.contains("injected open failure")
    }));
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert!(result.roots[1].root_removed);
    assert_eq!(result.roots[1].files_destroyed, 1);
    let events = io.events();
    let events = events.lock().unwrap();
    // Both files were opened; only B went through the cleanup lifecycle.
    assert_eq!(
        events
            .calls
            .iter()
            .filter(|call| *call == "open_regular")
            .count(),
        2
    );
    assert_eq!(
        events.calls.iter().filter(|call| *call == "rename").count(),
        1
    );
    assert_eq!(
        events.calls.iter().filter(|call| *call == "unlink").count(),
        1
    );
    // The shredder only ever saw B: A never produced an open handle.
    let calls = shredder.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].diagnostic_path(), second);
}

#[test]
fn post_write_error_yields_partial_and_cleans_up_current_file() {
    let parent = home_child("task22-partial");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let shredder = FakeShredder::new().outcome(FakeShredOutcome::PartialPostWriteError);
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal_io = Arc::new(RecordingJournalIo::new());
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::clone(&journal_io) as Arc<dyn JournalIo>,
    );

    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("partial", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &journal,
    );

    // The partially overwritten file still completes its destructive
    // lifecycle (M2 rule 2): rename/unlink run, the original post-write
    // issue is preserved with the Overwrite stage, and the root is never a
    // clean Destroyed.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.write_check, WriteCheckOutcome::NotRun);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Overwrite
            && error.message.contains("injected post-write failure")
    }));
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(events.calls.iter().any(|call| call == "rename"));
    assert!(events.calls.iter().any(|call| call == "unlink"));
    assert_eq!(
        journal_io
            .ops()
            .iter()
            .filter(|op| *op == "write_temp")
            .count(),
        2,
        "the partial file must append and clear its journal entry"
    );
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

#[test]
fn hard_linked_file_root_blocked_before_overwrite() {
    let parent = home_child("task23-hardlink-root");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .link_count(2, 2);
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![root_request("hard-link", &root, TargetKind::File)],
        },
        &io,
        &shredder,
    );

    // M6 preflight: the hard-linked file root is blocked before the file is
    // ever opened or renamed.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight && error.error_type == "hard_link_blocked"
    }));
    assert!(shredder.calls.lock().unwrap().is_empty());
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "open_regular"));
    assert!(!events.calls.iter().any(|call| call == "rename"));
}

#[test]
fn hard_linked_file_inside_directory_root_blocks_before_mutation() {
    let root = home_child("task23-hardlink-inside");
    let io = FakeIo::new()
        .root(
            root.clone(),
            1,
            directory(1, vec![(2, "linked"), (3, "clean")]),
        )
        .add_node(2, regular(2))
        .add_node(3, regular(3))
        .link_count(2, 2);
    let shredder = FakeShredder::new();

    let result = run(
        ExecuteRootsRequest {
            roots: vec![root_request("dir", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
    );

    // Any hard-linked regular file inside a directory root fails the WHOLE
    // batch preflight before any mutation (M6).
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight && error.error_type == "hard_link_blocked"
    }));
    assert!(shredder.calls.lock().unwrap().is_empty());
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "open_regular"));
    assert!(!events
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink" || call == "remove_dir"));
}

/// Real-filesystem proof (unix): a file root with a sibling hard link is
/// blocked in preflight, and BOTH names keep their bytes — the selected file
/// is never overwritten. Skips cleanly where hard links are unsupported.
#[cfg(unix)]
#[test]
fn sibling_hard_link_unchanged_after_block() {
    use crate::shredder::root_execution::unix::UnixSecureTreeIo;

    let fixture = TempHomeDir::new("hardlink-unix");
    let selected = fixture.path().join("selected.txt");
    std::fs::write(&selected, b"sensitive payload").expect("write fixture");
    let sibling = fixture.path().join("sibling.txt");
    if let Err(error) = std::fs::hard_link(&selected, &sibling) {
        // Filesystems without hard-link support skip cleanly.
        eprintln!("skipping hard-link fixture: {error}");
        return;
    }

    let adapter = UnixSecureTreeIo::new();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("hard-link", &selected, TargetKind::File)],
        },
        &adapter,
        &shredder,
        &journal,
    );

    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight && error.error_type == "hard_link_blocked"
    }));
    assert_eq!(
        std::fs::read(&selected).expect("selected readable"),
        b"sensitive payload",
        "selected name must be untouched"
    );
    assert_eq!(
        std::fs::read(&sibling).expect("sibling readable"),
        b"sensitive payload",
        "sibling name must be untouched"
    );
    assert!(shredder.calls.lock().unwrap().is_empty());
}

/// Real-filesystem proof (windows): same invariant as the unix-gated test,
/// with a runtime skip when hard-link creation fails (e.g. non-NTFS volume).
#[cfg(windows)]
#[test]
fn sibling_hard_link_unchanged_after_block() {
    use crate::shredder::root_execution::windows::WindowsSecureTreeIo;

    let fixture = TempHomeDir::new("hardlink-win");
    let selected = fixture.path().join("selected.txt");
    std::fs::write(&selected, b"sensitive payload").expect("write fixture");
    let sibling = fixture.path().join("sibling.txt");
    if let Err(error) = std::fs::hard_link(&selected, &sibling) {
        // Non-NTFS volumes without hard-link support skip cleanly.
        eprintln!("skipping hard-link fixture: {error}");
        return;
    }

    let adapter = WindowsSecureTreeIo::new();
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let result = run_with_journal(
        ExecuteRootsRequest {
            roots: vec![root_request("hard-link", &selected, TargetKind::File)],
        },
        &adapter,
        &shredder,
        &journal,
    );

    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight && error.error_type == "hard_link_blocked"
    }));
    assert_eq!(
        std::fs::read(&selected).expect("selected readable"),
        b"sensitive payload",
        "selected name must be untouched"
    );
    assert_eq!(
        std::fs::read(&sibling).expect("sibling readable"),
        b"sensitive payload",
        "sibling name must be untouched"
    );
    assert!(shredder.calls.lock().unwrap().is_empty());
}

#[test]
fn stop_during_file_a_completes_a_skips_b_c() {
    let root = home_child("task24-stop-during-a");
    let io = FakeIo::new()
        .root(
            root.clone(),
            1,
            directory(1, vec![(2, "a"), (3, "b"), (4, "c")]),
        )
        .add_node(2, regular(2))
        .add_node(3, regular(3))
        .add_node(4, regular(4));
    let cancel = CancellationToken::new();
    // Cancel as A is opened: A still completes its destructive lifecycle,
    // then the walk stops at the next node boundary.
    let io = io.cancel_on_nth("open_regular", 1, cancel.clone());
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal_io = Arc::new(RecordingJournalIo::new());
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::clone(&journal_io) as Arc<dyn JournalIo>,
    );

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("stop", &root, TargetKind::Directory)],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        &NoopProgressReporter,
        &cancel,
        &automatic_classifier(),
    );

    // Stop-after-current-file (D7): A completed its full cleanup, B and C
    // were never opened, the root is Cancelled with A's counters preserved,
    // and no error or partial-destruction augmentation is reported.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Cancelled);
    assert!(!root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.bytes_shredded, 7);
    assert!(root_result.errors.is_empty());
    let shred_calls = shredder.calls.lock().unwrap();
    assert_eq!(shred_calls.len(), 1, "only A may start overwrite");
    assert_eq!(shred_calls[0].diagnostic_path(), root.join("a"));
    let events = io.events();
    let events = events.lock().unwrap();
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
        1
    );
    assert_eq!(
        events
            .calls
            .iter()
            .filter(|call| *call == "remove_dir")
            .count(),
        0,
        "the cancelled root must not begin directory cleanup"
    );
    let journal_ops = journal_io.ops();
    assert_eq!(
        journal_ops.iter().filter(|op| *op == "write_temp").count(),
        2,
        "A must append and clear exactly one journal entry"
    );
}

#[test]
fn stop_after_sole_child_completes_prevents_parent_directory_removal() {
    let root = home_child("task24-stop-after-sole-child");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "only")]))
        .add_node(2, regular(2));
    let cancel = CancellationToken::new();
    let io = io.cancel_on_nth("open_regular", 1, cancel.clone());
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("stop", &root, TargetKind::Directory)],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        &NoopProgressReporter,
        &cancel,
        &automatic_classifier(),
    );

    // The sole child completes its configured lifecycle, but cancellation at
    // the following directory boundary prevents the parent from being removed.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Cancelled);
    assert!(!root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.directories_removed, 0);
    assert!(root_result.errors.is_empty());
    assert_eq!(shredder.calls.lock().unwrap().len(), 1);
    let events = io.events();
    let events = events.lock().unwrap();
    assert_eq!(
        events.calls.iter().filter(|call| *call == "rename").count(),
        1,
        "the active file must complete cleanup"
    );
    assert_eq!(
        events.calls.iter().filter(|call| *call == "unlink").count(),
        1,
        "the active file must complete cleanup"
    );
    assert_eq!(
        events
            .calls
            .iter()
            .filter(|call| *call == "remove_dir")
            .count(),
        0,
        "the parent directory must remain after stop"
    );
}

#[test]
fn stop_before_root_start_skips_all_destructive_processing() {
    let root = home_child("task24-stop-before-root");
    let io = FakeIo::new()
        .root(root.clone(), 1, directory(1, vec![(2, "a")]))
        .add_node(2, regular(2));
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal_io = Arc::new(RecordingJournalIo::new());
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::clone(&journal_io) as Arc<dyn JournalIo>,
    );
    let cancel = CancellationToken::new();
    cancel.cancel();

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("stop", &root, TargetKind::Directory)],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        &NoopProgressReporter,
        &cancel,
        &automatic_classifier(),
    );

    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Cancelled);
    assert!(!root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 0);
    assert_eq!(root_result.directories_removed, 0);
    assert_eq!(root_result.bytes_shredded, 0);
    assert!(root_result.errors.is_empty());
    assert!(
        shredder.calls.lock().unwrap().is_empty(),
        "no overwrite may start"
    );
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(
        events
            .calls
            .iter()
            .any(|call| call.starts_with("open_root:")),
        "preflight inspection may still open the root"
    );
    assert!(!events.calls.iter().any(|call| {
        matches!(
            call.as_str(),
            "open_regular" | "rename" | "unlink" | "remove_dir"
        )
    }));
    assert!(journal_io.ops().is_empty(), "no journal append may occur");
}

#[test]
fn stop_between_roots_marks_remaining_cancelled() {
    let first_parent = home_child("task24-between-parent-a");
    let first = first_parent.join("file");
    let second_parent = home_child("task24-between-parent-b");
    let second = second_parent.join("file");
    let io = FakeIo::new()
        .root(first_parent.clone(), 1, directory(1, vec![]))
        .root(first.clone(), 2, regular(2))
        .root(second_parent.clone(), 3, directory(3, vec![]))
        .root(second.clone(), 4, regular(4));
    let cancel = CancellationToken::new();
    // A file root's lifecycle records two sync events: after the rename and
    // after the unlink. Cancelling on the second lets A finish completely
    // before the batch boundary check.
    let io = io.cancel_on_nth("sync", 2, cancel.clone());
    let shredder = FakeShredder::new();
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![
                root_request("first", &first, TargetKind::File),
                root_request("second", &second, TargetKind::File),
            ],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        &NoopProgressReporter,
        &cancel,
        &automatic_classifier(),
    );

    // Root A completed its full lifecycle before the cancel fired and is
    // Destroyed; root B was never started and reports Cancelled.
    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert!(result.roots[0].root_removed);
    assert_eq!(result.roots[1].status, RootStatus::Cancelled);
    assert!(!result.roots[1].root_removed);
    assert_eq!(result.roots[1].bytes_shredded, 0);
    assert_eq!(
        io.events()
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|call| *call == "open_regular")
            .count(),
        1
    );
}

#[test]
fn zero_length_file_root_removed_and_journaled() {
    let parent = home_child("task25-zero-length");
    let root = parent.join("empty");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let journal_io = Arc::new(RecordingJournalIo::new());
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::with_io(
        directory.path().join("journal.json"),
        Arc::clone(&journal_io) as Arc<dyn JournalIo>,
    );
    let progress = Arc::new(RecordingProgress::default());
    let shredder = PolicyFileShredder::new(
        DeletionPolicy::default(),
        Arc::clone(&progress) as Arc<dyn ProgressReporter>,
    );

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("zero", &root, TargetKind::File)],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        progress.as_ref(),
        &CancellationToken::new(),
        &automatic_classifier(),
    );

    // M4 zero-length lifecycle: journal → rename → unlink → sync → clear,
    // with the vacuous Completed/NotRun overwrite outcome.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Destroyed);
    assert!(root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert_eq!(root_result.write_check, WriteCheckOutcome::NotRun);
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(events.calls.iter().any(|call| call == "rename"));
    assert!(events.calls.iter().any(|call| call == "unlink"));
    // The journal was appended and then cleared.
    let ops = journal_io.ops();
    // The journal lifecycle ran: append wrote the record and clear rewrote
    // the journal back to empty — both through the temp-file + atomic
    // replace dance (no `delete` op exists in the durable-rewrite path).
    assert!(
        ops.iter().filter(|op| *op == "write_temp").count() >= 2,
        "append and clear must each write a temporary journal record"
    );
    assert!(
        ops.iter().filter(|op| *op == "atomic_replace").count() >= 2,
        "append and clear must each atomically replace the journal"
    );
    // No pass or progress events for the vacuous overwrite (M5).
    assert!(progress.pass_starts.lock().unwrap().is_empty());
    assert!(progress.pass_completes.lock().unwrap().is_empty());
    assert!(progress.progress_events.lock().unwrap().is_empty());
}

#[test]
fn zero_length_emits_no_pass_events_and_valid_completion() {
    let parent = home_child("task25-zero-progress");
    let root = parent.join("empty");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2));
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let progress = Arc::new(RecordingProgress::default());
    let shredder = PolicyFileShredder::new(
        DeletionPolicy::default(),
        Arc::clone(&progress) as Arc<dyn ProgressReporter>,
    );

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("zero", &root, TargetKind::File)],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        progress.as_ref(),
        &CancellationToken::new(),
        &automatic_classifier(),
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    // The file lifecycle is announced, but no pass exists for zero-length.
    assert_eq!(*progress.file_starts.lock().unwrap(), vec![0]);
    assert!(progress.pass_starts.lock().unwrap().is_empty());
    assert!(progress.pass_completes.lock().unwrap().is_empty());
    assert!(progress.progress_events.lock().unwrap().is_empty());
    // Valid completion event: 0 passes completed, total_passes is the
    // policy's (>= 1, never 0 — D8).
    let completes = progress.file_completes.lock().unwrap();
    assert_eq!(completes.len(), 1);
    assert_eq!(completes[0].0, root);
    assert_eq!(completes[0].1, 0);
    assert_eq!(completes[0].2, 0);
    assert_eq!(completes[0].3, 1);
}

#[test]
fn legacy_progress_never_exceeds_100_percent() {
    let parent = home_child("task25-legacy-progress");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .regular_file_size(2 * 1024 * 1024)
        .media(root.clone(), MediaType::Hdd);
    let policy = DeletionPolicy {
        method: DeletionMethod::LegacyThreePass,
        write_check: WriteCheck::Off,
    };
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let progress = Arc::new(RecordingProgress::default());
    let shredder =
        PolicyFileShredder::new(policy, Arc::clone(&progress) as Arc<dyn ProgressReporter>);
    let classify = io.classifier();

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("legacy", &root, TargetKind::File)],
        },
        policy,
        &io,
        &shredder,
        &journal,
        progress.as_ref(),
        &CancellationToken::new(),
        &classify,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(result.roots[0].bytes_shredded, 3 * 2 * 1024 * 1024);
    // Every pass event carries the full 3-pass total (M5).
    assert_eq!(
        *progress.pass_starts.lock().unwrap(),
        vec![(1, 3), (2, 3), (3, 3)]
    );
    assert_eq!(
        *progress.pass_completes.lock().unwrap(),
        vec![(1, 3), (2, 3), (3, 3)]
    );
    // Pass-local progress: every event is within [0, file_size] — a
    // frontend percent computed from pass + pass-local bytes can never
    // exceed 100%.
    let events = progress.progress_events.lock().unwrap();
    assert!(!events.is_empty(), "2 MiB must produce progress events");
    for &(bytes, total) in events.iter() {
        assert!(bytes <= total, "progress {bytes} exceeds total {total}");
        assert_eq!(
            total,
            2 * 1024 * 1024,
            "pass-local total must equal the file size"
        );
    }
    // The completion event carries the policy pass total, never 0 (D8).
    let completes = progress.file_completes.lock().unwrap();
    assert_eq!(completes.len(), 1);
    assert_eq!(completes[0].0, root);
    assert_eq!(completes[0].1, 6 * 1024 * 1024);
    assert_eq!(completes[0].2, 3);
    assert_eq!(completes[0].3, 3);
}

#[test]
fn automatic_progress_within_bounds() {
    let parent = home_child("task25-automatic-progress");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .regular_file_size(2 * 1024 * 1024);
    let policy = DeletionPolicy {
        method: DeletionMethod::Automatic,
        write_check: WriteCheck::Off,
    };
    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let progress = Arc::new(RecordingProgress::default());
    let shredder =
        PolicyFileShredder::new(policy, Arc::clone(&progress) as Arc<dyn ProgressReporter>);

    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![root_request("automatic", &root, TargetKind::File)],
        },
        policy,
        &io,
        &shredder,
        &journal,
        progress.as_ref(),
        &CancellationToken::new(),
        &automatic_classifier(),
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(*progress.pass_starts.lock().unwrap(), vec![(1, 1)]);
    assert_eq!(*progress.pass_completes.lock().unwrap(), vec![(1, 1)]);
    let events = progress.progress_events.lock().unwrap();
    assert!(!events.is_empty(), "2 MiB must produce progress events");
    for &(bytes, total) in events.iter() {
        assert!(bytes <= total, "progress {bytes} exceeds total {total}");
        assert_eq!(total, 2 * 1024 * 1024);
    }
    let completes = progress.file_completes.lock().unwrap();
    assert_eq!(completes[0].3, 1, "completion total_passes must be 1");
}

#[test]
fn legacy_rejected_on_ssd_root_before_mutation() {
    let parent = home_child("task31-ssd-root-parent");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .media(root.clone(), MediaType::Ssd);
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![root_request("ssd", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &classify,
    );

    // M7: Legacy 3-pass on a confirmed SSD volume fails preflight with the
    // structured storage error before any mutation, and the target is intact.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 0);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight
            && error.error_type == "unsupported_storage_for_method"
    }));
    assert!(shredder.calls.lock().unwrap().is_empty());
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "open_regular"));
    assert!(!events
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink" || call == "remove_dir"));
    assert_eq!(io.classify_calls(), 1);
}

#[test]
fn legacy_allowed_on_hdd_root() {
    let parent = home_child("task31-hdd-root-parent");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .media(root.clone(), MediaType::Hdd);
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![root_request("hdd", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &classify,
    );

    // M7: Legacy 3-pass proceeds to destruction on a confirmed HDD volume.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Destroyed);
    assert!(root_result.root_removed);
    assert_eq!(root_result.files_destroyed, 1);
    assert!(root_result.errors.is_empty());
    assert_eq!(io.classify_calls(), 1);
}

#[test]
fn legacy_rejected_when_classifier_errors() {
    let parent = home_child("task31-classify-error-parent");
    let root = parent.join("file");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(root.clone(), 2, regular(2))
        .classify_error();
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![root_request("classify-error", &root, TargetKind::File)],
        },
        &io,
        &shredder,
        &classify,
    );

    // M7 fail closed: a classifier error blocks the root in preflight even
    // though the media type is unknown — nothing is opened or mutated.
    let root_result = &result.roots[0];
    assert_eq!(root_result.status, RootStatus::Failed);
    assert!(!root_result.root_removed);
    assert!(root_result.errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight
            && error.message.contains("injected classifier failure")
    }));
    assert!(shredder.calls.lock().unwrap().is_empty());
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "open_regular"));
    assert!(!events
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink" || call == "remove_dir"));
    assert_eq!(io.classify_calls(), 1);
}

#[test]
fn mixed_roots_ssd_aborts_whole_batch() {
    let ssd_parent = home_child("task31-mixed-ssd-parent");
    let ssd_root = ssd_parent.join("file");
    let hdd_parent = home_child("task31-mixed-hdd-parent");
    let hdd_root = hdd_parent.join("file");
    let io = FakeIo::new()
        .root(ssd_parent.clone(), 1, directory_on(1, 1, vec![]))
        .root(ssd_root.clone(), 2, regular_on(2, 1))
        .media(ssd_root.clone(), MediaType::Ssd)
        .root(hdd_parent.clone(), 3, directory_on(3, 2, vec![]))
        .root(hdd_root.clone(), 4, regular_on(4, 2))
        .media(hdd_root.clone(), MediaType::Hdd);
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![
                root_request("ssd", &ssd_root, TargetKind::File),
                root_request("hdd", &hdd_root, TargetKind::File),
            ],
        },
        &io,
        &shredder,
        &classify,
    );

    // M7: any non-HDD volume fails the WHOLE batch preflight — the HDD root
    // is skipped, and no mutation event exists for either root.
    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(result.roots[0].errors.iter().any(|error| {
        error.stage == ExecutionStage::Preflight
            && error.error_type == "unsupported_storage_for_method"
    }));
    assert_eq!(result.roots[1].status, RootStatus::Skipped);
    assert!(result.roots[1].errors.is_empty());
    assert!(shredder.calls.lock().unwrap().is_empty());
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "open_regular"));
    assert!(!events
        .calls
        .iter()
        .any(|call| call == "rename" || call == "unlink" || call == "remove_dir"));
    // One classification per distinct volume.
    assert_eq!(io.classify_calls(), 2);
}

#[test]
fn automatic_runs_without_classification() {
    let first_parent = home_child("task31-auto-parent-a");
    let first = first_parent.join("file");
    let second_parent = home_child("task31-auto-parent-b");
    let second = second_parent.join("file");
    let io = FakeIo::new()
        .root(first_parent.clone(), 1, directory_on(1, 1, vec![]))
        .root(first.clone(), 2, regular_on(2, 1))
        .root(second_parent.clone(), 3, directory_on(3, 2, vec![]))
        .root(second.clone(), 4, regular_on(4, 2));
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let directory = tempfile::tempdir().expect("temporary directory");
    let journal = JournalStore::at(directory.path().join("journal.json"));
    let result = run_full(
        ExecuteRootsRequest {
            roots: vec![
                root_request("auto-a", &first, TargetKind::File),
                root_request("auto-b", &second, TargetKind::File),
            ],
        },
        DeletionPolicy::default(),
        &io,
        &shredder,
        &journal,
        &NoopProgressReporter,
        &CancellationToken::new(),
        &classify,
    );

    // M7: Automatic performs zero media classification across multiple
    // roots, and both roots still destroy normally.
    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert_eq!(io.classify_calls(), 0);
}

#[test]
fn classification_cached_per_distinct_mount_id() {
    // Two roots on the SAME volume: one classification for the batch.
    let first_parent = home_child("task31-cache-same-parent-a");
    let first = first_parent.join("file");
    let second_parent = home_child("task31-cache-same-parent-b");
    let second = second_parent.join("file");
    let io = FakeIo::new()
        .root(first_parent.clone(), 1, directory_on(1, 1, vec![]))
        .root(first.clone(), 2, regular_on(2, 1))
        .media(first.clone(), MediaType::Hdd)
        .root(second_parent.clone(), 3, directory_on(3, 1, vec![]))
        .root(second.clone(), 4, regular_on(4, 1))
        .media(second.clone(), MediaType::Hdd);
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![
                root_request("same-a", &first, TargetKind::File),
                root_request("same-b", &second, TargetKind::File),
            ],
        },
        &io,
        &shredder,
        &classify,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert_eq!(io.classify_calls(), 1, "one call per distinct mount id");

    // Two roots on DIFFERENT volumes: one classification per volume.
    let third_parent = home_child("task31-cache-diff-parent-a");
    let third = third_parent.join("file");
    let fourth_parent = home_child("task31-cache-diff-parent-b");
    let fourth = fourth_parent.join("file");
    let io = FakeIo::new()
        .root(third_parent.clone(), 1, directory_on(1, 1, vec![]))
        .root(third.clone(), 2, regular_on(2, 1))
        .media(third.clone(), MediaType::Hdd)
        .root(fourth_parent.clone(), 3, directory_on(3, 2, vec![]))
        .root(fourth.clone(), 4, regular_on(4, 2))
        .media(fourth.clone(), MediaType::Hdd);
    let shredder = FakeShredder::new();
    let classify = io.classifier();

    let result = run_legacy(
        ExecuteRootsRequest {
            roots: vec![
                root_request("diff-a", &third, TargetKind::File),
                root_request("diff-b", &fourth, TargetKind::File),
            ],
        },
        &io,
        &shredder,
        &classify,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(result.roots[1].status, RootStatus::Destroyed);
    assert_eq!(io.classify_calls(), 2, "one call per distinct mount id");
}
