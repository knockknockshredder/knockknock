use super::plan::{ChildName, DirHandle, NodeHandle, NodeIdentity, NodeKind, RenamedNode};
use super::SecureTreeIo;
use crate::shredder::errors::ShredError;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::mem::{size_of, MaybeUninit};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Component, Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{Mutex, MutexGuard};

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FileIdBothDirectoryInformation, NtOpenFile, NtQueryDirectoryFile, NtSetInformationFile,
};
use windows_sys::Win32::Foundation::{
    DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FileAttributeTagInfo, FileDispositionInfoEx, FlushFileBuffers, GetDriveTypeW,
    GetFileInformationByHandle, GetFileInformationByHandleEx, SetFileInformationByHandle,
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_ATTRIBUTE_TAG_INFO, FILE_INFO_BY_HANDLE_CLASS,
};
use windows_sys::Win32::System::Kernel::{OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE};
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::IO::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0};

const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;
const FILE_READ_DATA: u32 = 0x0001;
const FILE_WRITE_DATA: u32 = 0x0002;
const FILE_SHARE_READ_VALUE: u32 = 0x0001;
const FILE_SHARE_WRITE_VALUE: u32 = 0x0002;
const FILE_SHARE_DELETE_VALUE: u32 = 0x0004;
const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
const FILE_ADD_FILE: u32 = 0x0002; // == FILE_WRITE_DATA: create entries in a directory
const FILE_DELETE_CHILD: u32 = 0x0040;
const DELETE: u32 = 0x0001_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
const FILE_DISPOSITION_FLAG_DELETE: u32 = 0x0000_0001;
const FILE_DISPOSITION_FLAG_POSIX_SEMANTICS: u32 = 0x0000_0002;
const STATUS_INVALID_PARAMETER: i32 = 0xC000_000D_u32 as i32;
const STATUS_OBJECT_NAME_COLLISION: i32 = 0xC000_0035_u32 as i32;
const STATUS_NO_MORE_FILES: i32 = 0x8000_0006_u32 as i32;
const STATUS_SUCCESS: i32 = 0;

const FOS_PICKFOLDERS: u32 = 0x20;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const FOS_PATHMUSTEXIST: u32 = 0x800;
const FOS_FILEMUSTEXIST: u32 = 0x1000;
const FOS_ALLOWMULTISELECT: u32 = 0x200;
const FOS_NODEREFERENCELINKS: u32 = 0x0010_0000;

struct HandleEntry {
    handle: Option<OwnedHandle>,
    identity: NodeIdentity,
    name: OsString,
    parent_id: Option<u64>,
    parent_identity: Option<NodeIdentity>,
    removed: bool,
}

#[derive(Default)]
struct HandleTable {
    next_id: u64,
    entries: HashMap<u64, HandleEntry>,
}

pub(crate) struct WindowsSecureTreeIo {
    table: Mutex<HandleTable>,
}

