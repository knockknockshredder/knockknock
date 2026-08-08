use super::{OpenFileShredder, SecureTreeIo};
use crate::shredder::cancel::CancellationToken;
use crate::shredder::engine::OverwriteState;
use crate::shredder::errors::{JournalError, ShredError};
use crate::shredder::journal::{JournalEntry, JournalStore};
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::{
    validate_storage_for_method, BatchRootResult, ChildErrorDto, DeletionPolicy,
    ExecuteRootRequest, ExecuteRootsRequest, ExecutionStage, MediaType, RootResultDto, RootStatus,
    TargetKind, WriteCheckOutcome,
};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

const MAX_DEPTH: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DirHandle {
    id: u64,
}

impl DirHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn as_node(&self) -> NodeHandle {
        NodeHandle::new(self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NodeHandle {
    id: u64,
}

impl NodeHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn as_dir(&self) -> DirHandle {
        DirHandle::new(self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildName(OsString);

impl ChildName {
    pub(crate) fn new(name: OsString) -> Self {
        Self(name)
    }

    pub(crate) fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NodeKind {
    RegularFile,
    Directory,
    Link,
    Special,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NodeIdentity {
    id: u128,
    mount_id: u64,
    kind: NodeKind,
}

impl PartialEq for NodeIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.mount_id == other.mount_id && self.kind == other.kind
    }
}

impl Eq for NodeIdentity {}

impl std::hash::Hash for NodeIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.mount_id.hash(state);
        self.kind.hash(state);
    }
}

impl NodeIdentity {
    pub(crate) fn new(id: u128, mount_id: u64, kind: NodeKind) -> Self {
        Self { id, mount_id, kind }
    }

    pub(crate) fn regular(id: u128, mount_id: u64) -> Self {
        Self::new(id, mount_id, NodeKind::RegularFile)
    }

    pub(crate) fn directory(id: u128, mount_id: u64) -> Self {
        Self::new(id, mount_id, NodeKind::Directory)
    }

    pub(crate) fn link(id: u128, mount_id: u64) -> Self {
        Self::new(id, mount_id, NodeKind::Link)
    }

    pub(crate) fn special(id: u128, mount_id: u64) -> Self {
        Self::new(id, mount_id, NodeKind::Special)
    }

    pub(crate) fn id(&self) -> u128 {
        self.id
    }

    pub(crate) fn mount_id(&self) -> u64 {
        self.mount_id
    }

    pub(crate) fn kind(&self) -> NodeKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RenamedNode;

impl RenamedNode {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileShredRequest {
    diagnostic_path: PathBuf,
}

impl FileShredRequest {
    pub(crate) fn new(diagnostic_path: PathBuf) -> Self {
        Self { diagnostic_path }
    }

    pub(crate) fn diagnostic_path(&self) -> &Path {
        &self.diagnostic_path
    }
}

/// Structured outcome of one file's destructive lifecycle (v2, M2/M3).
///
/// `overwrite_state` is the authoritative decision field: `NotStarted` means
/// the target is intact (no journal/rename), `Partial` and `Completed` both
/// proceed to best-effort cleanup. `write_check_status` aggregates the final
/// read-back result. There is deliberately no `success: bool` — callers
/// branch on state (M3).
#[derive(Debug)]
pub(crate) struct FileShredResult {
    pub(crate) overwrite_state: OverwriteState,
    pub(crate) write_check_status: WriteCheckOutcome,
    pub(crate) bytes_shredded: u64,
    pub(crate) issues: Vec<ShredError>,
}

#[derive(Debug)]
struct PlannedNode {
    parent: DirHandle,
    handle: NodeHandle,
    identity: NodeIdentity,
    kind: NodeKind,
    name: OsString,
    trusted_parent_path: PathBuf,
    diagnostic_path: PathBuf,
    children: Vec<PlannedNode>,
}

#[derive(Debug)]
struct RootPlan {
    request: ExecuteRootRequest,
    handle: DirHandle,
    trusted_parent: Option<DirHandle>,
    node: PlannedNode,
}

#[derive(Debug)]
struct RootFailure {
    request: ExecuteRootRequest,
    error: ChildErrorDto,
}

#[derive(Debug)]
enum PreflightOutcome {
    Ready(RootPlan),
    Failed(RootFailure),
}

pub(crate) fn execute_roots(
    request: ExecuteRootsRequest,
    policy: DeletionPolicy,
    io: &dyn SecureTreeIo,
    file_shredder: &dyn OpenFileShredder,
    journal: &JournalStore,
    progress: &dyn ProgressReporter,
    cancel: &CancellationToken,
    classify_media: &dyn Fn(&Path) -> Result<MediaType, ShredError>,
) -> BatchRootResult {
    let mut identities = HashSet::new();
    let mut requested_paths = Vec::with_capacity(request.roots.len());
    let mut outcomes = Vec::with_capacity(request.roots.len());
    // M7: one classification per distinct volume, cached for the batch by
    // mount identity. Each root is single-volume (inspect_directory rejects
    // intra-root mount crossings); bind mounts share the device id, so one
    // representative path (the first root path) per mount_id covers them.
    let mut media_cache: HashMap<u64, (PathBuf, MediaType)> = HashMap::new();
    for root in request.roots.iter().cloned() {
        let path = PathBuf::from(&root.path);
        outcomes.push(preflight_root(
            root,
            io,
            &mut identities,
            &requested_paths,
            policy,
            &mut media_cache,
            classify_media,
        ));
        requested_paths.push(path);
    }

    let has_preflight_failure = outcomes
        .iter()
        .any(|outcome| matches!(outcome, PreflightOutcome::Failed(_)));
    if has_preflight_failure {
        return BatchRootResult {
            roots: outcomes
                .into_iter()
                .map(|outcome| match outcome {
                    PreflightOutcome::Failed(failure) => {
                        failed_result(failure.request, vec![failure.error])
                    }
                    PreflightOutcome::Ready(plan) => skipped_result(plan.request),
                })
                .collect(),
        };
    }

    let mut results = Vec::with_capacity(outcomes.len());
    let mut execution_stopped = false;
    for outcome in outcomes {
        let PreflightOutcome::Ready(plan) = outcome else {
            unreachable!("preflight failure handled above")
        };

        if execution_stopped {
            results.push(skipped_result(plan.request));
            continue;
        }
        if cancel.is_cancelled() {
            execution_stopped = true;
            results.push(cancelled_result(plan.request));
            continue;
        }

        let request = plan.request.clone();
        let mut result =
            RootExecution::new(plan.request, plan.handle, plan.trusted_parent, plan.node);
        match result.execute(io, file_shredder, journal, progress, cancel) {
            Ok(()) => {}
            Err(ExecuteError::Cancelled) => {
                // Stop-after-current-file (D7): the current file completed its
                // destructive lifecycle; counters are preserved, no error is
                // pushed, and no "irreversible partial destruction"
                // augmentation applies (Failed-only). Real per-file issues
                // take precedence over cancellation.
                result.status = if result.errors.is_empty() {
                    RootStatus::Cancelled
                } else {
                    RootStatus::Failed
                };
                execution_stopped = true;
            }
            Err(ExecuteError::Failed(mut error)) => {
                if result.partial_destruction || result.bytes_shredded > 0 {
                    error.message = format!(
                        "{}; previous overwrites are irreversible partial destruction",
                        error.message
                    );
                    error.actionable = "Previous overwrites are irreversible; preserve the containing directory and investigate before retrying".to_string();
                }
                result.errors.push(error);
                result.root_removed = false;
                result.status = RootStatus::Failed;
                execution_stopped = true;
            }
        }
        results.push(result.finish(request));
    }

    BatchRootResult { roots: results }
}

fn preflight_root(
    request: ExecuteRootRequest,
    io: &dyn SecureTreeIo,
    identities: &mut HashSet<NodeIdentity>,
    requested_paths: &[PathBuf],
    policy: DeletionPolicy,
    media_cache: &mut HashMap<u64, (PathBuf, MediaType)>,
    classify_media: &dyn Fn(&Path) -> Result<MediaType, ShredError>,
) -> PreflightOutcome {
    let path = PathBuf::from(&request.path);
    if let Err(error) = validate_root_path(&path) {
        return PreflightOutcome::Failed(RootFailure {
            error: child_error(&path, ExecutionStage::Preflight, error),
            request,
        });
    }
    if requested_paths
        .iter()
        .any(|existing| roots_overlap(&path, existing))
    {
        return PreflightOutcome::Failed(RootFailure {
            error: child_error(
                &path,
                ExecutionStage::Preflight,
                ShredError::ValidationFailed("parent/child root overlap".to_string()),
            ),
            request,
        });
    }

    let handle = match io.open_root_nofollow(&path) {
        Ok(handle) => handle,
        Err(error) => {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(&path, ExecutionStage::Preflight, error),
                request,
            })
        }
    };
    let node_handle = handle.as_node();
    let identity = match io.identity(&node_handle) {
        Ok(identity) => identity,
        Err(error) => {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(&path, ExecutionStage::Preflight, error),
                request,
            })
        }
    };

