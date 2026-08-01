use super::plan::{
    ChildName, DirHandle, FileShredRequest, FileShredResult, NodeHandle, NodeIdentity, NodeKind,
    RenamedNode,
};
use super::{execute_roots, OpenFileShredder, SecureTreeIo};
use crate::shredder::algorithms::nist_clear::NistClear;
use crate::shredder::cancel::CancellationToken;
use crate::shredder::errors::ShredError;
use crate::shredder::progress::NoopProgressReporter;
use crate::shredder::types::{
    BatchRootResult, ExecuteRootRequest, ExecuteRootsRequest, RootStatus, TargetKind,
    VerificationLevel,
};
use crate::shredder::LegacyOpenFileShredder;
use std::collections::{HashMap, HashSet};
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
}

struct FakeIo {
    roots: HashMap<PathBuf, u64>,
    nodes: HashMap<u64, FakeNode>,
    fail_enumerate: HashSet<u64>,
    events: Arc<Mutex<FakeEvents>>,
}

impl FakeIo {
    fn new() -> Self {
        Self {
            roots: HashMap::new(),
            nodes: HashMap::new(),
            fail_enumerate: HashSet::new(),
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
        self.record("open_root");
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
        self.record("rename");
        Ok(RenamedNode::new())
    }

    fn unlink_leaf(&self, _parent: &DirHandle, _node: &NodeHandle) -> Result<(), ShredError> {
        self.record("unlink");
        Ok(())
    }

    fn remove_empty_dir(&self, _parent: &DirHandle, _node: &NodeHandle) -> Result<(), ShredError> {
        self.record("remove_dir");
        Ok(())
    }

    fn sync_parent(&self, _parent: &DirHandle) -> Result<(), ShredError> {
        self.record("sync");
        Ok(())
    }
}

struct FakeShredder {
    calls: Arc<Mutex<Vec<FileShredRequest>>>,
    fail: bool,
}

impl OpenFileShredder for FakeShredder {
    fn shred_open_file(
        &self,
        _file: File,
        _identity: NodeIdentity,
        request: &FileShredRequest,
    ) -> Result<FileShredResult, ShredError> {
        self.calls.lock().unwrap().push(request.clone());
        if self.fail {
            Err(ShredError::ValidationFailed(
                "injected child failure".to_string(),
            ))
        } else {
            Ok(FileShredResult::success(7))
        }
    }
}

fn home_child(name: &str) -> PathBuf {
    std::env::home_dir().expect("home directory").join(name)
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

fn run(request: ExecuteRootsRequest, io: &FakeIo, shredder: &FakeShredder) -> BatchRootResult {
    let progress = NoopProgressReporter;
    execute_roots(request, io, shredder, &progress, &CancellationToken::new())
}

#[test]
fn rejects_unsafe_roots_before_opening_or_mutation() {
    let io = FakeIo::new();
    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };
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
    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };

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
    let calls = Arc::new(Mutex::new(Vec::new()));
    let shredder = FakeShredder {
        calls: Arc::clone(&calls),
        fail: false,
    };

    let result = run(
        ExecuteRootsRequest {
            roots: vec![root_request("links", &root, TargetKind::Directory)],
        },
        &io,
        &shredder,
    );

    assert_eq!(result.roots[0].status, RootStatus::Destroyed);
    assert_eq!(calls.lock().unwrap().len(), 1);
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
fn child_failure_keeps_directories_and_stops_later_roots() {
    let first = home_child("task7-failing");
    let second = home_child("task7-later");
    let io = FakeIo::new()
        .root(first.clone(), 1, directory(1, vec![(2, "file")]))
        .add_node(2, regular(2))
        .root(second.clone(), 3, directory(3, vec![(4, "file")]))
        .add_node(4, regular(4));
    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: true,
    };

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

    assert_eq!(result.roots[0].status, RootStatus::Failed);
    assert!(!result.roots[0].root_removed);
    assert_eq!(result.roots[1].status, RootStatus::Skipped);
    let events = io.events();
    let events = events.lock().unwrap();
    assert!(!events.calls.iter().any(|call| call == "remove_dir"));
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
fn rejects_parent_child_root_overlap_before_mutation() {
    let parent = home_child("task7-parent");
    let child = parent.join("child");
    let io = FakeIo::new()
        .root(parent.clone(), 1, directory(1, vec![]))
        .root(child.clone(), 2, regular(2));
    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };

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
    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };

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

    let shredder = FakeShredder {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };
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
fn legacy_open_file_shredder_rejects_directory_identity() {
    let adapter = LegacyOpenFileShredder::new(
        Arc::new(NistClear),
        1,
        crate::shredder::PatternType::Zeros,
        VerificationLevel::None,
        Arc::new(NoopProgressReporter),
    );
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
