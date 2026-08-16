// src-tauri/src/shredder/platform/linux.rs

use super::map_drive_type;
use crate::shredder::errors::ShredError;
use crate::shredder::traits::PlatformIo;
use crate::shredder::types::MediaType;
use std::path::Path;

pub struct LinuxIo;

impl LinuxIo {
    pub fn new() -> Self {
        Self
    }
}

pub(crate) fn ensure_local_volume(path: &Path) -> Result<(), ShredError> {
    let filesystem = rustix::fs::statfs(path).map_err(|error| {
        ShredError::from_io_error(path.to_path_buf(), std::io::Error::from(error))
    })?;
    let filesystem_type = filesystem.f_type as u64;

    // Linux filesystem magic values for network filesystems. `statfs` is used
    // instead of `/proc/mounts` so local-volume validation does not depend on a
    // process-global text file or on path-prefix parsing.
    if is_denied_filesystem_magic(filesystem_type) {
        return Err(ShredError::NetworkDrive(path.to_path_buf()));
    }

    Ok(())
}

fn is_denied_filesystem_magic(filesystem_type: u64) -> bool {
    const DENIED_FILESYSTEM_MAGICS: &[u64] = &[
        0x5346_414F, // AFS
        0x7375_7245, // Coda
        0xFF53_4D42, // CIFS
        0xFE53_4D42, // SMB2
        0x0000_564C, // NCP
        0x6573_5546, // FUSE
        0x0000_6969, // NFS
        0x0000_517B, // SMB
        0x00C3_6400, // Ceph
        0x0102_1997, // 9p
    ];

    DENIED_FILESYSTEM_MAGICS.contains(&filesystem_type)
}

impl PlatformIo for LinuxIo {
    fn detect_media_type(&self, path: &Path) -> Result<MediaType, ShredError> {
        // Delegate to the drive module for centralized detection.
        match crate::drive::detect_drive_info(path) {
            Ok(info) => Ok(map_drive_type(info.drive_type)),
            Err(_) => Ok(MediaType::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::map_drive_type;
    use super::{ensure_local_volume, is_denied_filesystem_magic};
    use crate::drive::DriveType;
    use crate::shredder::types::MediaType;

    #[test]
    fn maps_usb_and_conservative_drive_types() {
        let cases = [
            (DriveType::Hdd, MediaType::Hdd),
            (DriveType::UsbHdd, MediaType::Hdd),
            (DriveType::Ssd, MediaType::Ssd),
            (DriveType::UsbSsd, MediaType::Ssd),
            (DriveType::Unknown, MediaType::Unknown),
            (DriveType::Network, MediaType::Unknown),
        ];

        for (drive_type, expected) in cases {
            assert_eq!(map_drive_type(drive_type), expected);
        }
    }

    #[test]
    fn denies_known_network_fuse_ceph_and_9p_filesystem_magics() {
        for filesystem_type in [
            0x6573_5546, // FUSE
            0x0000_6969, // NFS
            0x0000_517B, // SMB
            0xFF53_4D42, // CIFS
            0x00C3_6400, // Ceph
            0x0102_1997, // 9p
        ] {
            assert!(
                is_denied_filesystem_magic(filesystem_type),
                "filesystem magic {filesystem_type:#x} must be denied"
            );
        }
    }

    #[test]
    fn allows_a_local_filesystem_magic_and_fixture_root() {
        assert!(!is_denied_filesystem_magic(0x0000_EF53)); // ext4

        let fixture = tempfile::tempdir().expect("temporary fixture");
        ensure_local_volume(fixture.path()).expect("temporary fixture is local");
    }
}