    if let Err(error) = validate_root_kind(request.kind, identity.kind()) {
        return PreflightOutcome::Failed(RootFailure {
            error: child_error(&path, ExecutionStage::Preflight, error),
            request,
        });
    }
    if !identities.insert(identity) {
        return PreflightOutcome::Failed(RootFailure {
            error: child_error(
                &path,
                ExecutionStage::Preflight,
                ShredError::ValidationFailed("duplicate node identity".to_string()),
            ),
            request,
        });
    }

    // M7: the Legacy 3-pass method is only permitted on confirmed magnetic
    // HDD storage. One representative path per distinct volume (the first
    // root path on that mount) is classified once per batch; any volume not
    // confirmed Hdd — or any classifier error — fails this root's preflight
    // (fail closed), which aborts the whole batch before mutation via the
    // existing any-preflight-failure semantics. Automatic never classifies.
    if policy.requires_hdd() {
        let mount_id = identity.mount_id();
        let media = match media_cache.get(&mount_id) {
            Some((_, media)) => *media,
            None => {
                let media = match classify_media(&path) {
                    Ok(media) => media,
                    Err(error) => {
                        return PreflightOutcome::Failed(RootFailure {
                            error: child_error(&path, ExecutionStage::Preflight, error),
                            request,
                        })
                    }
                };
                media_cache.insert(mount_id, (path.clone(), media));
                media
            }
        };
        // The pure rule reports an empty path; re-raise it with the root
        // path so the structured error points at the real target (M11).
        if let Err(ShredError::UnsupportedStorageForMethod { method, media, .. }) =
            validate_storage_for_method(policy.method, media)
        {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(
                    &path,
                    ExecutionStage::Preflight,
                    ShredError::UnsupportedStorageForMethod {
                        path: path.clone(),
                        method,
                        media,
                    },
                ),
                request,
            });
        }
    }

    // M6 preflight: a regular-file root with more than one hard link is
    // blocked before any mutation — shredding one name would silently leave
    // the data reachable through the sibling. Query errors fail closed.
    let kind = identity.kind();
    if kind == NodeKind::RegularFile {
        let count = match io.link_count(&node_handle) {
            Ok(count) => count,
            Err(error) => {
                return PreflightOutcome::Failed(RootFailure {
                    error: child_error(&path, ExecutionStage::Preflight, error),
                    request,
                })
            }
        };
        if count > 1 {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(
                    &path,
                    ExecutionStage::Preflight,
                    ShredError::HardLinkBlocked {
                        path: path.clone(),
                        count,
                    },
                ),
                request,
            });
        }
    }

    let kind = identity.kind();
    let root_name = match path.file_name().map(OsStr::to_os_string) {
        Some(name) => name,
        None => {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(
                    &path,
                    ExecutionStage::Preflight,
                    ShredError::ValidationFailed(
                        "execution target has no original basename".to_string(),
                    ),
                ),
                request,
            })
        }
    };
    let trusted_parent = if kind == NodeKind::RegularFile {
        let parent_path = match path.parent() {
            Some(parent) => parent,
            None => {
                return PreflightOutcome::Failed(RootFailure {
                    error: child_error(
                        &path,
                        ExecutionStage::Preflight,
                        ShredError::ValidationFailed(
                            "file root has no containing directory".to_string(),
                        ),
                    ),
                    request,
                })
            }
        };
        let parent = match io.open_root_nofollow(parent_path) {
            Ok(parent) => parent,
            Err(error) => {
                return PreflightOutcome::Failed(RootFailure {
                    error: child_error(&path, ExecutionStage::Preflight, error),
                    request,
                })
            }
        };
        let parent_identity = match io.identity(&parent.as_node()) {
            Ok(identity) => identity,
            Err(error) => {
                return PreflightOutcome::Failed(RootFailure {
                    error: child_error(&path, ExecutionStage::Preflight, error),
                    request,
                })
            }
        };
        if parent_identity.kind() != NodeKind::Directory {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(
                    &path,
                    ExecutionStage::Preflight,
                    ShredError::ValidationFailed("file root parent is not a directory".to_string()),
                ),
                request,
            });
        }
        if parent_identity.mount_id() != identity.mount_id() {
            return PreflightOutcome::Failed(RootFailure {
                error: child_error(
                    &path,
                    ExecutionStage::Preflight,
                    ShredError::ValidationFailed(
                        "file root parent mount does not match the root".to_string(),
                    ),
                ),
                request,
            });
        }
        Some(parent)
    } else {
        None
    };

    let children = if kind == NodeKind::Directory {
        match inspect_directory(&handle, &path, identity.mount_id(), 0, io, identities) {
            Ok(children) => children,
            Err(error) => {
                return PreflightOutcome::Failed(RootFailure {
                    error: child_error(&path, ExecutionStage::Preflight, error),
                    request,
                })
            }
        }
    } else {
        Vec::new()
    };

    PreflightOutcome::Ready(RootPlan {
        request,
        handle,
        trusted_parent,
        node: PlannedNode {
            parent: trusted_parent.unwrap_or(handle),
            handle: node_handle,
            identity,
            kind,
            name: root_name,
            trusted_parent_path: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            diagnostic_path: path,
            children,
        },
    })
}

