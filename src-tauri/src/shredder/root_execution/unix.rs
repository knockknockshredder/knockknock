use super::plan::{ChildName, DirHandle, NodeHandle, NodeIdentity, NodeKind, RenamedNode};
use super::SecureTreeIo;
use crate::shredder::errors::ShredError;
use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags};
use rustix::io::{dup, Errno};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::os::fd::{FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VolumeIdentity {
    device: u64,
}

impl VolumeIdentity {
    pub(crate) const fn new(device: u64) -> Self {
        Self { device }
    }
}

pub(crate) const fn same_volume(left: VolumeIdentity, right: VolumeIdentity) -> bool {
    left.device == right.device
}

struct HandleEntry {
    fd: Option<OwnedFd>,
    identity: NodeIdentity,
    name: OsString,
    parent_id: Option<u64>,
    parent_identity: Option<NodeIdentity>,
    volume: VolumeIdentity,
    removed: bool,
}

#[derive(Default)]
struct HandleTable {
    next_id: u64,
    entries: HashMap<u64, HandleEntry>,
}

pub(crate) struct UnixSecureTreeIo {
    table: Mutex<HandleTable>,
}

impl UnixSecureTreeIo {
    pub(crate) fn new() -> Self {
        Self {
            table: Mutex::new(HandleTable {
                next_id: 1,
                entries: HashMap::new(),
            }),
        }
    }

    fn lock_table(&self) -> Result<MutexGuard<'_, HandleTable>, ShredError> {
        self.table.lock().map_err(|_| {
            ShredError::ValidationFailed("secure Unix handle table is poisoned".to_string())
        })
    }

    fn allocate_id(table: &mut HandleTable) -> Result<u64, ShredError> {
        let id = table.next_id;
        table.next_id = table.next_id.checked_add(1).ok_or_else(|| {
            ShredError::ValidationFailed("secure handle table is exhausted".to_string())
        })?;
        Ok(id)
    }

    fn insert_entry(table: &mut HandleTable, entry: HandleEntry) -> Result<u64, ShredError> {
        let id = Self::allocate_id(table)?;
        table.entries.insert(id, entry);
        Ok(id)
    }

    fn open_root(&self, path: &Path) -> Result<DirHandle, ShredError> {
        validate_absolute_path(path)?;
        ensure_local_volume(path)?;

        let components = normal_components(path)?;
        let mut current = open_root_directory()?;
        let mut parent_fd = None;
        let mut parent_identity = None;

        for (index, component) in components.iter().enumerate() {
            let is_final = index + 1 == components.len();
            if is_final {
                parent_identity = Some(identity_from_stat(
                    &rustix::fs::fstat(&current)
                        .map_err(|error| io_error("inspect root parent", error))?,
                ));
                parent_fd =
                    Some(dup(&current).map_err(|error| io_error("retain root parent", error))?);
            }

            let opened = open_component(&current, component, false).map_err(|error| {
                if error == Errno::LOOP {
                    ShredError::ValidationFailed(
                        "symlink root is not a safe execution target".to_string(),
                    )
                } else {
                    io_error("open root component", error)
                }
            })?;
            let fd = match opened {
                OpenedComponent::Handle(fd) => fd,
                OpenedComponent::Link(_) => {
                    return Err(ShredError::ValidationFailed(
                        "symlink root is not a safe execution target".to_string(),
                    ));
                }
            };
            let stat = rustix::fs::fstat(&fd)
                .map_err(|error| io_error("inspect root component", error))?;
            let identity = identity_from_stat(&stat);
            if !is_directory(identity.kind()) && !is_final {
                return Err(ShredError::ValidationFailed(
                    "root path component is not a directory".to_string(),
                ));
            }
            if !same_volume(
                volume_from_identity(identity),
                volume_from_stat(
                    &rustix::fs::fstat(&current)
                        .map_err(|error| io_error("inspect root volume", error))?,
                ),
            ) {
                return Err(ShredError::ValidationFailed(
                    "mount crossing detected while opening root".to_string(),
                ));
            }
            current = fd;
        }

        let stat = rustix::fs::fstat(&current).map_err(|error| io_error("inspect root", error))?;
        let identity = identity_from_stat(&stat);
        if identity.kind() == NodeKind::Link {
            return Err(ShredError::ValidationFailed(
                "symlink root is not a safe execution target".to_string(),
            ));
        }
        let name = components.last().cloned().ok_or_else(|| {
            ShredError::ValidationFailed(
                "filesystem roots are not safe execution targets".to_string(),
            )
        })?;
        let parent_fd = parent_fd.ok_or_else(|| {
            ShredError::ValidationFailed("root containing directory is unavailable".to_string())
        })?;
        let parent_identity = parent_identity.ok_or_else(|| {
            ShredError::ValidationFailed(
                "root containing directory identity is unavailable".to_string(),
            )
        })?;
        let parent_entry = HandleEntry {
            fd: Some(parent_fd),
            identity: parent_identity,
            name: OsString::from("."),
            parent_id: None,
            parent_identity: None,
            volume: volume_from_identity(parent_identity),
            removed: false,
        };
        let mut table = self.lock_table()?;
        let parent_id = Self::insert_entry(&mut table, parent_entry)?;
        let root_id = Self::insert_entry(
            &mut table,
            HandleEntry {
                fd: Some(current),
                identity,
                name,
                parent_id: Some(parent_id),
                parent_identity: Some(parent_identity),
                volume: volume_from_identity(identity),
                removed: false,
            },
        )?;
        Ok(DirHandle::new(root_id))
    }

    fn open_child(&self, parent: &DirHandle, name: &OsStr) -> Result<NodeHandle, ShredError> {
        validate_component(name)?;
        let mut table = self.lock_table()?;
        let parent_entry = table.entries.get(&parent.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure parent handle".to_string())
        })?;
        let parent_fd = parent_entry.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure parent handle has no descriptor".to_string())
        })?;
        if parent_entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "child open parent is not a directory".to_string(),
            ));
        }
        let parent_identity = identity_from_stat(
            &rustix::fs::fstat(parent_fd)
                .map_err(|error| io_error("inspect child parent", error))?,
        );
        if parent_identity != parent_entry.identity {
            return Err(ShredError::ValidationFailed(
                "child open parent identity changed".to_string(),
            ));
        }

        let opened = open_component(parent_fd, name, true).map_err(|error| {
            if error == Errno::XDEV {
                ShredError::ValidationFailed(
                    "mount crossing detected while opening child".to_string(),
                )
            } else {
                io_error("open child component", error)
            }
        })?;
        let (fd, stat) = match opened {
            OpenedComponent::Handle(fd) => {
                let stat =
                    rustix::fs::fstat(&fd).map_err(|error| io_error("inspect child", error))?;
                (Some(fd), stat)
            }
            OpenedComponent::Link(stat) => (None, stat),
        };
        let identity = identity_from_stat(&stat);
        if !same_volume(parent_entry.volume, volume_from_identity(identity)) {
            return Err(ShredError::ValidationFailed(
                "mount crossing detected while opening child".to_string(),
            ));
        }
        let id = Self::insert_entry(
            &mut table,
            HandleEntry {
                fd,
                identity,
                name: name.to_os_string(),
                parent_id: Some(parent.id()),
                parent_identity: Some(parent_identity),
                volume: volume_from_identity(identity),
                removed: false,
            },
        )?;
        Ok(NodeHandle::new(id))
    }

    fn resolve_mutation<'a>(
        table: &'a HandleTable,
        parent: &DirHandle,
        node: &NodeHandle,
    ) -> Result<MutationTarget<'a>, ShredError> {
        let node_entry = table.entries.get(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure node handle".to_string())
        })?;
        let requested_parent = table.entries.get(&parent.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure parent handle".to_string())
        })?;
        let actual_parent_id = if parent.id() == node.id() {
            node_entry.parent_id.ok_or_else(|| {
                ShredError::ValidationFailed(
                    "root has no retained containing directory".to_string(),
                )
            })?
        } else {
            parent.id()
        };
        let mutation_parent = if actual_parent_id == parent.id() {
            requested_parent
        } else {
            table.entries.get(&actual_parent_id).ok_or_else(|| {
                ShredError::ValidationFailed(
                    "retained containing directory handle is missing".to_string(),
                )
            })?
        };
        let parent_fd = mutation_parent.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure mutation parent has no descriptor".to_string())
        })?;
        if mutation_parent.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "secure mutation parent is not a directory".to_string(),
            ));
        }
        let parent_identity = identity_from_stat(
            &rustix::fs::fstat(parent_fd)
                .map_err(|error| io_error("inspect mutation parent", error))?,
        );
        if Some(parent_identity) != node_entry.parent_identity {
            return Err(ShredError::ValidationFailed(
                "mutation parent identity mismatch".to_string(),
            ));
        }
        let actual = rustix::fs::statat(parent_fd, &node_entry.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| io_error("inspect mutation target", error))?;
        let actual_identity = identity_from_stat(&actual);
        if actual_identity != node_entry.identity {
            return Err(ShredError::ValidationFailed(
                "mutation target identity mismatch".to_string(),
            ));
        }
        Ok(MutationTarget {
            parent_fd,
            node_entry,
            mutation_parent_id: actual_parent_id,
        })
    }
}

