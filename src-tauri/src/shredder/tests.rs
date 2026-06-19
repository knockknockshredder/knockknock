// src-tauri/src/shredder/tests.rs

#[cfg(test)]
mod tests {
    use crate::shredder::algorithms::dod_522022m::Dod522022M;
    use crate::shredder::algorithms::nist_clear::NistClear;
    use crate::shredder::algorithms::random_only::RandomOnly;
    use crate::shredder::progress::NoopProgressReporter;
    use crate::shredder::traits::{ShredAlgorithm, VerificationStrategy};
    use crate::shredder::types::*;
    use crate::shredder::validation::*;
    use crate::shredder::verification::*;
    use crate::shredder::ShredError;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_nist_clear_single_pass_zeros() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 1024]).unwrap();
        temp.flush().unwrap();

        let algo = NistClear;
        let progress = NoopProgressReporter;
        let mut file = temp.reopen().unwrap();

        let result = algo
            .shred(&mut file, 1024, 1, PatternType::Zeros, &progress)
            .unwrap();
        assert!(result.success);
        assert_eq!(result.passes_completed, 1);
        assert_eq!(result.bytes_written, 1024);
    }

    #[test]
    fn test_dod_3_pass() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 1024]).unwrap();
        temp.flush().unwrap();

        let algo = Dod522022M;
        let progress = NoopProgressReporter;
        let mut file = temp.reopen().unwrap();

        let result = algo
            .shred(&mut file, 1024, 3, PatternType::Random, &progress)
            .unwrap();
        assert!(result.success);
        assert_eq!(result.passes_completed, 3);
    }

    #[test]
    fn test_random_only() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 4096]).unwrap();
        temp.flush().unwrap();

        let algo = RandomOnly;
        let progress = NoopProgressReporter;
        let mut file = temp.reopen().unwrap();

        let result = algo
            .shred(&mut file, 4096, 1, PatternType::Random, &progress)
            .unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_sample_verification_passes() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00; 4096]).unwrap();
        temp.flush().unwrap();

        let verifier = SampleVerification::new();
        let mut file = temp.reopen().unwrap();
        let result = verifier
            .verify(&mut file, &PatternType::Zeros, 4096)
            .unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_sample_verification_fails_on_wrong_pattern() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00; 4096]).unwrap();
        temp.flush().unwrap();

        let verifier = SampleVerification::new();
        let mut file = temp.reopen().unwrap();
        let result = verifier
            .verify(&mut file, &PatternType::Ones, 4096)
            .unwrap();
        assert!(!result.passed);
    }

    #[test]
    #[cfg(unix)]
    fn test_validation_rejects_symlinks() {
        let temp = NamedTempFile::new().unwrap();
        let link = temp.path().with_extension("link");
        std::os::unix::fs::symlink(temp.path(), &link).unwrap();
        assert!(matches!(
            validate_path(&link),
            Err(ShredError::SymlinkDetected(_))
        ));
        std::fs::remove_file(link).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn test_validation_rejects_system_paths() {
        let result = validate_path(std::path::Path::new("C:\\Windows\\system32\\cmd.exe"));
        assert!(matches!(result, Err(ShredError::SystemFile(_))));
    }

    #[test]
    fn test_validation_rejects_empty_path() {
        assert!(matches!(
            validate_path(std::path::Path::new("")),
            Err(ShredError::EmptyPath)
        ));
    }
}