fn inspect_directory(
    parent: &DirHandle,
    diagnostic_path: &Path,
    root_mount_id: u64,
    depth: usize,
    io: &dyn SecureTreeIo,
    identities: &mut HashSet<NodeIdentity>,
) -> Result<Vec<PlannedNode>, ShredError> {
    if depth >= MAX_DEPTH {
        return Err(ShredError::ValidationFailed(format!(
            "directory depth limit ({MAX_DEPTH}) exceeded"
        )));
    }

    let children = io.enumerate(parent)?;
    let mut plans = Vec::with_capacity(children.len());
    for child_name in children {
        validate_child_name(child_name.as_os_str())?;
        let node = io.open_child_nofollow(parent, child_name.as_os_str())?;
        let identity = io.identity(&node)?;

        if identity.mount_id() != root_mount_id {
            return Err(ShredError::ValidationFailed(
                "mount crossing detected".to_string(),
            ));
        }
        if !identities.insert(identity) {
            return Err(ShredError::ValidationFailed(
                "duplicate node identity".to_string(),
            ));
        }

        let child_path = diagnostic_path.join(child_name.as_os_str());
        let kind = identity.kind();
        // M6 preflight: a hard-linked regular file inside a directory root
        // blocks the WHOLE batch before any mutation. Query errors fail
        // closed. Hard links elsewhere in the tree are not an issue — only
        // the data-bearing files are enumerated targets.
        if kind == NodeKind::RegularFile {
            let count = io.link_count(&node)?;
            if count > 1 {
                return Err(ShredError::HardLinkBlocked {
                    path: child_path.clone(),
                    count,
                });
            }
        }
        let nested = if kind == NodeKind::Directory {
            let child_dir = node.as_dir();
            inspect_directory(
                &child_dir,
                &child_path,
                root_mount_id,
                depth + 1,
                io,
                identities,
            )?
        } else {
            if kind == NodeKind::Special {
                return Err(ShredError::ValidationFailed(
                    "special files are not safe execution targets".to_string(),
                ));
            }
            Vec::new()
        };

        plans.push(PlannedNode {
            parent: *parent,
            handle: node,
            identity,
            kind,
            name: child_name.0,
            trusted_parent_path: diagnostic_path.to_path_buf(),
            diagnostic_path: child_path,
            children: nested,
        });
    }
    Ok(plans)
}