struct MutationTarget<'a> {
    parent_fd: &'a OwnedFd,
    node_entry: &'a HandleEntry,
    mutation_parent_id: u64,
}

impl SecureTreeIo for UnixSecureTreeIo {
    fn open_root_nofollow(&self, path: &Path) -> Result<DirHandle, ShredError> {
        self.open_root(path)
    }

    fn enumerate(&self, dir: &DirHandle) -> Result<Vec<ChildName>, ShredError> {
        let table = self.lock_table()?;
        let entry = table.entries.get(&dir.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure directory handle".to_string())
        })?;
        let fd = entry.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure directory handle has no descriptor".to_string())
        })?;
        if entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "cannot enumerate a non-directory handle".to_string(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            let mut storage: Vec<u8> = Vec::with_capacity(64 * 1024);
            let mut directory = rustix::fs::RawDir::new(fd, storage.spare_capacity_mut());
            let mut names = Vec::new();
            while let Some(result) = directory.next() {
                let entry = result.map_err(|error| io_error("enumerate directory", error))?;
                let bytes = entry.file_name().to_bytes();
                if bytes != b"." && bytes != b".." {
                    names.push(ChildName::new(OsString::from_vec(bytes.to_vec())));
                }
            }
            return Ok(names);
        }

        #[cfg(not(target_os = "linux"))]
        {
            let mut directory = rustix::fs::Dir::read_from(fd)
                .map_err(|error| io_error("enumerate directory", error))?;
            let mut names = Vec::new();
            while let Some(result) = directory.read() {
                let entry = result.map_err(|error| io_error("enumerate directory", error))?;
                let bytes = entry.file_name().to_bytes();
                if bytes != b"." && bytes != b".." {
                    names.push(ChildName::new(OsString::from_vec(bytes.to_vec())));
                }
            }
            Ok(names)
        }
    }

    fn open_child_nofollow(
        &self,
        parent: &DirHandle,
        name: &OsStr,
    ) -> Result<NodeHandle, ShredError> {
        self.open_child(parent, name)
    }

    fn identity(&self, node: &NodeHandle) -> Result<NodeIdentity, ShredError> {
        let table = self.lock_table()?;
        let entry = table.entries.get(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure node handle".to_string())
        })?;
        if let Some(fd) = entry.fd.as_ref() {
            let identity = identity_from_stat(
                &rustix::fs::fstat(fd).map_err(|error| io_error("inspect node handle", error))?,
            );
            if identity != entry.identity {
                return Err(ShredError::ValidationFailed(
                    "node handle identity changed".to_string(),
                ));
            }
            return Ok(identity);
        }
        let parent_id = entry.parent_id.ok_or_else(|| {
            ShredError::ValidationFailed("link handle has no containing directory".to_string())
        })?;
        let parent = table.entries.get(&parent_id).ok_or_else(|| {
            ShredError::ValidationFailed("link containing directory handle is missing".to_string())
        })?;
        let parent_fd = parent.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("link containing directory has no descriptor".to_string())
        })?;
        let identity = identity_from_stat(
            &rustix::fs::statat(parent_fd, &entry.name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| io_error("inspect link handle", error))?,
        );
        if identity != entry.identity {
            return Err(ShredError::ValidationFailed(
                "link handle identity changed".to_string(),
            ));
        }
        Ok(identity)
    }

    fn open_regular_for_shred(&self, node: &NodeHandle) -> Result<File, ShredError> {
        let table = self.lock_table()?;
        let entry = table.entries.get(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure node handle".to_string())
        })?;
        if entry.identity.kind() != NodeKind::RegularFile {
            return Err(ShredError::ValidationFailed(
                "secure shred open requires a regular file".to_string(),
            ));
        }
        let fd = entry.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("regular file handle has no descriptor".to_string())
        })?;
        let identity = identity_from_stat(
            &rustix::fs::fstat(fd).map_err(|error| io_error("inspect shred handle", error))?,
        );
        if identity != entry.identity {
            return Err(ShredError::ValidationFailed(
                "shred handle identity changed".to_string(),
            ));
        }
        let duplicate = dup(fd).map_err(|error| io_error("duplicate shred handle", error))?;
        let raw_fd = duplicate.into_raw_fd();
        // SAFETY: `raw_fd` is transferred from the owned duplicate exactly once
        // and is therefore owned by the returned `File`.
        Ok(unsafe { File::from_raw_fd(raw_fd) })
    }

    fn rename_noreplace(
        &self,
        parent: &DirHandle,
        node: &NodeHandle,
        new_name: &OsStr,
    ) -> Result<RenamedNode, ShredError> {
        validate_component(new_name)?;
        let mut table = self.lock_table()?;
        let target = Self::resolve_mutation(&table, parent, node)?;
        rustix::fs::renameat_with(
            target.parent_fd,
            target.node_entry.name.as_os_str(),
            target.parent_fd,
            new_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| {
            if is_unsupported_noreplace(error) {
                ShredError::ValidationFailed(format!(
                    "safe no-replace rename is unsupported on this platform/filesystem: {error}"
                ))
            } else if error == Errno::EXIST {
                ShredError::ValidationFailed(
                    "safe no-replace rename rejected an existing destination".to_string(),
                )
            } else {
                io_error("safe no-replace rename", error)
            }
        })?;

        let mutation_parent_id = target.mutation_parent_id;
        drop(target);
        let node_entry = table.entries.get_mut(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("renamed secure node handle disappeared".to_string())
        })?;
        node_entry.name = new_name.to_os_string();
        node_entry.parent_id = Some(mutation_parent_id);
        Ok(RenamedNode::new())
    }

    fn unlink_leaf(&self, parent: &DirHandle, node: &NodeHandle) -> Result<(), ShredError> {
        let mut table = self.lock_table()?;
        let target = Self::resolve_mutation(&table, parent, node)?;
        if target.node_entry.identity.kind() == NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "unlink_leaf received a directory handle".to_string(),
            ));
        }
        rustix::fs::unlinkat(target.parent_fd, &target.node_entry.name, AtFlags::empty())
            .map_err(|error| io_error("unlink leaf", error))?;
        drop(target);
        table.entries.remove(&node.id());
        Ok(())
    }

    fn remove_empty_dir(&self, parent: &DirHandle, node: &NodeHandle) -> Result<(), ShredError> {
        let mut table = self.lock_table()?;
        let target = Self::resolve_mutation(&table, parent, node)?;
        if target.node_entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "remove_empty_dir received a non-directory handle".to_string(),
            ));
        }
        rustix::fs::unlinkat(
            target.parent_fd,
            &target.node_entry.name,
            AtFlags::REMOVEDIR,
        )
        .map_err(|error| io_error("remove empty directory", error))?;
        let retain_root_handle = parent.id() == node.id();
        drop(target);
        if retain_root_handle {
            let node_entry = table.entries.get_mut(&node.id()).ok_or_else(|| {
                ShredError::ValidationFailed("removed root handle disappeared".to_string())
            })?;
            node_entry.removed = true;
        } else {
            table.entries.remove(&node.id());
        }
        Ok(())
    }

    fn sync_parent(&self, parent: &DirHandle) -> Result<(), ShredError> {
        let mut table = self.lock_table()?;
        let entry = table.entries.get(&parent.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure sync parent handle".to_string())
        })?;
        let removed = entry.removed;
        let sync_id = if removed {
            entry.parent_id.ok_or_else(|| {
                ShredError::ValidationFailed("removed root has no containing directory".to_string())
            })?
        } else {
            parent.id()
        };
        let sync_entry = table.entries.get(&sync_id).ok_or_else(|| {
            ShredError::ValidationFailed("secure sync parent handle is missing".to_string())
        })?;
        if sync_entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "sync parent is not a directory".to_string(),
            ));
        }
        let fd = sync_entry.fd.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("sync parent has no descriptor".to_string())
        })?;
        let result = match rustix::fs::fsync(fd) {
            Ok(()) => Ok(()),
            #[cfg(target_os = "macos")]
            Err(Errno::INVAL) => Ok(()),
            Err(error) => Err(io_error("sync parent", error)),
        };
        if result.is_ok() && removed {
            table.entries.remove(&parent.id());
        }
        result
    }
}