impl WindowsSecureTreeIo {
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
            ShredError::ValidationFailed("secure Windows handle table is poisoned".to_string())
        })
    }

    fn insert_entry(&self, entry: HandleEntry) -> Result<u64, ShredError> {
        let mut table = self.lock_table()?;
        let id = table.next_id;
        table.next_id = table.next_id.checked_add(1).ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows handle table is exhausted".to_string())
        })?;
        table.entries.insert(id, entry);
        Ok(id)
    }

    fn open_root(&self, path: &Path) -> Result<DirHandle, ShredError> {
        let (drive_letter, components) = validate_root_path(path)?;
        let volume_root = format!("{}:\\", drive_letter as char);
        let volume_handle = open_fixed_volume_root(&volume_root)?;
        let volume_identity = query_identity(volume_handle.as_raw_handle())?;
        let mut current = volume_handle;
        let mut retained_parent = None;
        let mut retained_parent_identity = None;

        for (index, component) in components.iter().enumerate() {
            let final_component = index + 1 == components.len();
            if final_component {
                retained_parent = Some(duplicate_owned_handle(&current)?);
                retained_parent_identity = Some(query_identity(current.as_raw_handle())?);
            }

            let role = if final_component {
                OpenRole::Destructive
            } else if index + 2 == components.len() {
                OpenRole::RenameParent
            } else {
                OpenRole::Traverse
            };
            let opened = open_relative(current.as_raw_handle(), component, role)?;
            if opened.identity.kind() == NodeKind::Link {
                return Err(ShredError::ValidationFailed(
                    "reparse point in execution root is not safe".to_string(),
                ));
            }
            if !final_component && opened.identity.kind() != NodeKind::Directory {
                return Err(ShredError::ValidationFailed(
                    "root path component is not a directory".to_string(),
                ));
            }
            if opened.identity.mount_id() != volume_identity.mount_id() {
                return Err(ShredError::ValidationFailed(
                    "volume crossing detected while opening root".to_string(),
                ));
            }
            current = opened.handle;
        }

        let identity = query_identity(current.as_raw_handle())?;
        if identity.kind() == NodeKind::Link {
            return Err(ShredError::ValidationFailed(
                "reparse point in execution root is not safe".to_string(),
            ));
        }
        let name = components.last().cloned().ok_or_else(|| {
            ShredError::ValidationFailed(
                "filesystem roots are not safe execution targets".to_string(),
            )
        })?;
        let parent_handle = retained_parent.ok_or_else(|| {
            ShredError::ValidationFailed("root containing directory is unavailable".to_string())
        })?;
        let parent_identity = retained_parent_identity.ok_or_else(|| {
            ShredError::ValidationFailed(
                "root containing directory identity is unavailable".to_string(),
            )
        })?;
        let parent_id = self.insert_entry(HandleEntry {
            handle: Some(parent_handle),
            identity: parent_identity,
            name: OsString::from("."),
            parent_id: None,
            parent_identity: None,
            removed: false,
        })?;
        let root_id = self.insert_entry(HandleEntry {
            handle: Some(current),
            identity,
            name,
            parent_id: Some(parent_id),
            parent_identity: Some(parent_identity),
            removed: false,
        })?;
        Ok(DirHandle::new(root_id))
    }

    fn open_child(&self, parent: &DirHandle, name: &OsStr) -> Result<NodeHandle, ShredError> {
        validate_component(name)?;
        let table = self.lock_table()?;
        let parent_entry = table.entries.get(&parent.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure Windows parent handle".to_string())
        })?;
        let parent_handle = parent_entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows parent handle is closed".to_string())
        })?;
        if parent_entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "child open parent is not a directory".to_string(),
            ));
        }
        verify_entry_identity(parent_entry, parent_handle.as_raw_handle())?;
        let opened = open_relative(parent_handle.as_raw_handle(), name, OpenRole::Destructive)?;
        if opened.identity.mount_id() != parent_entry.identity.mount_id() {
            return Err(ShredError::ValidationFailed(
                "volume crossing detected while opening child".to_string(),
            ));
        }
        let parent_identity = parent_entry.identity;
        drop(table);
        let id = self.insert_entry(HandleEntry {
            handle: Some(opened.handle),
            identity: opened.identity,
            name: name.to_os_string(),
            parent_id: Some(parent.id()),
            parent_identity: Some(parent_identity),
            removed: false,
        })?;
        Ok(NodeHandle::new(id))
    }

    fn resolve_mutation<'a>(
        table: &'a HandleTable,
        parent: &DirHandle,
        node: &NodeHandle,
    ) -> Result<MutationTarget<'a>, ShredError> {
        let node_entry = table.entries.get(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure Windows node handle".to_string())
        })?;
        let _requested_parent = table.entries.get(&parent.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure Windows parent handle".to_string())
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
        let mutation_parent = table.entries.get(&actual_parent_id).ok_or_else(|| {
            ShredError::ValidationFailed(
                "retained containing directory handle is missing".to_string(),
            )
        })?;
        let parent_handle = mutation_parent.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure mutation parent handle is closed".to_string())
        })?;
        if mutation_parent.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "secure mutation parent is not a directory".to_string(),
            ));
        }
        verify_entry_identity(mutation_parent, parent_handle.as_raw_handle())?;
        let node_handle = node_entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure mutation node handle is closed".to_string())
        })?;
        verify_entry_identity(node_entry, node_handle.as_raw_handle())?;
        if node_entry.parent_identity != Some(mutation_parent.identity) {
            return Err(ShredError::ValidationFailed(
                "mutation parent identity mismatch".to_string(),
            ));
        }
        let current_name = open_relative(
            parent_handle.as_raw_handle(),
            node_entry.name.as_os_str(),
            OpenRole::Traverse,
        )?;
        if current_name.identity != node_entry.identity {
            return Err(ShredError::ValidationFailed(
                "mutation target identity mismatch".to_string(),
            ));
        }
        Ok(MutationTarget {
            parent: mutation_parent,
            node: node_entry,
            parent_id: actual_parent_id,
        })
    }
}

struct OpenedComponent {
    handle: OwnedHandle,
    identity: NodeIdentity,
}

/// Access-rights role of a single relative component open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenRole {
    /// Intermediate path component: traversal/read only, never DELETE.
    Traverse,
    /// Directory that hosts the no-replace rename of the root (its containing
    /// directory): traversal plus rename-destination rights. Deliberately
    /// omits DELETE so a directory pinned by the shell without
    /// FILE_SHARE_DELETE can still be opened.
    RenameParent,
    /// The destructive root itself: full mutation access including DELETE,
    /// which is inherent to destroying the selected root.
    Destructive,
}

struct MutationTarget<'a> {
    parent: &'a HandleEntry,
    node: &'a HandleEntry,
    parent_id: u64,
}

impl SecureTreeIo for WindowsSecureTreeIo {
    fn open_root_nofollow(&self, path: &Path) -> Result<DirHandle, ShredError> {
        self.open_root(path)
    }