fn validate_root_path(path: &Path) -> Result<(), ShredError> {
    if !path.is_absolute() {
        return Err(ShredError::ValidationFailed(
            "relative paths are not safe execution roots".to_string(),
        ));
    }
    if path.parent().is_none() {
        return Err(ShredError::ValidationFailed(
            "filesystem roots are not safe execution roots".to_string(),
        ));
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(ShredError::ValidationFailed(
            "parent traversal is not safe for execution roots".to_string(),
        ));
    }
    if crate::shredder::validation::is_network_drive(path) {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }

    let home = std::env::home_dir()
        .ok_or_else(|| ShredError::ValidationFailed("home directory is unavailable".to_string()))?;
    if path == home {
        return Err(ShredError::ValidationFailed(
            "the home directory is not a safe execution root".to_string(),
        ));
    }
    if !path.starts_with(&home) {
        return Err(ShredError::ValidationFailed(
            "execution roots must be inside the home directory".to_string(),
        ));
    }
    Ok(())
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        let left = left.to_string_lossy().to_ascii_lowercase();
        let right = right.to_string_lossy().to_ascii_lowercase();
        let left = left.trim_end_matches(|character| character == '\\' || character == '/');
        let right = right.trim_end_matches(|character| character == '\\' || character == '/');
        left == right
            || left
                .strip_prefix(right)
                .is_some_and(|suffix| suffix.starts_with('\\') || suffix.starts_with('/'))
            || right
                .strip_prefix(left)
                .is_some_and(|suffix| suffix.starts_with('\\') || suffix.starts_with('/'))
    }
    #[cfg(not(windows))]
    {
        left == right || left.starts_with(right) || right.starts_with(left)
    }
}