enum OpenedComponent {
    Handle(OwnedFd),
    Link(rustix::fs::Stat),
}

fn open_root_directory() -> Result<OwnedFd, ShredError> {
    rustix::fs::openat(
        rustix::fs::CWD,
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| io_error("open filesystem root", error))
}

fn open_component<Fd: AsFd>(
    parent: Fd,
    name: &OsStr,
    allow_link: bool,
) -> Result<OpenedComponent, Errno> {
    #[cfg(target_os = "linux")]
    {
        let resolve = rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV;
        match openat2_component(&parent, name, OFlags::RDWR | OFlags::CLOEXEC, resolve) {
            Ok(fd) => return Ok(OpenedComponent::Handle(fd)),
            Err(Errno::ISDIR) => {
                match openat2_component(
                    &parent,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    resolve,
                ) {
                    Ok(fd) => return Ok(OpenedComponent::Handle(fd)),
                    Err(Errno::LOOP) if allow_link => return link_stat(&parent, name),
                    Err(Errno::NOSYS | Errno::INVAL) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(Errno::LOOP) if allow_link => return link_stat(&parent, name),
            Err(Errno::NOSYS | Errno::INVAL) => {}
            Err(error) => return Err(error),
        }
    }

    fallback_open_component(parent, name, allow_link)
}

#[cfg(target_os = "linux")]
fn openat2_component<Fd: AsFd>(
    parent: Fd,
    name: &OsStr,
    flags: OFlags,
    resolve: rustix::fs::ResolveFlags,
) -> Result<OwnedFd, Errno> {
    rustix::fs::openat2(parent, name, flags, Mode::empty(), resolve)
}

fn fallback_open_component<Fd: AsFd>(
    parent: Fd,
    name: &OsStr,
    allow_link: bool,
) -> Result<OpenedComponent, Errno> {
    // Linux bind mounts can share a device ID, so the st_dev fallback cannot
    // distinguish them. This fallback remains no-follow and fails closed; it
    // does not relax no-replace or link protections.
    let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match rustix::fs::openat(&parent, name, flags, Mode::empty()) {
        Ok(fd) => Ok(OpenedComponent::Handle(fd)),
        Err(Errno::ISDIR) => rustix::fs::openat(
            &parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(OpenedComponent::Handle),
        Err(Errno::LOOP) if allow_link => link_stat(&parent, name),
        Err(error) => Err(error),
    }
}

fn link_stat<Fd: AsFd>(parent: Fd, name: &OsStr) -> Result<OpenedComponent, Errno> {
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink {
        Ok(OpenedComponent::Link(stat))
    } else {
        Err(Errno::LOOP)
    }
}

fn validate_absolute_path(path: &Path) -> Result<(), ShredError> {
    if !path.is_absolute() {
        return Err(ShredError::ValidationFailed(
            "secure Unix root must be absolute".to_string(),
        ));
    }
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err(ShredError::ValidationFailed(
                "secure Unix root contains a non-normal component".to_string(),
            ));
        }
    }
    Ok(())
}

fn normal_components(path: &Path) -> Result<Vec<OsString>, ShredError> {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_os_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Err(ShredError::ValidationFailed(
            "filesystem roots are not safe execution targets".to_string(),
        ));
    }
    Ok(components)
}

