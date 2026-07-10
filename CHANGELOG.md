# Changelog

All notable changes to KnockKnock will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2025-07-10

### "Unstoppable Shredding"

This release makes the shredding pipeline robust against locked files, permission errors, power loss, and crashes. Every critical bug from the codebase audit is fixed, missing platform features are implemented, and the app now recovers from interruptions.

### Added

#### Verification & Correctness
- **ChaCha20 seeded PRNG verification** — Random pattern verification now uses a deterministic seeded PRNG (`chacha20` crate) with `StreamCipherSeek::try_seek()` for O(1) seeking. Previously, verification only checked for all-zeros/all-ones blocks (a false-negative machine that passed unshredded files).
- **Per-chunk random buffer regeneration** — Random data buffers are now regenerated every 4 MB during overwrite passes. Previously, the same 256 KB random pattern was repeated across the entire file.
- **1 MB write buffer** — Increased from 256 KB to 1 MB for ~4x throughput improvement on NVMe drives.
- **DoD verification pattern mismatch fix** — Fixed-sequence algorithms (DoD 5220.22-M) now verify against the actual final pass pattern, not the user-selected pattern.
- **`final_pattern()` trait method** — `ShredAlgorithm` trait now reports which pattern the final pass uses, enabling correct verification for fixed-sequence algorithms.

#### Crash Recovery & Resilience
- **Orphan tracking journal** — Sidecar journal (`.knockknock-journal.json`) tracks in-progress shred operations. If a crash occurs mid-shred, orphaned renamed files are cleaned up on next launch via the `cleanup_orphans` Tauri command.
- **Journal atomicity** — Journal writes use temp-file-then-rename to prevent corruption on crash.
- **Journal entry TTL** — Orphan entries older than 7 days are auto-expired on cleanup.
- **Cancellation support** — New `cancel_shred` Tauri command allows cancelling in-progress shred operations. Uses `AtomicBool` for lock-free per-chunk cancellation checks in the write loop.
- **Cancellation-safe cleanup** — When cancellation triggers during a fixed-sequence algorithm, the file is still renamed, truncated, and deleted. Partially-shredded files are never left under their original name.

