// src-tauri/src/shredder/tests.rs

#[cfg(test)]
mod tests {
    use crate::shredder::validation::*;
    use crate::shredder::ShredError;
    use tempfile::NamedTempFile;

    #[test]
    #[cfg(unix)]
    fn test_validation_rejects_symlinks() {
        let temp = NamedTempFile::new().unwrap();
        let link = temp.path().with_extension("link");
        std::os::unix::fs::symlink(temp.path(), &link).unwrap();

        // classify_path must recognize a symlink as a Shortcut.
        match classify_path(&link).unwrap() {
            PathClassification::Shortcut { .. } => {}
            PathClassification::Normal => panic!("symlink classified as Normal"),
        }

        // validate_path with allow_shortcut=false must reject with ShortcutDetected.
        assert!(matches!(
            validate_path(&link, false),
            Err(ShredError::ShortcutDetected { .. })
        ));

        // validate_path with allow_shortcut=true must accept (caller opted in).
        assert!(validate_path(&link, true).is_ok());

        std::fs::remove_file(link).unwrap();
    }

    #[test]
    fn test_classify_path_normal_file() {
        let temp = NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), b"hello world").unwrap();

        match classify_path(temp.path()).unwrap() {
            PathClassification::Normal => {}
            PathClassification::Shortcut { target } => {
                panic!("normal file classified as Shortcut: {:?}", target)
            }
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_classify_path_fake_lnk_falls_back_to_normal() {
        // Create a file with a `.lnk` extension containing garbage. The `lnks`
        // crate should fail to parse it; classify_path must fall back to
        // Normal (with a warning logged) so the user's intent — shred this
        // file — is preserved.
        let dir = std::env::temp_dir();
        let fake_lnk = dir.join(format!(
            "knockknock_fake_lnk_{}_{}.lnk",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&fake_lnk, b"definitely not a real .lnk file").unwrap();

        let result = classify_path(&fake_lnk).unwrap();
        let _ = std::fs::remove_file(&fake_lnk);

        match result {
            PathClassification::Normal => {}
            PathClassification::Shortcut { target } => {
                panic!(
                    "fake .lnk should fall back to Normal, got Shortcut target={:?}",
                    target
                )
            }
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_validation_rejects_system_paths() {
        let result = validate_path(
            std::path::Path::new("C:\\Windows\\system32\\cmd.exe"),
            false,
        );
        assert!(matches!(result, Err(ShredError::SystemFile(_))));
    }

    #[test]
    fn test_validation_rejects_empty_path() {
        assert!(matches!(
            validate_path(std::path::Path::new(""), false),
            Err(ShredError::EmptyPath)
        ));
    }

    // --- Cancellation token tests ---

    #[test]
    fn test_cancellation_token_basic() {
        let token = crate::shredder::cancel::CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancellation_global_reset() {
        crate::shredder::cancel::reset_global();
        assert!(!crate::shredder::cancel::is_cancelled_global());
        crate::shredder::cancel::cancel_global();
        assert!(crate::shredder::cancel::is_cancelled_global());
        crate::shredder::cancel::reset_global();
        assert!(!crate::shredder::cancel::is_cancelled_global());
    }
}