fn validate_root_kind(requested: TargetKind, actual: NodeKind) -> Result<(), ShredError> {
    if matches!(actual, NodeKind::Link | NodeKind::Special) {
        return Err(ShredError::ValidationFailed(
            "links and special files are not safe execution roots".to_string(),
        ));
    }

    let valid = match requested {
        TargetKind::File => actual == NodeKind::RegularFile,
        TargetKind::Directory => actual == NodeKind::Directory,
        TargetKind::Link => false,
        TargetKind::UnknownLegacy => true,
    };
    if valid {
        Ok(())
    } else {
        Err(ShredError::ValidationFailed(
            "execution root kind does not match the opened node".to_string(),
        ))
    }
}

fn validate_child_name(name: &OsStr) -> Result<(), ShredError> {
    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(ShredError::ValidationFailed(
            "adapter returned an unsafe child name".to_string(),
        )),
    }
}

/// Failure mode of one root's execution walk (D7).
enum ExecuteError {
    /// Stop-after-current-file cancellation observed at a node boundary.
    /// The current file's destructive lifecycle already completed; remaining
    /// nodes and roots are not started. No error is reported.
    Cancelled,
    /// Hard node-level failure (journal/rename/unlink/sync) that aborts this
    /// file's cleanup and stops the walk (M2 rule 4).
    Failed(ChildErrorDto),
}

struct RootExecution {
    handle: DirHandle,
    node: PlannedNode,
    status: RootStatus,
    root_removed: bool,
    files_destroyed: u64,
    directories_removed: u64,
    bytes_shredded: u64,
    partial_destruction: bool,
    write_check_failed_any: bool,
    write_check_passed_any: bool,
    errors: Vec<ChildErrorDto>,
}

impl RootExecution {
    fn new(
        _request: ExecuteRootRequest,
        handle: DirHandle,
        trusted_parent: Option<DirHandle>,
        mut node: PlannedNode,
    ) -> Self {
        if let Some(parent) = trusted_parent {
            node.parent = parent;
        }
        Self {
            handle,
            node,
            status: RootStatus::Destroyed,
            root_removed: false,
            files_destroyed: 0,
            directories_removed: 0,
            bytes_shredded: 0,
            partial_destruction: false,
            write_check_failed_any: false,
            write_check_passed_any: false,
            errors: Vec::new(),
        }
    }

    fn execute(
        &mut self,
        io: &dyn SecureTreeIo,
        file_shredder: &dyn OpenFileShredder,
        journal: &JournalStore,
        progress: &dyn ProgressReporter,
        cancel: &CancellationToken,
    ) -> Result<(), ExecuteError> {
        let root = std::mem::replace(
            &mut self.node,
            PlannedNode {
                parent: self.handle,
                handle: self.handle.as_node(),
                identity: NodeIdentity::new(0, 0, NodeKind::Special),
                kind: NodeKind::Special,
                name: OsString::new(),
                trusted_parent_path: PathBuf::new(),
                diagnostic_path: PathBuf::new(),
                children: Vec::new(),
            },
        );
        // A file root is removed only when its own file was destroyed; a
        // directory root is removed when the walk completed its cleanup.
        let root_removed =
            self.execute_node(&root, io, file_shredder, journal, progress, cancel)?;
        self.root_removed = root_removed;
        Ok(())
    }