fn validate_component(name: &OsStr) -> Result<(), ShredError> {
    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) if !name.as_bytes().is_empty() => Ok(()),
        _ => Err(ShredError::ValidationFailed(
            "secure child name is not a single normal component".to_string(),
        )),
    }
}

fn identity_from_stat(stat: &rustix::fs::Stat) -> NodeIdentity {
    let kind = match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => NodeKind::RegularFile,
        FileType::Directory => NodeKind::Directory,
        FileType::Symlink => NodeKind::Link,
        _ => NodeKind::Special,
    };
    // Journal recovery defines the stable Unix identity as st_dev + st_ino.
    // Do not substitute Linux's optional mount ID: mount_id is deliberately
    // st_dev for parity with journal metadata and portable fallback behavior.
    NodeIdentity::new(stat.st_ino as u128, stat.st_dev as u64, kind)
}

fn volume_from_identity(identity: NodeIdentity) -> VolumeIdentity {
    VolumeIdentity::new(identity.mount_id())
}

fn volume_from_stat(stat: &rustix::fs::Stat) -> VolumeIdentity {
    VolumeIdentity::new(stat.st_dev as u64)
}

fn is_directory(kind: NodeKind) -> bool {
    kind == NodeKind::Directory
}

fn ensure_local_volume(path: &Path) -> Result<(), ShredError> {
    #[cfg(target_os = "linux")]
    {
        return crate::shredder::platform::linux::ensure_local_volume(path);
    }
    #[cfg(target_os = "macos")]
    {
        return crate::shredder::platform::macos::ensure_local_volume(path);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        Err(ShredError::ValidationFailed(
            "secure Unix adapter has no local-volume policy for this platform".to_string(),
        ))
    }
}