    fn enumerate(&self, dir: &DirHandle) -> Result<Vec<ChildName>, ShredError> {
        let table = self.lock_table()?;
        let entry = table.entries.get(&dir.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure Windows directory handle".to_string())
        })?;
        let handle = entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows directory handle is closed".to_string())
        })?;
        if entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "cannot enumerate a non-directory handle".to_string(),
            ));
        }
        verify_entry_identity(entry, handle.as_raw_handle())?;
        enumerate_handle(handle.as_raw_handle())
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
            ShredError::ValidationFailed("unknown secure Windows node handle".to_string())
        })?;
        let handle = entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows node handle is closed".to_string())
        })?;
        verify_entry_identity(entry, handle.as_raw_handle())
    }

    fn open_regular_for_shred(&self, node: &NodeHandle) -> Result<File, ShredError> {
        let table = self.lock_table()?;
        let entry = table.entries.get(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("unknown secure Windows node handle".to_string())
        })?;
        if entry.identity.kind() != NodeKind::RegularFile {
            return Err(ShredError::ValidationFailed(
                "secure shred open requires a regular file".to_string(),
            ));
        }
        let handle = entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows shred handle is closed".to_string())
        })?;
        verify_entry_identity(entry, handle.as_raw_handle())?;
        let duplicate = duplicate_raw_handle(handle.as_raw_handle())?;
        // SAFETY: `duplicate` is a newly owned handle returned by DuplicateHandle;
        // ownership is transferred exactly once to the returned File.
        Ok(unsafe { File::from_raw_handle(duplicate as RawHandle) })
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
        let source = target.node.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("rename source handle is closed".to_string())
        })?;
        let destination = encode_component(new_name)?;
        let mut bytes =
            vec![0u8; size_of::<RawRenameInfo>() - size_of::<u16>() + destination.len() * 2];
        let info = bytes.as_mut_ptr() as *mut RawRenameInfo;
        // SAFETY: `bytes` is sized for the fixed header plus the UTF-16 tail and
        // remains alive for the synchronous native rename call.
        unsafe {
            (*info).replace_if_exists = 0;
            (*info).root_directory = target
                .parent
                .handle
                .as_ref()
                .ok_or_else(|| {
                    ShredError::ValidationFailed(
                        "rename mutation parent handle is closed".to_string(),
                    )
                })?
                .as_raw_handle();
            (*info).file_name_length = (destination.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(
                destination.as_ptr(),
                (*info).file_name.as_mut_ptr(),
                destination.len(),
            );
            let mut io_status = IO_STATUS_BLOCK {
                Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
                Information: 0,
            };
            let status = NtSetInformationFile(
                source.as_raw_handle() as HANDLE,
                &mut io_status,
                info.cast(),
                bytes.len() as u32,
                10,
            ) as i32;
            if status == STATUS_OBJECT_NAME_COLLISION {
                return Err(ShredError::ValidationFailed(
                    "safe no-replace rename rejected an existing destination".to_string(),
                ));
            }
            if status != STATUS_SUCCESS {
                return Err(nt_error("safe no-replace rename", status));
            }
        }

        let parent_id = target.parent_id;
        let parent_identity = target.parent.identity;
        drop(target);
        let node_entry = table.entries.get_mut(&node.id()).ok_or_else(|| {
            ShredError::ValidationFailed("renamed secure node handle disappeared".to_string())
        })?;
        node_entry.name = new_name.to_os_string();
        node_entry.parent_id = Some(parent_id);
        node_entry.parent_identity = Some(parent_identity);
        Ok(RenamedNode::new())
    }

    fn unlink_leaf(&self, parent: &DirHandle, node: &NodeHandle) -> Result<(), ShredError> {
        let mut table = self.lock_table()?;
        let target = Self::resolve_mutation(&table, parent, node)?;
        if target.node.identity.kind() == NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "unlink_leaf received a directory handle".to_string(),
            ));
        }
        set_delete_disposition(target.node.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("unlink node handle is closed".to_string())
        })?)?;
        table.entries.remove(&node.id());
        Ok(())
    }

    fn remove_empty_dir(&self, parent: &DirHandle, node: &NodeHandle) -> Result<(), ShredError> {
        let mut table = self.lock_table()?;
        let target = Self::resolve_mutation(&table, parent, node)?;
        if target.node.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "remove_empty_dir received a non-directory handle".to_string(),
            ));
        }
        set_delete_disposition(target.node.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("remove node handle is closed".to_string())
        })?)?;
        let retain_root_handle = parent.id() == node.id();
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
            ShredError::ValidationFailed("unknown secure Windows sync parent handle".to_string())
        })?;
        let sync_id = if entry.removed {
            entry.parent_id.ok_or_else(|| {
                ShredError::ValidationFailed("removed root has no containing directory".to_string())
            })?
        } else {
            parent.id()
        };
        let sync_entry = table.entries.get(&sync_id).ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows sync parent handle is missing".to_string())
        })?;
        let handle = sync_entry.handle.as_ref().ok_or_else(|| {
            ShredError::ValidationFailed("secure Windows sync parent handle is closed".to_string())
        })?;
        if sync_entry.identity.kind() != NodeKind::Directory {
            return Err(ShredError::ValidationFailed(
                "sync parent is not a directory".to_string(),
            ));
        }
        verify_entry_identity(sync_entry, handle.as_raw_handle())?;
        // SAFETY: the handle is owned by the table and remains valid while the
        // mutex guard is held for this synchronous flush.
        if unsafe { FlushFileBuffers(handle.as_raw_handle()) } == 0 {
            return Err(io_error("sync parent", std::io::Error::last_os_error()));
        }
        if entry.removed {
            table.entries.remove(&parent.id());
        }
        Ok(())
    }
}