#### Platform Completeness (Windows)
- **Hard link detection** — `GetFileInformationByHandle` API for accurate hard link counts. Previously hardcoded to 1, silently missing multi-hard-link data leakage.
- **`schedule_delete_on_reboot`** — `MoveFileExW` with `MOVEFILE_DELAY_UNTIL_REBOOT` for locked files that can't be shredded immediately.
- **`find_locking_processes`** — Exclusive open test to detect if a file is locked by another process.
- **`FILE_FLAG_WRITE_THROUGH`** — Bypasses OS write cache for guaranteed disk persistence.
- **Mapped network drive detection** — `GetDriveTypeW` detects `DRIVE_REMOTE` mapped drives (e.g., `Z:\` → `\\server\share`).
- **Windows reserved name filtering** — `generate_random_name` now filters out CON, PRN, AUX, NUL, COM1-9, LPT1-9.
- **Rename collision retry** — 100-attempt loop for random name generation (matching macOS/Linux behavior).

#### Platform Completeness (macOS)
- **`F_FULLFSYNC` durability** — Replaced `fsync()` with `fcntl(F_FULLFSYNC)` for power-loss-safe writes. Apple's own docs explicitly call out `fsync` as insufficient.
- **`F_NOCACHE` error handling** — Logs warning when `fcntl(F_NOCACHE)` fails instead of silently ignoring.
- **`find_locking_processes`** — `lsof` integration to identify processes holding file locks.
- **`detect_media_type`** — `diskutil info -plist` parsing for SSD detection.

#### Platform Completeness (Linux)
- **Network drive detection** — Parses `/proc/mounts` for NFS, CIFS, SSHFS, and other network filesystem types.
- **`detect_media_type`** — Reads `/sys/block/*/queue/rotational` for SSD vs HDD detection.
- **`find_locking_processes`** — `lsof` integration to identify processes holding file locks.
- **SSD TRIM** — `fstrim` integration on the mount point after shredding. Runs as best-effort (non-fatal if unavailable).

#### Error Handling & IPC
- **`ShredError` derives `Serialize`** — Error types now cross Tauri IPC without losing information.
- **`ShredErrorDto`** — IPC-safe error DTO with `error_type`, `message`, `path`, and `actionable` fields. Every error variant maps to user-friendly guidance (e.g., "Run as administrator", "Close the application using this file").
- **`ShredErrorDto` wired into batch reports** — `ShredReport.errors` now carries actionable error messages.

#### Progress & Logging
- **`on_pass_complete` events** — Pass completion events now emit to the frontend (was an empty body).
- **Emit error logging** — All Tauri event emissions use `emit_or_log` helper. Emit failures are logged to stderr instead of silently discarded.
- **Poison-safe mutex locking** — All `Mutex::lock()` calls use `unwrap_or_else(|e| e.into_inner())` to survive thread panics.
- **Structured logging module** — New `logging.rs` with `LogObfuscation` enum: `None` (full paths), `Numbered` (File_1, File_2), or `PartialMask` (first 3 + last 5 chars with `***`).

#### Safety & Validation
- **Directory depth limit** — `validate_paths` caps recursive directory traversal at 50 levels deep.
- **Symlink protection** — `validate_paths` now uses `symlink_metadata` to detect and skip symlinks, matching the backend's `SymlinkDetected` rejection.
- **`symlink_metadata` in hard link check** — `check_hard_links` no longer follows symlinks.
- **`issue_trim` path fix** — TRIM now targets the renamed path (was using the original path after rename).
- **`issue_trim` ordering** — TRIM runs before file deletion so the FTL sees the LBA range while the file still exists.
- **Hard link detection warnings** — Emits `eprintln!` warnings when hard link count detection falls back to assuming 1 link.
- **UNC path network drive detection** — Extended to catch `\\?\UNC\` prefix (case-insensitive).
- **Network drive check before canonicalization** — Prevents Windows `canonicalize()` from injecting `\\?\` prefix that bypasses the check.

#### Frontend
- **`validate_paths` tuple destructuring** — Frontend now correctly handles the new `(Vec<FileMetadata>, Vec<String>)` return type, surfacing validation errors to the user.

### Changed
- **Buffer size** — 256 KB → 1 MB (algorithms/common.rs).
- **`write_pass` signature** — Now accepts `PatternType`, `&mut [u8]` buffer, and `Option<&PrngSeed>`.
- **`VerificationStrategy::verify` signature** — Now accepts `Option<&PrngSeed>`.
- **`ShredAlgorithm::shred` signature** — Now accepts `Option<&PrngSeed>`.
- **`shred_file` signature** — Now accepts `&CancellationToken`.
- **`validate_paths` return type** — `Vec<FileMetadata>` → `(Vec<FileMetadata>, Vec<String>)`.

### Fixed
- **Random data pattern repeating** — Same 256 KB random block was written repeatedly across the entire file.
- **`issue_trim` using wrong path** — Called on original path after rename; original path no longer exists.
- **`on_pass_complete` empty body** — Pass completion events never reached the frontend.
- **Emit errors silently discarded** — `let _ = self.app.emit(...)` dropped all progress, warning, and error events.
- **DoD double-emit** — DoD algorithm emitted `on_pass_complete` internally, then the pipeline emitted it again.
- **Windows rename collision** — No retry loop (macOS/Linux had 100 attempts; Windows had 1).
- **`check_hard_links` using `metadata` instead of `symlink_metadata`** — Followed symlinks when it shouldn't have.
- **Cancellation leaving partially-shredded files** — Fixed-sequence algorithms skipped cleanup on cancellation, leaving damaged files under original names.

### Security
- **No sensitive data in logs** — File paths can be obfuscated in logs. Journal uses path hashes, not actual paths.
- **No sensitive data in journal** — Only obfuscated references stored on disk.

## [0.1.0] — 2025-06-01

### Initial Release
- File shredding with multiple algorithms (NIST 800-88 Clear, DoD 5220.22-M, Random Only)
- Browser profile detection and cleanup (Chrome, Firefox, Edge, Brave, Opera, Vivaldi, Safari, Tor)
- System path protection (Windows, macOS, Linux)
- Progress reporting via Tauri IPC
- Cross-platform support (Windows, macOS, Linux)
