# Changelog

All notable changes to KnockKnock will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.1] — 2026-07-23

### "Drive Detection & Window Polish"

This release adds cross-platform drive type detection (SSD/HDD), removes the confusing default algorithm setting in favor of per-algorithm tooltips, improves window draggability throughout the app, and tightens PIN change UX.

### Added

#### Drive Type Detection
- **Linux drive detection** — `/sys/block` rotational flag check, NVMe detection, fallback to `lsblk -d -o ROTA`.
- **macOS drive detection** — `IOKit` property `Solid State` via `IORegistryEntryCreateCFProperty`.
- **Windows NVMe fix** — Use `FILE_READ_ATTRIBUTES` instead of `GENERIC_READ` for NVMe SSD detection query, preventing access-denied on NVMe drives.
- **Windows legacy HDD/SSD detection** — Drive module's `IOCTL_STORAGE_QUERY_PROPERTY` and rotational rate check, gated by admin elevation.

#### Window Draggability
- **Pre-gate screen drag regions** — Loading, config-error, onboarding, PIN gate, and vault-restore screens now draggable via `data-tauri-drag-region`. These render before `TitleBar`/`AppShell`, so the window was previously undraggable.
- **Dialog backdrop drag passthrough** — Dialog backdrop (`fixed inset-0`) now passes clicks through to the gate screen's drag region, so dialog-closed users can move the window.

#### Onboarding
- **First-launch onboarding flow** — New first-run experience guides users through initial PIN setup before they can use the app.

### Changed

- **Per-algorithm tooltips** — Removed the global default algorithm setting. Each algorithm now has a (?) tooltip with detailed description, giving users context *when they choose* rather than hiding it in settings.
- **Sidebar width to percentage-based** — Sidebar widths reworked from fixed pixel values to percentage-based with dampened resize, better adapting to window size.
- **PIN disable confirmation** — Disabling PIN now shows a confirmation dialog. "Change PIN" button hidden when PIN is disabled. Labels clarified.

### Fixed
- **Dialog backdrop blocking drag** — `data-tauri-drag-region` on backdrop allows clicks through (dialog closes on outside click).
- **Settings scrollbar visibility** — Added native scrollbar styling (`::-webkit-scrollbar`, `scrollbar-width`) to match the dark theme.

### Refactored
- **Dead code removal** — Unused `acknowledgedBrowsers` state, `useCallback` import, map index parameters, and vestigial React imports removed as exposed by `noUnusedLocals`.

### CI/CD
- **macOS x64 runner retired** — `macos-13` (Intel) build matrix entry removed; macOS ARM64 (`macos-14`) remains for `aarch64-apple-darwin`.
- **Versioned Windows artifacts** — `.exe` output renamed to `KnockKnock_${VERSION}_x64.exe` for distinguishable release downloads.

## [0.4.0] — 2026-07-21

### "Portable App & Shortcut Awareness"

This release converts KnockKnock into a fully portable application with no localStorage dependency, adds Windows shortcut awareness for the shredder, and hardens PIN security with fail-closed gates and atomic config writes.

### Added

#### Portable App Architecture
- **Portable data paths** — Settings, vault, journal, and lockout state now stored alongside the executable (Windows) or in `~/.local/share/KnockKnock` (macOS/Linux). No AppData or localStorage dependency.
- **Rust-backed settings** — `get_settings` / `save_settings` Tauri commands replace localStorage. Settings validated on save and stored as JSON via `serde_json`.
- **macOS dataStoreIdentifier** — WKWebView data namespaced per app install on macOS 14+, preventing Safari data collisions.
- **Windows WebView2 embed** — Portable Windows .exe bundles the WebView2 bootstrapper, removing the runtime dependency.
- **Portable release artifacts** — Windows: raw `.exe` via `--no-bundle`. macOS: `.dmg` via `--bundles dmg`. Linux: AppImage via `--bundles appimage`.

#### Shortcut Awareness
- **`lnks` crate integration** — Windows shortcut (.lnk) resolution via the `lnks` crate.
- **`FileMetadata` type** — New struct with `is_shortcut` and `shortcut_target` fields for shortcut-aware file handling.
- **`ShortcutDetected` error** — New error variant for Windows shortcuts pointing to unexpected targets.
- **Shortcut-aware UI** — `FileDropZone` and `FileListItem` now display shortcut indicators and resolve targets.

#### PIN Security Hardening
- **Fail-closed PIN gates** — PIN verification dialogs cannot be bypassed by closing; app remains locked until valid PIN entered.
- **Atomic config writes** — Settings and lockout state written to temp file then renamed, preventing corruption on crash.
- **Vault auto-save reliability** — Vault persisted on every file list change with 500ms debounce.

### Changed
- **Dependencies upgraded** — `native-dialog` 0.7 → 0.9 (deadlock fix, modern objc2 bindings).
- **Version bumped** — `0.3.0` → `0.4.0` in `package.json`, `Cargo.toml`, and about dialog.