fn io_error(operation: &str, error: Errno) -> ShredError {
    ShredError::IoError {
        path: Path::new("<handle-relative>").to_path_buf(),
        kind: operation.to_string(),
        message: error.to_string(),
    }
}

fn is_unsupported_noreplace(error: Errno) -> bool {
    if matches!(error, Errno::NOSYS | Errno::INVAL | Errno::OPNOTSUPP) {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        return error == Errno::NOTSUP;
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::UnixSecureTreeIo;
    use crate::shredder::root_execution::{NodeKind, SecureTreeIo};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    fn assert_error_contains<T: std::fmt::Debug>(
        result: Result<T, crate::shredder::ShredError>,
        expected: &str,
    ) {
        let error = result.expect_err("operation must fail");
        assert!(
            error.to_string().contains(expected),
            "expected error containing {expected:?}, got {error}"
        );
    }

    #[test]
    fn rejects_symlink_roots_and_never_follows_symlink_children() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let real_root = fixture.path().join("real-root");
        fs::create_dir(&real_root).expect("real root");
        fs::write(real_root.join("outside-data"), b"preserve").expect("outside data");
        let symlink_root = fixture.path().join("symlink-root");
        symlink(&real_root, &symlink_root).expect("symlink root");

        let io = UnixSecureTreeIo::new();
        assert_error_contains(io.open_root_nofollow(&symlink_root), "symlink");

        let root = io.open_root_nofollow(&real_root).expect("open real root");
        let child_link = real_root.join("child-link");
        symlink("outside-data", &child_link).expect("child symlink");
        let child = io
            .open_child_nofollow(&root, OsStr::new("child-link"))
            .expect("open child symlink without following it");
        assert_eq!(
            io.identity(&child).expect("child identity").kind(),
            NodeKind::Link
        );
        assert!(real_root.join("outside-data").exists());
    }

    #[test]
    fn link_replaced_after_enumeration_is_opened_as_link() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("entry"), b"preserve").expect("entry");

        let io = UnixSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        let names = io.enumerate(&root).expect("enumerate root");
        assert_eq!(names.len(), 1);
        fs::remove_file(root_path.join("entry")).expect("remove original entry");
        symlink("/definitely/not-the-entry", root_path.join("entry")).expect("replacement link");

        let child = io
            .open_child_nofollow(&root, Path::new("entry").as_os_str())
            .expect("open replacement without following it");
        assert_eq!(
            io.identity(&child).expect("replacement identity").kind(),
            NodeKind::Link
        );
    }

    #[test]
    fn detects_mount_crossing_through_the_volume_policy_seam() {
        let same_volume = super::VolumeIdentity::new(7);
        let different_volume = super::VolumeIdentity::new(8);

        assert!(super::same_volume(same_volume, same_volume));
        assert!(!super::same_volume(same_volume, different_volume));
    }

    #[test]
    fn no_replace_collision_preserves_both_entries_and_handle_state() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("source"), b"source").expect("source");
        fs::write(root_path.join("destination"), b"destination").expect("destination");

        let io = UnixSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        let source = io
            .open_child_nofollow(&root, OsStr::new("source"))
            .expect("open source");
        assert_error_contains(
            io.rename_noreplace(&root, &source, OsStr::new("destination")),
            "destination",
        );
        assert_eq!(
            fs::read(root_path.join("source")).expect("source remains"),
            b"source"
        );
        assert_eq!(
            fs::read(root_path.join("destination")).expect("destination remains"),
            b"destination"
        );

        fs::remove_file(root_path.join("destination")).expect("remove collision fixture");
        io.rename_noreplace(&root, &source, OsStr::new("destination"))
            .expect("rename after collision target is removed");
        assert!(!root_path.join("source").exists());
        assert!(root_path.join("destination").exists());

        io.unlink_leaf(&root, &source)
            .expect("delete renamed source by retained handle");
        assert!(!root_path.join("destination").exists());
    }

    #[test]
    fn identity_mismatch_blocks_unlink_of_replaced_name() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("entry"), b"original").expect("entry");

        let io = UnixSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        let entry = io
            .open_child_nofollow(&root, OsStr::new("entry"))
            .expect("open entry");
        fs::remove_file(root_path.join("entry")).expect("remove original");
        fs::write(root_path.join("entry"), b"replacement").expect("replacement");

        assert_error_contains(io.unlink_leaf(&root, &entry), "identity");
        assert_eq!(
            fs::read(root_path.join("entry")).expect("replacement remains"),
            b"replacement"
        );
    }

    #[test]
    fn identity_maps_inode_to_id_and_device_to_mount_id() {
        use std::os::unix::fs::MetadataExt;

        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("entry"), b"entry").expect("entry");

        let io = UnixSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        let child = io
            .open_child_nofollow(&root, OsStr::new("entry"))
            .expect("open child");
        let identity = io.identity(&child).expect("child identity");
        let metadata = fs::symlink_metadata(root_path.join("entry")).expect("metadata");

        assert_eq!(identity.id(), metadata.ino() as u128);
        assert_eq!(identity.mount_id(), metadata.dev());
    }

    #[test]
    fn root_directory_retains_its_containing_handle_until_sync() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");

        let io = UnixSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        io.remove_empty_dir(&root, &root.as_node())
            .expect("remove root directory");
        io.sync_parent(&root).expect("sync containing directory");

        assert!(!root_path.exists());
    }
}