    fn execute_node(
        &mut self,
        node: &PlannedNode,
        io: &dyn SecureTreeIo,
        file_shredder: &dyn OpenFileShredder,
        journal: &JournalStore,
        progress: &dyn ProgressReporter,
        cancel: &CancellationToken,
    ) -> Result<bool, ExecuteError> {
        // Stop-after-current-file (D7): cancellation is only observed at node
        // boundaries. The previous file completed its destructive lifecycle;
        // this node and everything after it is not started.
        if cancel.is_cancelled() {
            return Err(ExecuteError::Cancelled);
        }

        match node.kind {
            NodeKind::RegularFile => self.execute_file(node, io, file_shredder, journal, progress),
            NodeKind::Link => self.execute_link(node, io),
            NodeKind::Directory => {
                for child in &node.children {
                    if let Err(error) =
                        self.execute_node(child, io, file_shredder, journal, progress, cancel)
                    {
                        return Err(error);
                    }
                }
                io.remove_empty_dir(&node.parent, &node.handle)
                    .map_err(|error| {
                        ExecuteError::Failed(child_error(
                            &node.diagnostic_path,
                            ExecutionStage::DirectoryRemove,
                            error,
                        ))
                    })?;
                io.sync_parent(&node.parent).map_err(|error| {
                    ExecuteError::Failed(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Sync,
                        error,
                    ))
                })?;
                self.directories_removed += 1;
                Ok(true)
            }
            NodeKind::Special => Err(ExecuteError::Failed(child_error(
                &node.diagnostic_path,
                ExecutionStage::Preflight,
                ShredError::ValidationFailed(
                    "special files are not safe execution targets".to_string(),
                ),
            ))),
        }
    }

    /// Shred one planned regular file and remove it. Returns `Ok(true)` when
    /// the file was removed, `Ok(false)` when a per-file issue left the
    /// target intact (the walk continues — per-file issues never stop the
    /// batch, ora-2 amendment 2), and `Err` only for hard node-level failures
    /// (journal/rename/unlink/sync) that abort this file's cleanup and stop
    /// the walk (M2 rule 4).
    fn execute_file(
        &mut self,
        node: &PlannedNode,
        io: &dyn SecureTreeIo,
        file_shredder: &dyn OpenFileShredder,
        journal: &JournalStore,
        _progress: &dyn ProgressReporter,
    ) -> Result<bool, ExecuteError> {
        let file = match io.open_regular_for_shred(&node.handle) {
            Ok(file) => file,
            Err(error) => {
                // Open failure is a PER-FILE issue, not a batch abort
                // (ORACLE-2 SHOULD-FIX 1): the target could not be opened
                // before any byte was written — it is intact and must not be
                // journaled, renamed, or unlinked. Report the Overwrite-stage
                // issue and continue the walk (same mechanism as the
                // shredder-Err NotStarted arm below).
                self.errors.push(child_error(
                    &node.diagnostic_path,
                    ExecutionStage::Overwrite,
                    error,
                ));
                return Ok(false);
            }
        };
        let request = FileShredRequest::new(node.diagnostic_path.clone());
        let shred_result = match file_shredder.shred_open_file(file, node.identity, &request) {
            Ok(result) => result,
            Err(error) => {
                // Hard shredder error (identity/kind rejection, execution-time
                // hard-link recheck): the handle was refused before any byte
                // was written — the target is intact. Report the issue and
                // continue the walk.
                self.errors.push(child_error(
                    &node.diagnostic_path,
                    ExecutionStage::Overwrite,
                    error,
                ));
                return Ok(false);
            }
        };
        self.bytes_shredded += shred_result.bytes_shredded;

        match (
            shred_result.overwrite_state,
            shred_result.write_check_status,
        ) {
            // Failure before any byte was modified (M2 rule 1): the target is
            // intact — it must not be journaled, renamed, or unlinked. The
            // issue is reported and the walk continues.
            (OverwriteState::NotStarted, _) => {
                for error in shred_result.issues {
                    self.errors.push(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Overwrite,
                        error,
                    ));
                }
                return Ok(false);
            }
            // At least one byte was written but the overwrite did not
            // complete: best-effort cleanup below; the root is never a clean
            // Destroyed (M2 rule 2).
            (OverwriteState::Partial, _) => {
                self.partial_destruction = true;
                for error in shred_result.issues {
                    self.errors.push(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Overwrite,
                        error,
                    ));
                }
            }
            // Overwrite completed; the final read-back failed. Removal
            // continues, but write_check reports Failed and the root is not
            // a clean success (M2 rule 3).
            (OverwriteState::Completed, WriteCheckOutcome::Failed) => {
                self.write_check_failed_any = true;
                for error in shred_result.issues {
                    self.errors.push(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Verify,
                        error,
                    ));
                }
            }
            (OverwriteState::Completed, WriteCheckOutcome::Passed) => {
                self.write_check_passed_any = true;
                // The engine produces no issues on this path; surface any a
                // shredder reports instead of silently dropping them.
                for error in shred_result.issues {
                    self.errors.push(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Overwrite,
                        error,
                    ));
                }
            }
            // Zero-length vacuous completion or Off write check: nothing to
            // report beyond the state.
            (OverwriteState::Completed, WriteCheckOutcome::NotRun) => {
                for error in shred_result.issues {
                    self.errors.push(child_error(
                        &node.diagnostic_path,
                        ExecutionStage::Overwrite,
                        error,
                    ));
                }
            }
        }

        // Best-effort cleanup — journal → rename → unlink → sync parent →
        // clear (M4 lifecycle; no truncate). Runs for Partial and Completed
        // outcomes regardless of write-check status. Journal/rename/unlink/
        // sync failures at the node level remain hard errors that abort this
        // file's cleanup and stop the walk (M2 rule 4).
        let new_name = OsString::from(format!(".knockknock-{:032x}", node.identity.id()));
        let parent_path = &node.trusted_parent_path;
        let parent = node.parent;
        let parent_identity = io.identity(&parent.as_node()).map_err(|error| {
            ExecuteError::Failed(child_error(
                &node.diagnostic_path,
                ExecutionStage::Journal,
                error,
            ))
        })?;
        let entry = JournalEntry::for_root_rename(
            parent_path,
            parent_identity,
            &new_name,
            node.identity,
            node.kind,
        )
        .map_err(|error| ExecuteError::Failed(journal_child_error(&node.diagnostic_path, error)))?;
        journal.append(entry.clone()).map_err(|error| {
            ExecuteError::Failed(journal_child_error(&node.diagnostic_path, error))
        })?;

        io.rename_noreplace(&parent, &node.handle, &new_name)
            .map_err(|error| {
                ExecuteError::Failed(child_error(
                    &node.diagnostic_path,
                    ExecutionStage::Rename,
                    error,
                ))
            })?;

        if let Err(error) = io.sync_parent(&parent) {
            return Err(ExecuteError::Failed(self.rollback_after_failure(
                node,
                &parent,
                io,
                error,
                ExecutionStage::Sync,
                "rename durability sync failed",
            )));
        }

        if let Err(error) = io.unlink_leaf(&parent, &node.handle) {
            return Err(ExecuteError::Failed(self.rollback_after_failure(
                node,
                &parent,
                io,
                error,
                ExecutionStage::Delete,
                "deletion failed",
            )));
        }
        self.files_destroyed += 1;

        io.sync_parent(&parent).map_err(|error| {
            ExecuteError::Failed(child_error(
                &node.diagnostic_path,
                ExecutionStage::Sync,
                error,
            ))
        })?;

        journal.clear(&entry).map_err(|error| {
            ExecuteError::Failed(journal_child_error(&node.diagnostic_path, error))
        })?;
        Ok(true)
    }

    fn rollback_after_failure(
        &self,
        node: &PlannedNode,
        parent: &DirHandle,
        io: &dyn SecureTreeIo,
        error: ShredError,
        stage: ExecutionStage,
        description: &str,
    ) -> ChildErrorDto {
        let rollback = io
            .rename_noreplace(parent, &node.handle, &node.name)
            .and_then(|_| io.sync_parent(parent));
        let error = match rollback {
            Ok(()) => error,
            Err(rollback_error) => ShredError::ValidationFailed(format!(
                "{description}: {error}; rollback to original name failed: {rollback_error}"
            )),
        };
        child_error(&node.diagnostic_path, stage, error)
    }

    fn execute_link(
        &mut self,
        node: &PlannedNode,
        io: &dyn SecureTreeIo,
    ) -> Result<bool, ExecuteError> {
        io.unlink_leaf(&node.parent, &node.handle)
            .map_err(|error| {
                ExecuteError::Failed(child_error(
                    &node.diagnostic_path,
                    ExecutionStage::Delete,
                    error,
                ))
            })?;
        io.sync_parent(&node.parent).map_err(|error| {
            ExecuteError::Failed(child_error(
                &node.diagnostic_path,
                ExecutionStage::Sync,
                error,
            ))
        })?;
        self.files_destroyed += 1;
        Ok(true)
    }

    fn finish(mut self, request: ExecuteRootRequest) -> RootResultDto {
        // A root with any per-file issue is not a clean Destroyed: downgrade
        // so the frontend never renders plain success for a degraded
        // operation (ora-2 amendment 2). `root_removed` is preserved — the
        // entry may still have been removed.
        if self.status == RootStatus::Destroyed && !self.errors.is_empty() {
            self.status = RootStatus::Failed;
        }
        let write_check = if self.write_check_failed_any {
            WriteCheckOutcome::Failed
        } else if self.write_check_passed_any {
            WriteCheckOutcome::Passed
        } else {
            WriteCheckOutcome::NotRun
        };
        RootResultDto {
            target_id: request.target_id,
            requested_path: request.path,
            kind: request.kind,
            status: self.status,
            root_removed: self.root_removed,
            files_destroyed: self.files_destroyed,
            directories_removed: self.directories_removed,
            bytes_shredded: self.bytes_shredded,
            write_check,
            errors: self.errors,
        }
    }
}

