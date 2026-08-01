// src-tauri/src/vault/storage.rs
//
// File-based persistence for the encrypted vault. Each save generates a new
// salt and nonce, so the file is safe to keep on disk without the PIN.
//
// File layout: <KnockKnock-data>/vault.json

use super::crypto::{self, EncryptedData};
use crate::pin::config::set_owner_only;
use crate::shredder::root_execution::types::{
    TargetKind, VaultError, VaultSchemaSource, VaultTarget,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
struct VaultFile {
    /// Format version. Mirrors [`crypto::VAULT_VERSION`] at the time of
    /// encryption. Stored so we can reject unsupported vaults explicitly.
    version: u32,
    salt: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultPayloadV2 {
    pub schema_version: u32,
    pub targets: Vec<VaultTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultLoadDto {
    pub source_schema: VaultSchemaSource,
    pub migration_required: bool,
    pub targets: Vec<VaultTarget>,
}

pub struct VaultStore {
    vault_path: PathBuf,
}

impl VaultStore {
    pub fn production() -> Result<Self, VaultError> {
        let data_dir = crate::paths::portable_data_dir().map_err(|message| VaultError::Io {
            action: "resolve vault path",
            source: std::io::Error::other(message),
        })?;
        Ok(Self {
            vault_path: data_dir.join("vault.json"),
        })
    }

    pub fn at(vault_path: PathBuf) -> Self {
        Self { vault_path }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.vault_path
    }

    pub fn load(&self, pin: &str) -> Result<VaultLoadDto, VaultError> {
        if !self.vault_path.exists() {
            return Ok(VaultLoadDto {
                source_schema: VaultSchemaSource::V2,
                migration_required: false,
                targets: Vec::new(),
            });
        }

        let json = std::fs::read(&self.vault_path).map_err(|source| VaultError::Io {
            action: "read vault",
            source,
        })?;
        let vault_file: VaultFile = serde_json::from_slice(&json)
            .map_err(|source| VaultError::Decode(source.to_string()))?;

        if vault_file.version != crypto::VAULT_VERSION {
            return Err(VaultError::UnsupportedSchema(vault_file.version));
        }

        let encrypted = EncryptedData {
            version: vault_file.version,
            salt: vault_file.salt,
            nonce: vault_file.nonce,
            ciphertext: vault_file.ciphertext,
        };
        let plaintext = crypto::decrypt(&encrypted, pin).map_err(VaultError::Crypto)?;
        decode_payload(&plaintext)
    }

    pub fn save_v2(&self, targets: &[VaultTarget], pin: &str) -> Result<(), VaultError> {
        let io = ProductionVaultIo;
        self.save_v2_with_io(targets, pin, &io)
    }

    pub fn rekey(&self, old_pin: &str, new_pin: &str) -> Result<(), VaultError> {
        if !self.vault_path.exists() {
            return Ok(());
        }

        let loaded = match self.load(old_pin) {
            Ok(payload) => payload,
            Err(_) => return Ok(()),
        };

        self.save_v2(&loaded.targets, new_pin)
    }

    pub(crate) fn save_v2_with_io(
        &self,
        targets: &[VaultTarget],
        pin: &str,
        io: &dyn VaultIo,
    ) -> Result<(), VaultError> {
        let payload = VaultPayloadV2 {
            schema_version: 2,
            targets: targets.to_vec(),
        };
        let plaintext = serde_json::to_vec(&payload)
            .map_err(|source| VaultError::Decode(source.to_string()))?;
        let encrypted = crypto::encrypt(&plaintext, pin).map_err(VaultError::Crypto)?;
        let vault_file = VaultFile {
            version: encrypted.version,
            salt: encrypted.salt,
            nonce: encrypted.nonce,
            ciphertext: encrypted.ciphertext,
        };
        let bytes = serde_json::to_vec(&vault_file)
            .map_err(|source| VaultError::Decode(source.to_string()))?;

        let (temporary_path, mut temporary_file) = create_unique_temp(io, &self.vault_path)?;
        if let Err(error) = io.write_temp(&mut temporary_file, &temporary_path, &bytes) {
            drop(temporary_file);
            return Err(save_failure_with_cleanup(io, &temporary_path, error));
        }
        if let Err(error) = io.sync_temp(&temporary_file, &temporary_path) {
            drop(temporary_file);
            return Err(save_failure_with_cleanup(io, &temporary_path, error));
        }
        drop(temporary_file);

        let replacement = if self.vault_path.exists() {
            io.replace_existing(&temporary_path, &self.vault_path)
        } else {
            io.replace(&temporary_path, &self.vault_path)
        };
        if let Err(error) = replacement {
            return Err(save_failure_with_cleanup(io, &temporary_path, error));
        }

        if let Err(error) = io.sync_parent(&self.vault_path) {
            return Err(error);
        }

        Ok(())
    }
}

fn save_failure_with_cleanup(
    io: &dyn VaultIo,
    temporary_path: &Path,
    primary: VaultError,
) -> VaultError {
    match io.cleanup_temp(temporary_path) {
        Ok(()) => primary,
        Err(cleanup) => VaultError::Io {
            action: "save V2 vault and clean up temporary file",
            source: std::io::Error::other(format!(
                "primary error: {primary}; cleanup error: {cleanup}"
            )),
        },
    }
}

fn create_unique_temp(io: &dyn VaultIo, vault_path: &Path) -> Result<(PathBuf, File), VaultError> {
    let parent = vault_path.parent().unwrap_or_else(|| Path::new("."));
    let name = vault_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("vault.json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let process_id = std::process::id();

    for attempt in 0..128u32 {
        let temporary_path = parent.join(format!(
            ".{name}.{}.{}.tmp",
            process_id,
            timestamp + u128::from(attempt)
        ));
        match io.create_temp(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if is_already_exists(&error) => continue,
            Err(error) => return Err(error),
        }
    }

    Err(VaultError::Io {
        action: "create unique temporary vault",
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "temporary vault name collision limit reached",
        ),
    })
}

fn is_already_exists(error: &VaultError) -> bool {
    matches!(
        error,
        VaultError::Io { source, .. } if source.kind() == std::io::ErrorKind::AlreadyExists
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultIoFailure {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    Replace,
    ReplaceExisting,
    SyncParent,
    CleanupTemp,
}

pub(crate) trait VaultIo {
    fn create_temp(&self, path: &std::path::Path) -> Result<File, VaultError>;
    fn write_temp(
        &self,
        file: &mut File,
        path: &std::path::Path,
        bytes: &[u8],
    ) -> Result<(), VaultError>;
    fn sync_temp(&self, file: &File, path: &std::path::Path) -> Result<(), VaultError>;
    fn replace(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError>;
    fn replace_existing(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError>;
    fn sync_parent(&self, vault_path: &std::path::Path) -> Result<(), VaultError>;
    fn cleanup_temp(&self, temporary_path: &std::path::Path) -> Result<(), VaultError>;
}

pub(crate) struct ProductionVaultIo;

impl VaultIo for ProductionVaultIo {
    fn create_temp(&self, path: &std::path::Path) -> Result<File, VaultError> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| VaultError::Io {
                action: "create temporary vault",
                source,
            })
    }

    fn write_temp(
        &self,
        file: &mut File,
        path: &std::path::Path,
        bytes: &[u8],
    ) -> Result<(), VaultError> {
        file.write_all(bytes).map_err(|source| VaultError::Io {
            action: "write temporary vault",
            source,
        })?;
        set_owner_only(path).map_err(|message| VaultError::Io {
            action: "protect temporary vault",
            source: std::io::Error::other(message),
        })
    }

    fn sync_temp(&self, file: &File, path: &std::path::Path) -> Result<(), VaultError> {
        file.sync_all().map_err(|source| VaultError::Sync {
            path: path.to_path_buf(),
            source,
        })
    }

    fn replace(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError> {
        std::fs::rename(temporary_path, vault_path).map_err(|source| VaultError::Replace { source })
    }

    fn replace_existing(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError> {
        #[cfg(unix)]
        {
            std::fs::rename(temporary_path, vault_path)
                .map_err(|source| VaultError::Replace { source })
        }

        #[cfg(windows)]
        {
            replace_file_windows(temporary_path, vault_path)
        }
    }

    fn sync_parent(&self, vault_path: &std::path::Path) -> Result<(), VaultError> {
        let Some(parent) = vault_path.parent() else {
            return Ok(());
        };

        #[cfg(unix)]
        {
            let parent_file = File::open(parent).map_err(|source| VaultError::Sync {
                path: parent.to_path_buf(),
                source,
            })?;
            parent_file.sync_all().map_err(|source| VaultError::Sync {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        #[cfg(not(unix))]
        let _ = parent;

        Ok(())
    }

    fn cleanup_temp(&self, temporary_path: &std::path::Path) -> Result<(), VaultError> {
        match std::fs::remove_file(temporary_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(VaultError::Io {
                action: "cleanup temporary vault",
                source,
            }),
        }
    }
}

#[cfg(test)]
pub(crate) struct FaultInjectingVaultIo {
    failures: Vec<VaultIoFailure>,
}

#[cfg(test)]
impl FaultInjectingVaultIo {
    pub(crate) fn failing_at(failure: VaultIoFailure) -> Self {
        Self {
            failures: vec![failure],
        }
    }

    pub(crate) fn failing_at_operations(failures: &[VaultIoFailure]) -> Self {
        Self {
            failures: failures.to_vec(),
        }
    }

    fn fail_if(&self, operation: VaultIoFailure) -> Result<(), VaultError> {
        if self.failures.contains(&operation) {
            return Err(VaultError::Io {
                action: "fault-injected vault operation",
                source: std::io::Error::other(format!("fault at {operation:?}")),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
impl VaultIo for FaultInjectingVaultIo {
    fn create_temp(&self, path: &std::path::Path) -> Result<File, VaultError> {
        self.fail_if(VaultIoFailure::CreateTemp)?;
        ProductionVaultIo.create_temp(path)
    }

    fn write_temp(
        &self,
        file: &mut File,
        path: &std::path::Path,
        bytes: &[u8],
    ) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::WriteTemp)?;
        ProductionVaultIo.write_temp(file, path, bytes)
    }

    fn sync_temp(&self, file: &File, path: &std::path::Path) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::SyncTemp)?;
        ProductionVaultIo.sync_temp(file, path)
    }

    fn replace(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::Replace)?;
        ProductionVaultIo.replace(temporary_path, vault_path)
    }

    fn replace_existing(
        &self,
        temporary_path: &std::path::Path,
        vault_path: &std::path::Path,
    ) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::ReplaceExisting)?;
        ProductionVaultIo.replace_existing(temporary_path, vault_path)
    }

    fn sync_parent(&self, vault_path: &std::path::Path) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::SyncParent)?;
        ProductionVaultIo.sync_parent(vault_path)
    }

    fn cleanup_temp(&self, temporary_path: &std::path::Path) -> Result<(), VaultError> {
        self.fail_if(VaultIoFailure::CleanupTemp)?;
        ProductionVaultIo.cleanup_temp(temporary_path)
    }
}

#[cfg(windows)]
fn replace_file_windows(
    temporary_path: &std::path::Path,
    vault_path: &std::path::Path,
) -> Result<(), VaultError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let vault_path_wide: Vec<u16> = vault_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let temporary_path_wide: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: Both UTF-16 buffers are NUL-terminated, remain alive and
    // immutable for the duration of the call, and the optional backup,
    // exclude, and reserved pointers are explicitly null as permitted by
    // ReplaceFileW.
    let result = unsafe {
        ReplaceFileW(
            vault_path_wide.as_ptr(),
            temporary_path_wide.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(VaultError::Replace {
            source: std::io::Error::last_os_error(),
        })
    } else {
        Ok(())
    }
}

fn decode_payload(plaintext: &[u8]) -> Result<VaultLoadDto, VaultError> {
    let value: serde_json::Value = serde_json::from_slice(plaintext)
        .map_err(|source| VaultError::Decode(source.to_string()))?;

    match value {
        serde_json::Value::Array(_) => {
            let paths: Vec<String> = serde_json::from_value(value)
                .map_err(|source| VaultError::Decode(source.to_string()))?;
            Ok(VaultLoadDto {
                source_schema: VaultSchemaSource::V1,
                migration_required: true,
                targets: paths
                    .into_iter()
                    .map(|path| VaultTarget {
                        path,
                        kind: TargetKind::UnknownLegacy,
                    })
                    .collect(),
            })
        }
        serde_json::Value::Object(ref object) => {
            let schema_version = object
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| VaultError::Decode("Missing schema_version".to_string()))?;
            let schema_version = u32::try_from(schema_version)
                .map_err(|_| VaultError::UnsupportedSchema(u32::MAX))?;
            if schema_version != 2 {
                return Err(VaultError::UnsupportedSchema(schema_version));
            }

            let payload: VaultPayloadV2 = serde_json::from_value(value)
                .map_err(|source| VaultError::Decode(source.to_string()))?;
            Ok(VaultLoadDto {
                source_schema: VaultSchemaSource::V2,
                migration_required: false,
                targets: payload.targets,
            })
        }
        _ => Err(VaultError::Decode(
            "Vault payload must be a V1 array or V2 object".to_string(),
        )),
    }
}

fn vault_path() -> Result<PathBuf, String> {
    let app_dir = crate::paths::portable_data_dir()?;
    Ok(app_dir.join("vault.json"))
}

/// Delete the on-disk vault if present. No-op if it doesn't exist.
pub fn clear() -> Result<(), String> {
    let path = vault_path()?;

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete vault: {}", e))?;
    }

    Ok(())
}

/// True if a vault file currently exists on disk.
pub fn exists() -> bool {
    vault_path().map(|p| p.exists()).unwrap_or(false)
}

/// Re-encrypt the on-disk vault from `old_pin` to `new_pin`.
///
/// Best-effort: if no vault exists or decryption fails, the PIN change
/// still succeeds (the user can start a fresh session). Only an I/O error
/// during the re-save is surfaced as an `Err`.
pub fn rekey(old_pin: &str, new_pin: &str) -> Result<(), String> {
    let store = VaultStore::production().map_err(String::from)?;
    store.rekey(old_pin, new_pin).map_err(String::from)
}