#[repr(C)]
struct RawRenameInfo {
    replace_if_exists: i32,
    root_directory: HANDLE,
    file_name_length: u32,
    file_name: [u16; 1],
}

#[repr(C)]
struct RawDispositionInfoEx {
    flags: u32,
}

fn open_fixed_volume_root(root: &str) -> Result<OwnedHandle, ShredError> {
    let wide = root
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: `wide` is NUL-terminated and all other arguments are constants or
    // null optional pointers. This is the only full-path CreateFileW call in the
    // adapter; all selected components use RootDirectory-relative NtOpenFile.
    let handle = unsafe {
        windows::Win32::Storage::FileSystem::CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_GENERIC_READ.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                | windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            None,
            windows::Win32::Storage::FileSystem::OPEN_EXISTING,
            windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
                | windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|error| {
        io_error(
            "open fixed volume root",
            std::io::Error::from_raw_os_error(error.code().0),
        )
    })?;
    if handle.0 == INVALID_HANDLE_VALUE || handle.0.is_null() {
        return Err(io_error(
            "open fixed volume root",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: CreateFileW returned a valid owned handle and this function returns
    // it through OwnedHandle exactly once.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0 as RawHandle) })
}

fn open_relative(
    parent: RawHandle,
    component: &OsStr,
    role: OpenRole,
) -> Result<OpenedComponent, ShredError> {
    let name = encode_component(component)?;
    let mut unicode = windows_sys::Win32::Foundation::UNICODE_STRING {
        Length: (name.len() * 2) as u16,
        MaximumLength: (name.len() * 2) as u16,
        Buffer: name.as_ptr() as *mut u16,
    };
    let mut attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent as HANDLE,
        ObjectName: &mut unicode,
        Attributes: (OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE) as u32,
        SecurityDescriptor: null_mut(),
        SecurityQualityOfService: null_mut(),
    };
    let mut handle = null_mut();
    let mut io_status = IO_STATUS_BLOCK {
        Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
        Information: 0,
    };
    // Only the destructive root requests DELETE. Intermediates stay
    // traversal/read-only and the root's containing directory adds exactly
    // the rename-destination rights (FILE_ADD_FILE | FILE_DELETE_CHILD) the
    // no-replace rename requires on its root_directory handle. Requesting
    // DELETE on a directory held by the shell without FILE_SHARE_DELETE
    // fails with STATUS_SHARING_VIOLATION, which is why the reduced
    // containing-directory mask must never include DELETE. The base mask
    // (FILE_READ_DATA == FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    // SYNCHRONIZE) covers enumeration, identity checks, and relative
    // traversal; FILE_ADD_FILE (== FILE_WRITE_DATA) also satisfies
    // FlushFileBuffers on the retained parent handle.
    let desired_access = match role {
        OpenRole::Traverse => FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        OpenRole::RenameParent => {
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE | FILE_ADD_FILE | FILE_DELETE_CHILD
        }
        OpenRole::Destructive => {
            FILE_READ_DATA
                | FILE_READ_ATTRIBUTES
                | SYNCHRONIZE
                | FILE_WRITE_DATA
                | FILE_WRITE_ATTRIBUTES
                | DELETE
        }
    };
    let share_access = FILE_SHARE_READ_VALUE | FILE_SHARE_WRITE_VALUE | FILE_SHARE_DELETE_VALUE;
    // Synchronous handles only: the open-file shredder (and any other std
    // file I/O) issues synchronous ReadFile/WriteFile calls with a null
    // OVERLAPPED, which Windows rejects with STATUS_INVALID_PARAMETER on
    // asynchronous handles. Without this flag no file could ever be shredded
    // through the secure adapter.
    let open_options = FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT;
    // SAFETY: all pointers refer to stack values or the live UTF-16 component;
    // NtOpenFile is synchronous and consumes only the one relative component.
    let status = unsafe {
        NtOpenFile(
            &mut handle,
            desired_access,
            &attributes,
            &mut io_status,
            share_access,
            open_options,
        )
    } as i32;
    if status == STATUS_INVALID_PARAMETER {
        // Windows releases predating OBJ_DONT_REPARSE reject the flag with
        // STATUS_INVALID_PARAMETER. Retry the exact same one-component,
        // RootDirectory-relative open while retaining FILE_OPEN_REPARSE_POINT;
        // tag verification below still prevents following any reparse node.
        attributes.Attributes = OBJ_CASE_INSENSITIVE as u32;
        handle = null_mut();
        io_status = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        // SAFETY: this is the documented compatibility retry described above;
        // it remains relative to the trusted parent handle and opens no path.
        let retry_status = unsafe {
            NtOpenFile(
                &mut handle,
                desired_access,
                &attributes,
                &mut io_status,
                share_access,
                open_options,
            )
        } as i32;
        if retry_status != STATUS_SUCCESS {
            return Err(nt_error("open relative component", retry_status));
        }
    } else if status != STATUS_SUCCESS {
        return Err(nt_error("open relative component", status));
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(ShredError::ValidationFailed(
            "relative native open returned an invalid handle".to_string(),
        ));
    }
    // SAFETY: NtOpenFile returned a valid handle and this function assumes its
    // ownership exactly once.
    let owned = unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) };
    let identity = query_identity(owned.as_raw_handle())?;
    Ok(OpenedComponent {
        handle: owned,
        identity,
    })
}