fn failed_result(request: ExecuteRootRequest, errors: Vec<ChildErrorDto>) -> RootResultDto {
    RootResultDto {
        target_id: request.target_id,
        requested_path: request.path,
        kind: request.kind,
        status: RootStatus::Failed,
        root_removed: false,
        files_destroyed: 0,
        directories_removed: 0,
        bytes_shredded: 0,
        write_check: WriteCheckOutcome::NotRun,
        errors,
    }
}

fn skipped_result(request: ExecuteRootRequest) -> RootResultDto {
    RootResultDto {
        target_id: request.target_id,
        requested_path: request.path,
        kind: request.kind,
        status: RootStatus::Skipped,
        root_removed: false,
        files_destroyed: 0,
        directories_removed: 0,
        bytes_shredded: 0,
        write_check: WriteCheckOutcome::NotRun,
        errors: Vec::new(),
    }
}

fn cancelled_result(request: ExecuteRootRequest) -> RootResultDto {
    RootResultDto {
        target_id: request.target_id,
        requested_path: request.path,
        kind: request.kind,
        status: RootStatus::Cancelled,
        root_removed: false,
        files_destroyed: 0,
        directories_removed: 0,
        bytes_shredded: 0,
        write_check: WriteCheckOutcome::NotRun,
        errors: Vec::new(),
    }
}

