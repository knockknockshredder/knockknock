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
    use chacha20::cipher::{StreamCipher, StreamCipherSeek};
    use std::io::{Read, Seek, SeekFrom, Write};
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
            .shred(&mut file, 1024, 1, PatternType::Zeros, &progress, None)
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
            .shred(&mut file, 1024, 3, PatternType::Random, &progress, None)
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
            .shred(&mut file, 4096, 1, PatternType::Random, &progress, None)
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
            .verify(&mut file, &PatternType::Zeros, 4096, None)
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
            .verify(&mut file, &PatternType::Ones, 4096, None)
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

    // --- ChaCha20 seeded PRNG verification tests ---

    #[test]
    fn test_prng_seed_generate_is_unique() {
        let s1 = PrngSeed::generate().unwrap();
        let s2 = PrngSeed::generate().unwrap();
        assert_ne!(s1.key, s2.key);
        assert_ne!(s1.nonce, s2.nonce);
    }

    #[test]
    fn test_prng_seed_cipher_is_deterministic() {
        let seed = PrngSeed::generate().unwrap();
        let mut buf_a = vec![0u8; 1024];
        let mut buf_b = vec![0u8; 1024];

        let mut c1 = seed.cipher();
        c1.apply_keystream(&mut buf_a);
        let mut c2 = seed.cipher();
        c2.apply_keystream(&mut buf_b);
        assert_eq!(buf_a, buf_b);
    }

    #[test]
    fn test_prng_seed_cipher_try_seek_is_o1() {
        // Verifies try_seek jumps to arbitrary offset and produces the same
        // bytes as running the keystream forward from offset 0.
        let seed = PrngSeed::generate().unwrap();

        let mut full = vec![0u8; 100_000];
        let mut full_cipher = seed.cipher();
        full_cipher.apply_keystream(&mut full);

        let mut jumped = vec![0u8; 4096];
        let mut jumped_cipher = seed.cipher();
        jumped_cipher.try_seek(50_000).unwrap();
        jumped_cipher.apply_keystream(&mut jumped);

        assert_eq!(&full[50_000..50_000 + 4096], &jumped[..]);
    }

    #[test]
    fn test_nist_clear_random_with_seed_verifies() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 8192]).unwrap();
        temp.flush().unwrap();

        let algo = NistClear;
        let progress = NoopProgressReporter;
        let mut file = temp.reopen().unwrap();

        let seed = PrngSeed::generate().unwrap();
        let result = algo
            .shred(
                &mut file,
                8192,
                1,
                PatternType::Random,
                &progress,
                Some(&seed),
            )
            .unwrap();
        assert!(result.success);

        // Verify the written bytes match ChaCha20(seed).
        let verifier = SampleVerification::new();
        let result = verifier
            .verify(&mut file, &PatternType::Random, 8192, Some(&seed))
            .unwrap();
        assert!(result.passed, "expected seeded Random to verify");
    }

    #[test]
    fn test_random_only_with_seed_writes_deterministic_data() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xAA; 16384]).unwrap();
        temp.flush().unwrap();

        let algo = RandomOnly;
        let progress = NoopProgressReporter;
        let mut file = temp.reopen().unwrap();

        let seed = PrngSeed::generate().unwrap();
        algo.shred(
            &mut file,
            16384,
            1,
            PatternType::Random,
            &progress,
            Some(&seed),
        )
        .unwrap();

        // Regenerate expected data from seed and confirm match.
        let mut expected = vec![0u8; 16384];
        let mut cipher = seed.cipher();
        cipher.apply_keystream(&mut expected);

        let mut actual = vec![0u8; 16384];
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_exact(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_full_verification_random_with_seed_detects_corruption() {
        // Write ChaCha20(seed) data, corrupt one byte, then verify. With seed,
        // FullVerification must catch the corruption (heuristic-only would miss it).
        let mut temp = NamedTempFile::new().unwrap();
        let seed = PrngSeed::generate().unwrap();
        let mut expected = vec![0u8; 8192];
        let mut cipher = seed.cipher();
        cipher.apply_keystream(&mut expected);
        // Inject a corruption: flip one byte to 0x00. With 8192 bytes of
        // PRNG output, this is virtually guaranteed not to be the natural value.
        expected[4096] = 0x00;
        temp.write_all(&expected).unwrap();
        temp.flush().unwrap();

        let verifier = FullVerification;
        let mut file = temp.reopen().unwrap();
        let result = verifier
            .verify(&mut file, &PatternType::Random, 8192, Some(&seed))
            .unwrap();
        assert!(
            !result.passed,
            "expected seeded verifier to detect corruption"
        );
    }

    #[test]
    fn test_sample_verification_random_no_seed_uses_heuristic() {
        // Without a seed, Random verification uses the all-zeros/all-ones
        // heuristic. All-FF data should fail (matches the old behavior).
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0xFF; 4096]).unwrap();
        temp.flush().unwrap();

        let verifier = SampleVerification::new();
        let mut file = temp.reopen().unwrap();
        let result = verifier
            .verify(&mut file, &PatternType::Random, 4096, None)
            .unwrap();
        assert!(!result.passed);
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