fn enumerate_handle(handle: RawHandle) -> Result<Vec<ChildName>, ShredError> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut names = Vec::new();
    let mut restart_scan = true;
    loop {
        let mut io_status = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        // SAFETY: `buffer` is writable for its full length and the directory
        // handle is trusted. The query is synchronous with no APC callback.
        let status = unsafe {
            NtQueryDirectoryFile(
                handle as HANDLE,
                null_mut(),
                None,
                null(),
                &mut io_status,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
                FileIdBothDirectoryInformation,
                0,
                null(),
                restart_scan as u8,
            )
        } as i32;
        if status == STATUS_NO_MORE_FILES {
            break;
        }
        if status != STATUS_SUCCESS {
            return Err(nt_error("enumerate trusted directory handle", status));
        }

        let bytes_used = io_status.Information;
        if bytes_used == 0 {
            break;
        }
        // The 8-byte alignment of the FileId field pushes FileName to offset
        // 104, not the 102 implied by the unaligned field listing. Parsing
        // against the wrong header reads names six bytes late and rejects the
        // final record of every enumeration.
        let header_length = file_name_offset();
        let mut offset = 0usize;
        while offset < bytes_used {
            if bytes_used - offset < size_of::<u32>() {
                return Err(ShredError::ValidationFailed(
                    "directory enumeration returned a truncated record".to_string(),
                ));
            }
            // SAFETY: the kernel filled `buffer`; bounds are checked before each
            // record and the structure is only read within the returned bytes.
            let record = unsafe {
                &*(buffer.as_ptr().add(offset)
                    as *const windows_sys::Wdk::Storage::FileSystem::FILE_ID_BOTH_DIR_INFORMATION)
            };
            let name_length = record.FileNameLength as usize;
            if name_length % 2 != 0 || offset + header_length + name_length > bytes_used {
                return Err(ShredError::ValidationFailed(
                    "directory enumeration returned an invalid name record".to_string(),
                ));
            }
            // SAFETY: the checked record bounds include the UTF-16 file-name
            // payload, and the slice remains within `buffer` for this iteration.
            let name =
                unsafe { std::slice::from_raw_parts(record.FileName.as_ptr(), name_length / 2) };
            if name != [b'.' as u16] && name != [b'.' as u16, b'.' as u16] {
                names.push(ChildName::new(OsString::from_wide(name)));
            }
            let next = record.NextEntryOffset as usize;
            if next == 0 {
                break;
            }
            if next > bytes_used - offset {
                return Err(ShredError::ValidationFailed(
                    "directory enumeration returned an invalid record offset".to_string(),
                ));
            }
            offset += next;
        }
        restart_scan = false;
    }
    Ok(names)
}

fn file_name_offset() -> usize {
    // offsetof(FILE_ID_BOTH_DIR_INFORMATION, FileName), computed from a
    // zeroed instance so the parser stays correct if the layout changes.
    let instance = windows_sys::Wdk::Storage::FileSystem::FILE_ID_BOTH_DIR_INFORMATION {
        NextEntryOffset: 0,
        FileIndex: 0,
        CreationTime: 0,
        LastAccessTime: 0,
        LastWriteTime: 0,
        ChangeTime: 0,
        EndOfFile: 0,
        AllocationSize: 0,
        FileAttributes: 0,
        FileNameLength: 0,
        EaSize: 0,
        ShortNameLength: 0,
        ShortName: [0; 12],
        FileId: 0,
        FileName: [0; 1],
    };
    (&instance.FileName as *const u16 as usize) - (&instance as *const _ as usize)
}