fn child_error(path: &Path, stage: ExecutionStage, error: ShredError) -> ChildErrorDto {
    ChildErrorDto {
        path: path.to_string_lossy().into_owned(),
        stage,
        error_type: error_type(&error).to_string(),
        message: error.to_string(),
        actionable: actionable(stage).to_string(),
    }
}

fn journal_child_error(path: &Path, error: JournalError) -> ChildErrorDto {
    ChildErrorDto {
        path: path.to_string_lossy().into_owned(),
        stage: ExecutionStage::Journal,
        error_type: "journal_error".to_string(),
        message: error.to_string(),
        actionable: "Journal durability or recovery failed; preserve the containing directory and investigate before retrying".to_string(),
    }
}

fn error_type(error: &ShredError) -> &'static str {
    match error {
        ShredError::FileNotFound(_) => "file_not_found",
        ShredError::PermissionDenied(_) => "permission_denied",
        ShredError::IoError { .. } => "io_error",
        ShredError::HardLinkBlocked { .. } => "hard_link_blocked",
        ShredError::UnsupportedStorageForMethod { .. } => "unsupported_storage_for_method",
        ShredError::WriteCheckFailed { .. } => "write_check_failed",
        ShredError::BrowserCollectionFailed { .. } => "browser_collection_failed",
        ShredError::NetworkDrive(_) => "network_drive",
        ShredError::SystemFile(_) => "system_file",
        ShredError::ShortcutDetected { .. } => "shortcut_detected",
        ShredError::InvalidPathType(_) => "invalid_path_type",
        ShredError::EmptyPath => "empty_path",
        ShredError::ValidationFailed(_) => "validation_failed",
    }
}

fn actionable(stage: ExecutionStage) -> &'static str {
    match stage {
        ExecutionStage::Preflight => "Fix the target validation errors and retry",
        ExecutionStage::Overwrite | ExecutionStage::Verify => {
            "The target may be partially destroyed; inspect it before retrying"
        }
        ExecutionStage::Rename | ExecutionStage::Delete => {
            "The target may be partially destroyed; do not assume cleanup completed"
        }
        ExecutionStage::DirectoryRemove | ExecutionStage::Journal | ExecutionStage::Sync => {
            "Containing directories were preserved; resolve the error before retrying"
        }
    }
}
