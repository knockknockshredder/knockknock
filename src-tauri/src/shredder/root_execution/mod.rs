pub(crate) mod plan;
pub mod types;

pub use types::{
    BatchRootResult, ChildErrorDto, ExecuteRootRequest, ExecuteRootsRequest, ExecutionStage,
    RootResultDto, RootStatus, TargetAvailability, TargetKind, TargetMetadataDto, VaultError,
    VaultSchemaSource, VaultTarget,
};

pub(crate) use plan::{
    execute_roots, ChildName, DirHandle, FileShredRequest, FileShredResult, NodeHandle,
    NodeIdentity, NodeKind, RenamedNode,
};

pub(crate) trait SecureTreeIo: Send + Sync {
    fn open_root_nofollow(
        &self,
        path: &std::path::Path,
    ) -> Result<DirHandle, crate::shredder::ShredError>;
    fn enumerate(&self, dir: &DirHandle) -> Result<Vec<ChildName>, crate::shredder::ShredError>;
    fn open_child_nofollow(
        &self,
        parent: &DirHandle,
        name: &std::ffi::OsStr,
    ) -> Result<NodeHandle, crate::shredder::ShredError>;
    fn identity(&self, node: &NodeHandle) -> Result<NodeIdentity, crate::shredder::ShredError>;
    fn open_regular_for_shred(
        &self,
        node: &NodeHandle,
    ) -> Result<std::fs::File, crate::shredder::ShredError>;
    fn rename_noreplace(
        &self,
        parent: &DirHandle,
        node: &NodeHandle,
        new_name: &std::ffi::OsStr,
    ) -> Result<RenamedNode, crate::shredder::ShredError>;
    fn unlink_leaf(
        &self,
        parent: &DirHandle,
        node: &NodeHandle,
    ) -> Result<(), crate::shredder::ShredError>;
    fn remove_empty_dir(
        &self,
        parent: &DirHandle,
        node: &NodeHandle,
    ) -> Result<(), crate::shredder::ShredError>;
    fn sync_parent(&self, parent: &DirHandle) -> Result<(), crate::shredder::ShredError>;
}

pub(crate) trait OpenFileShredder: Send + Sync {
    fn shred_open_file(
        &self,
        file: std::fs::File,
        identity: NodeIdentity,
        request: &FileShredRequest,
    ) -> Result<FileShredResult, crate::shredder::ShredError>;
}

#[cfg(test)]
mod tests;