fn query_identity(handle: RawHandle) -> Result<NodeIdentity, ShredError> {
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: `information` is writable storage for the documented structure and
    // `handle` is owned by the caller.
    if unsafe { GetFileInformationByHandle(handle as HANDLE, information.as_mut_ptr()) } == 0 {
        return Err(io_error(
            "inspect Windows handle",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: GetFileInformationByHandle returned success and initialized the
    // complete BY_HANDLE_FILE_INFORMATION structure.
    let information = unsafe { information.assume_init() };
    let mut tag = FILE_ATTRIBUTE_TAG_INFO {
        FileAttributes: 0,
        ReparseTag: 0,
    };
    // SAFETY: `tag` is writable storage for the documented tag structure.
    if unsafe {
        GetFileInformationByHandleEx(
            handle as HANDLE,
            FileAttributeTagInfo,
            &mut tag as *mut _ as *mut _,
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    } == 0
    {
        return Err(io_error(
            "inspect Windows reparse tag",
            std::io::Error::last_os_error(),
        ));
    }
    let attributes = information.dwFileAttributes;
    let kind = if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        if tag.ReparseTag == 0 {
            return Err(ShredError::ValidationFailed(
                "reparse node has no verifiable reparse tag".to_string(),
            ));
        }
        NodeKind::Link
    } else if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        NodeKind::Directory
    } else {
        NodeKind::RegularFile
    };
    let id = ((information.nFileIndexHigh as u128) << 32) | information.nFileIndexLow as u128;
    Ok(NodeIdentity::new(
        id,
        information.dwVolumeSerialNumber as u64,
        kind,
    ))
}

fn verify_entry_identity(
    entry: &HandleEntry,
    handle: RawHandle,
) -> Result<NodeIdentity, ShredError> {
    let actual = query_identity(handle)?;
    if actual != entry.identity {
        return Err(ShredError::ValidationFailed(
            "Windows handle identity mismatch".to_string(),
        ));
    }
    Ok(actual)
}

fn set_delete_disposition(handle: &OwnedHandle) -> Result<(), ShredError> {
    let information = RawDispositionInfoEx {
        flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
    };
    // SAFETY: `information` is valid for the synchronous handle-based call. No
    // pathname or diagnostic path participates in deletion.
    if unsafe {
        SetFileInformationByHandle(
            handle.as_raw_handle() as HANDLE,
            FileDispositionInfoEx as FILE_INFO_BY_HANDLE_CLASS,
            &information as *const _ as *const _,
            size_of::<RawDispositionInfoEx>() as u32,
        )
    } == 0
    {
        return Err(io_error(
            "delete verified Windows handle",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn duplicate_owned_handle(handle: &OwnedHandle) -> Result<OwnedHandle, ShredError> {
    let raw = duplicate_raw_handle(handle.as_raw_handle())?;
    // SAFETY: DuplicateHandle returned a new owned handle exactly once.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) })
}

fn duplicate_raw_handle(handle: RawHandle) -> Result<HANDLE, ShredError> {
    let mut duplicate = null_mut();
    // SAFETY: the source handle is owned by the caller; the target process is the
    // current process and DuplicateHandle writes one new handle to `duplicate`.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle as HANDLE,
            GetCurrentProcess(),
            &mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io_error(
            "duplicate Windows handle",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(duplicate)
}

fn validate_root_path(path: &Path) -> Result<(u8, Vec<OsString>), ShredError> {
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            std::path::Prefix::Disk(letter) => letter,
            _ => {
                return Err(ShredError::NetworkDrive(path.to_path_buf()));
            }
        },
        _ => {
            return Err(ShredError::ValidationFailed(
                "secure Windows root must be an absolute fixed-drive path".to_string(),
            ));
        }
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(ShredError::ValidationFailed(
            "secure Windows root must be an absolute fixed-drive path".to_string(),
        ));
    }
    let components = components
        .map(|component| match component {
            Component::Normal(name) => Ok(name.to_os_string()),
            _ => Err(ShredError::ValidationFailed(
                "secure Windows root contains a non-normal component".to_string(),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(ShredError::ValidationFailed(
            "filesystem roots are not safe execution targets".to_string(),
        ));
    }
    let drive_root = format!("{}:\\", drive as char);
    let wide = drive_root
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: `wide` is NUL-terminated and points to the fixed drive root only.
    let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
    if drive_type == DRIVE_REMOTE {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }
    if drive_type == DRIVE_REMOVABLE
        || drive_type == DRIVE_CDROM
        || drive_type == DRIVE_RAMDISK
        || drive_type != DRIVE_FIXED
    {
        return Err(ShredError::ValidationFailed(
            "only fixed local Windows volumes are safe execution roots".to_string(),
        ));
    }
    Ok((drive, components))
}

fn validate_component(name: &OsStr) -> Result<(), ShredError> {
    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) if !name.is_empty() => Ok(()),
        _ => Err(ShredError::ValidationFailed(
            "secure Windows child name is not a single normal component".to_string(),
        )),
    }
}

fn encode_component(name: &OsStr) -> Result<Vec<u16>, ShredError> {
    validate_component(name)?;
    Ok(name.encode_wide().collect())
}

fn io_error(operation: &str, error: std::io::Error) -> ShredError {
    ShredError::IoError {
        path: PathBuf::from("<handle-relative>"),
        kind: operation.to_string(),
        message: error.to_string(),
    }
}

fn nt_error(operation: &str, status: i32) -> ShredError {
    ShredError::IoError {
        path: PathBuf::from("<handle-relative>"),
        kind: operation.to_string(),
        message: format!("NTSTATUS 0x{:08X}", status as u32),
    }
}

pub(crate) fn folder_picker_options() -> u32 {
    FOS_PICKFOLDERS | FOS_NODEREFERENCELINKS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
}

pub(crate) fn file_picker_options() -> u32 {
    FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST | FOS_NODEREFERENCELINKS | FOS_PATHMUSTEXIST
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shredder::root_execution::SecureTreeIo;
    use std::ffi::OsStr;
    use std::fs;
    use std::io::Write;
    use std::os::windows::fs::{symlink_dir, symlink_file};
    use std::path::Path;

    fn assert_error_contains<T: std::fmt::Debug>(
        result: Result<T, crate::shredder::ShredError>,
        expected: &str,
    ) {
        let error = result.expect_err("operation must fail");
        assert!(
            error.to_string().to_ascii_lowercase().contains(expected),
            "expected error containing {expected:?}, got {error}"
        );
    }

    fn skip_if_symlink_privilege_error(error: &std::io::Error) -> bool {
        matches!(error.raw_os_error(), Some(5 | 1314))
    }

    #[test]
    fn folder_picker_options_are_non_resolving_multi_path_filesystem_folders() {
        let options = folder_picker_options();
        assert_ne!(options & FOS_PICKFOLDERS, 0);
        assert_ne!(options & FOS_NODEREFERENCELINKS, 0);
        assert_ne!(options & FOS_FORCEFILESYSTEM, 0);
        assert_ne!(options & FOS_PATHMUSTEXIST, 0);
    }

    #[test]
    fn file_picker_options_allow_multiple_raw_filesystem_paths() {
        let options = file_picker_options();
        assert_ne!(options & FOS_ALLOWMULTISELECT, 0);
        assert_ne!(options & FOS_FILEMUSTEXIST, 0);
        assert_ne!(options & FOS_NODEREFERENCELINKS, 0);
        assert_ne!(options & FOS_PATHMUSTEXIST, 0);
    }

    #[test]
    fn rejects_remote_roots_before_opening_a_volume_handle() {
        let io = WindowsSecureTreeIo::new();
        assert_error_contains(
            io.open_root_nofollow(Path::new(r"\\localhost\share\target")),
            "network",
        );
    }

    #[test]
    fn rejects_reparse_roots_and_keeps_reparse_children_as_link_leaves() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let real_root = fixture.path().join("real-root");
        fs::create_dir(&real_root).expect("real root");
        fs::write(real_root.join("outside-data"), b"preserve").expect("outside data");

        let symlink_root = fixture.path().join("symlink-root");
        if let Err(error) = symlink_dir(&real_root, &symlink_root) {
            if skip_if_symlink_privilege_error(&error) {
                return;
            }
            panic!("create symlink root: {error}");
        }

        let io = WindowsSecureTreeIo::new();
        assert_error_contains(io.open_root_nofollow(&symlink_root), "reparse");

        let root = io.open_root_nofollow(&real_root).expect("open real root");
        let child_link = real_root.join("child-link");
        if let Err(error) = symlink_file("outside-data", &child_link) {
            if skip_if_symlink_privilege_error(&error) {
                return;
            }
            panic!("create child symlink: {error}");
        }
        let child = io
            .open_child_nofollow(&root, OsStr::new("child-link"))
            .expect("open child reparse point without following it");
        assert_eq!(
            io.identity(&child).expect("child identity").kind(),
            NodeKind::Link
        );
        io.unlink_leaf(&root, &child)
            .expect("unlink verified link handle");
        assert!(real_root.join("outside-data").exists());
    }

    #[test]
    fn identity_mismatch_blocks_unlink_of_replaced_name() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("entry"), b"original").expect("entry");

        let io = WindowsSecureTreeIo::new();
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
    fn no_replace_collision_preserves_both_entries_and_handle_state() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("source"), b"source").expect("source");
        fs::write(root_path.join("destination"), b"destination").expect("destination");

        let io = WindowsSecureTreeIo::new();
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
    }

    #[test]
    fn enumerate_returns_child_names_without_dot_entries() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");
        fs::write(root_path.join("alpha.txt"), b"a").expect("alpha");
        fs::write(root_path.join("beta"), b"b").expect("beta");
        fs::create_dir(root_path.join("gamma")).expect("gamma");

        let io = WindowsSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        let mut names = io
            .enumerate(&root)
            .expect("enumerate root")
            .into_iter()
            .map(|child| child.as_os_str().to_os_string())
            .collect::<Vec<_>>();
        names.sort();

        assert_eq!(
            names,
            vec![
                OsString::from("alpha.txt"),
                OsString::from("beta"),
                OsString::from("gamma")
            ]
        );
    }

    #[test]
    fn removed_root_syncs_its_retained_containing_directory() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let root_path = fixture.path().join("root");
        fs::create_dir(&root_path).expect("root");

        let io = WindowsSecureTreeIo::new();
        let root = io.open_root_nofollow(&root_path).expect("open root");
        io.remove_empty_dir(&root, &root.as_node())
            .expect("remove root directory");
        io.sync_parent(&root).expect("sync containing directory");

        assert!(!root_path.exists());
    }

    fn open_directory_handle(path: &Path, access: u32, share: u32) -> std::io::Result<OwnedHandle> {
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: `wide` is NUL-terminated for the duration of the call and
        // the returned handle is transferred to the caller exactly once.
        let handle = unsafe {
            windows::Win32::Storage::FileSystem::CreateFileW(
                windows::core::PCWSTR(wide.as_ptr()),
                access,
                windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(share),
                None,
                windows::Win32::Storage::FileSystem::OPEN_EXISTING,
                windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )
        }
        .map_err(|error| {
            // CreateFileW failures surface as HRESULT_FROM_WIN32 codes; the
            // low word is the Win32 error (e.g. ERROR_SHARING_VIOLATION).
            std::io::Error::from_raw_os_error(error.code().0 & 0xFFFF)
        })?;
        if handle.0 == INVALID_HANDLE_VALUE || handle.0.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: CreateFileW returned a valid owned handle exactly once.
        Ok(unsafe { OwnedHandle::from_raw_handle(handle.0 as RawHandle) })
    }

    /// Holds `path` open like the shell does: a share mode that excludes
    /// FILE_SHARE_DELETE, so any open requesting DELETE must be rejected.
    fn pin_directory_without_delete_share(path: &Path) -> OwnedHandle {
        let share = windows::Win32::Storage::FileSystem::FILE_SHARE_READ.0
            | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE.0;
        let access = windows::Win32::Storage::FileSystem::FILE_GENERIC_READ.0
            | windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE.0;
        open_directory_handle(path, access, share).expect("pin directory without FILE_SHARE_DELETE")
    }

    /// Proves the pin is effective: opening the pinned directory with DELETE
    /// must fail with ERROR_SHARING_VIOLATION, so any adapter success below
    /// demonstrates DELETE was never requested on the pinned directory.
    fn assert_delete_open_of_pinned_directory_is_rejected(path: &Path) {
        let share = windows::Win32::Storage::FileSystem::FILE_SHARE_READ.0
            | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE.0;
        let access = windows::Win32::Storage::FileSystem::FILE_GENERIC_READ.0
            | windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE.0
            | DELETE;
        let error = open_directory_handle(path, access, share)
            .expect_err("opening the pinned directory with DELETE must fail");
        assert_eq!(
            error.raw_os_error(),
            Some(windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION as i32),
            "expected a sharing violation, got {error}"
        );
    }

    /// Mirrors the file-root destruction sequence of the execution pipeline
    /// (overwrite, no-replace rename, disposition delete) through the real
    /// Windows adapter.
    fn execute_file_root_through_real_adapter(io: &WindowsSecureTreeIo, file_path: &Path) {
        let root = io
            .open_root_nofollow(file_path)
            .expect("open file root through the real adapter");
        assert_eq!(
            io.identity(&root.as_node()).expect("root identity").kind(),
            NodeKind::RegularFile
        );

        let mut file = io
            .open_regular_for_shred(&root.as_node())
            .expect("open regular file for shred");
        file.write_all(&[0u8; 8192]).expect("overwrite pass");
        file.flush().expect("flush overwrite pass");
        drop(file);

        let obliterated = OsString::from(".knockknock-regression");
        io.rename_noreplace(&root, &root.as_node(), &obliterated)
            .expect("no-replace rename inside pinned directory");
        io.unlink_leaf(&root, &root.as_node())
            .expect("disposition delete of renamed file root");
    }

    #[test]
    fn pinned_containing_directory_is_never_opened_with_delete() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let pinned = fixture.path().join("pinned");
        fs::create_dir(&pinned).expect("pinned directory");
        let file_path = pinned.join("secret.txt");
        fs::write(&file_path, b"top secret").expect("target file");

        let _pin = pin_directory_without_delete_share(&pinned);
        assert_delete_open_of_pinned_directory_is_rejected(&pinned);

        execute_file_root_through_real_adapter(&WindowsSecureTreeIo::new(), &file_path);

        assert!(!file_path.exists(), "file root must be destroyed");
        assert!(pinned.exists(), "pinned containing directory must survive");
        assert_eq!(
            fs::read_dir(&pinned)
                .expect("read pinned directory")
                .count(),
            0,
            "pinned directory must be empty after destruction"
        );
    }

    #[test]
    fn pinned_intermediate_ancestor_allows_deeper_file_root_destruction() {
        let fixture = tempfile::tempdir().expect("temporary fixture");
        let pinned = fixture.path().join("pinned");
        fs::create_dir(&pinned).expect("pinned directory");
        let level = pinned.join("level");
        fs::create_dir(&level).expect("intermediate directory");
        let file_path = level.join("deep.txt");
        fs::write(&file_path, b"deep secret").expect("deep target file");

        let _pin = pin_directory_without_delete_share(&pinned);
        assert_delete_open_of_pinned_directory_is_rejected(&pinned);

        execute_file_root_through_real_adapter(&WindowsSecureTreeIo::new(), &file_path);

        assert!(!file_path.exists(), "deeper file root must be destroyed");
        assert!(pinned.exists(), "pinned intermediate ancestor must survive");
        assert_eq!(
            fs::read_dir(&level)
                .expect("read intermediate directory")
                .count(),
            0,
            "intermediate directory must be empty"
        );
    }
}