### Fixed
- **PIN arg names** — `PinVerify` and `PinSetup` now pass `snake_case` parameter names matching Rust backend.
- **macOS startup guidance** — `startup_fatal` now includes writable-folder tip for macOS users.
- **PIN lock bypass** — Closing PIN dialog no longer reveals the full app.
- **Vault decryption after PIN change** — Vault re-encrypted when user changes PIN.
- **Double-shred from React StrictMode** — Dialog re-fire no longer triggers duplicate shred operations.
- **File handle read access** — Verification step now opens files with read permissions.

## [0.3.0] — 2026-07-17

### "PIN-Protected Emergency Preparation"

This release introduces PIN-based security, encrypted session persistence, drive-aware file grouping, and ergonomic improvements throughout the UI. Users can now prepare files for emergency shredding across app restarts with military-grade encryption.

### Added

#### Security & Authentication
- **PIN protection system** — 6-32 digit PIN with bcrypt-hashed storage. PIN required on app open, before shred operations, and before cancellation.
- **Brute-force lockout** — 3 failed attempts trigger a 5-minute lockout. Lockout state is persisted to disk (`lockout.json`), surviving app restarts.
- **PIN setup UI** — Full digits-only PIN entry with confirmation field, validation (6-32 digits, numeric-only), and disclaimer about safe storage.
- **PIN verification dialogs** — Three-purpose dialog (app open / shred / cancel) with live lockout countdown and "Forgot PIN?" reset flow.

#### Session Persistence
- **Encrypted vault** — File/folder paths are encrypted to disk with AES-256-GCM and PBKDF2-HMAC-SHA256 (1,000,000 iterations). Vault format includes a version field for future migrations.
- **Auto-save on file changes** — Vault is automatically saved (500ms debounce) whenever the shred list changes, so user preparations survive app restarts.
- **Auto-load on startup** — After PIN verification, vault is decrypted and previous file selection is restored. Invalid/missing paths are silently dropped.

#### Drive Type Grouping
- **Drive-aware file grouping** — Files in the sidebar are grouped by drive letter (Windows) or mount point (macOS/Linux). Each group header shows drive type: SSD, HDD, Network, USB HDD, or Unknown.
- **Collapsible groups** — Each drive group can be collapsed/expanded independently.
- **Platform-aware key extraction** — `getDriveKey()` handles Windows drive letters, UNC paths, and Unix mount paths.

#### Folder Selection
- **Add Files / Add Folder buttons** — Separate buttons for selecting files or entire directories. Backend already handled recursive directory traversal.
- **Updated drop zone text** — "Drop files or folders here" with two explicit action buttons.

### Improved

#### Confirmation & Progress
- **Immediate dialog close** — Confirmation dialog closes as soon as the user clicks DESTROY, preventing the stale "Nothing to destroy" message.
- **Real-time progress display** — Progress bar and file count (N/M files) shown under the shred button during operation.
- **Cancel button** — During shredding, button turns into amber "Cancel Shredding" (requires PIN).

#### Error Handling
- **Permission denied distinction** — `ShredError` now distinguishes `FileLocked` (held by another process) from `PermissionDenied` (ACL/privilege), offering the right UI remedy for each.
- **Elevation prompt** — New `ElevationPrompt` dialog guides the user to restart as administrator when insufficient privileges are detected.

#### File Path Privacy
- **Log filepath masking** — User-configurable obfuscation in Settings: Full Paths / Numbered (File_1) / Partial Mask (C:\***.txt). Applied to all progress events emitted during shredding.
- **Log Path Display setting** — Configured in Settings section, persisted in localStorage.

### Changed
- **Log path selector moved to Settings** — The Log Path Display selector appears only in Settings section (not on the main shred page per user feedback).
- **Version bumped** — `0.2.0` → `0.3.0` in `package.json`, `Cargo.toml`, and about dialog.

### Removed
- **All dead code** — ~356 lines of unused code removed across the codebase: unused enum variants (`Hdd`, `UsbSsd`, `HardLinksDetected`, `Verifying`, `Renaming`, etc.), unused functions (`detect_data_types`, `init_logging`, `byte_value`), unused trait methods (`write_data`, `schedule_delete_on_reboot`), unused struct/impl (`NoopProgressReporter`), unused imports, and stale TODO comments.

### Security
- **PIN brute-force protection persisted** — Lockout state stored to disk prevents bypass via app restart.
- **AES-256-GCM vault** — Paths encrypted with AEAD authentication (tampering detected). Key derived via PBKDF2-SHA256 at 1M iterations.
- **`reset_app` requires PIN** — Unauthorized wipe of settings and vault prevented.
- **Digits-only PIN enforcement** — Three-layer validation: HTML `inputMode="numeric"`, JS `replace(/\D/g, "")`, and Rust `is_ascii_digit()`.

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
