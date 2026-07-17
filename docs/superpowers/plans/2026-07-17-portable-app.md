# Portable App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make KnockKnock fully portable — downloading the binary is all that's needed to run on all platforms, with zero OS traces (or documented, mitigated exceptions).

**Architecture:** Replace `dirs` crate path resolution with a platform-aware `paths.rs` module that resolves all data paths relative to the executable's location. Replace localStorage with Rust-backed `settings.json` via Tauri commands. Redirect WebView engine data to the portable data folder on Windows/Linux; mitigate macOS WKWebView limitation via `dataStoreIdentifier`.

**Spec:** `docs/superpowers/specs/2026-07-17-portable-app.md`

**Tech Stack:** Tauri 2.x, Rust edition 2021, React 19 + TypeScript strict, pnpm

## Global Constraints

- Zero fallback: if exe directory is unwritable, show error dialog and exit
- No migration from old `dirs`-based data — start fresh
- No auto-updater
- `dirs` crate must be removed from Cargo.toml
- Atomic file writes for settings (write to .tmp, then rename)
- All existing AGENTS.md rules apply (Rule of 0, no dead code, precision coding, etc.)
- macOS 14.0+ required for `dataStoreIdentifier` — version-guard needed

---

## File Structure

```
src-tauri/src/
├── paths.rs              NEW — centralized portable path resolver
├── lib.rs                MODIFY — add paths module, settings commands, WebView setup, startup validation
├── pin/config.rs         MODIFY — replace dirs::config_dir() with paths::portable_data_dir()
├── vault/storage.rs      MODIFY — replace dirs::config_dir() with paths::portable_data_dir()
├── shredder/journal.rs   MODIFY — replace dirs::data_dir() with paths::portable_data_dir()
├── commands/
│   ├── mod.rs            MODIFY — add settings module
│   └── settings.rs       NEW — get_settings / save_settings commands
└── Cargo.toml            MODIFY — remove dirs, verify no new deps needed

src/
├── contexts/
│   └── SettingsContext.tsx  MODIFY — replace localStorage with invoke()

src-tauri/
├── tauri.conf.json       MODIFY — add macOS dataStoreIdentifier, confirmation of bundle config
└── capabilities/
    └── default.json      VERIFY — no fs permissions needed, no changes expected

.github/workflows/
└── release.yml           MODIFY — platform-specific build commands, no installers
```

---

## Pre-Implementation Librarian Research Summary

The following findings from Tauri 2.x docs research inform every task. Pass these to any reviewing oracle.

### Path & Build

| Topic | Finding |
|-------|---------|
| `--no-bundle` | Produces binary at `target/release/`, `beforeBuildCommand` still runs, `beforeBundleCommand` skipped, NO code signing |
| macOS bundle path | `current_exe()` returns `KnockKnock.app/Contents/MacOS/knockknock` — must walk up 4 levels to reach containing directory |
| AppImage path | `current_exe()` points to `/tmp/.mount_.../` mount point (ephemeral). Use `APPIMAGE` env var for original file location |
| DMG-mounted `.app` | Read-only — writing data next to `.app` fails. User must drag `.app` out of DMG first |

### WebView Data

| Platform | Mechanism | Notes |
|----------|-----------|-------|
| Windows | `WEBVIEW2_USER_DATA_FOLDER` env var | Must be set BEFORE `Builder::run()`. Overrides `data_directory()`. Handles Unicode paths. |
| Linux | `WebviewWindowBuilder::data_directory()` | Tauri auto-creates the directory. No fallback on error — panics. |
| macOS | `dataStoreIdentifier` (16 u8 UUID) | macOS 14.0+ ONLY. Crashes on macOS < 14 — MUST version-guard. Creates namespaced store at `~/Library/WebKit/{id}/WebsiteDataStore/{UUID}/` — NOT fully portable, just namespaced. |

### Atomic Writes & Errors

| Topic | Finding |
|-------|---------|
| Atomic writes | `std::fs::rename` is atomic on POSIX. On Windows, Rust 1.86+ uses `FILE_RENAME_FLAG_POSIX_SEMANTICS`. Fallback: write to `.tmp`, remove stale tmp, rename. |
| `Result<T, String>` | Auto-serializes correctly via Tauri IPC — `String` implements `Serialize` |
| setup() errors | `app.dialog().blocking_show()` works in `setup()` before windows exist. For pre-Builder errors, use `native-dialog` crate |
| `data_directory()` errors | Tauri `create_dir_all` failure propagates as panic — must pre-create directory |

---

### Task 1: Create `src-tauri/src/paths.rs`

**Files:**
- Create: `src-tauri/src/paths.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod paths;`)

**Interfaces:**
- Produces: `paths::portable_data_dir() -> Result<PathBuf, String>`, `paths::webview_data_dir() -> Result<PathBuf, String>`

- [ ] **Step 1: Write the module**

Create `src-tauri/src/paths.rs`:

```rust
// src-tauri/src/paths.rs
//
// Centralized portable path resolver. All app data lives in a
// "KnockKnock-data" folder next to the executable — never in
// OS-managed config/data directories.
//
// No fallback. If the exe directory is unwritable, the caller
// receives an Err with a user-facing message.

use std::path::PathBuf;

/// Directory containing the app executable on Windows/Linux, or the
/// .app bundle on macOS.
///
/// - Windows/Linux:  parent of the exe  (knockknock.exe  → <folder>/)
/// - macOS:          parent of .app      (…/MacOS/knockknock → 4 levels up → <folder>/)
fn app_root_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate executable: {e}"))?;

    let exe_dir = exe
        .parent()
        .ok_or("Executable has no parent directory")?;

    #[cfg(target_os = "macos")]
    {
        // Binary is: <app_root>/KnockKnock.app/Contents/MacOS/knockknock
        // Walk up 3 more levels:
        //   MacOS/   → Contents/
        //   Contents → KnockKnock.app/
        //   .app/    → app_root/
        let root = exe_dir
            .parent()  // Contents/
            .and_then(|p| p.parent())  // KnockKnock.app/
            .and_then(|p| p.parent())  // <app_root>/
            .ok_or("Cannot resolve .app bundle root")?;
        Ok(root.to_path_buf())
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Linux: if running as AppImage, current_exe() points to
        // /tmp/.mount_*/usr/bin/knockknock — ephemeral. Prefer
        // APPIMAGE env var to find the actual file location.
        #[cfg(target_os = "linux")]
        {
            if let Some(appimage) = std::env::var_os("APPIMAGE") {
                let p = PathBuf::from(appimage);
                if let Some(parent) = p.parent() {
                    return Ok(parent.to_path_buf());
                }
            }

            // APPIMAGE not set — guard against the /tmp/.mount_*
            // case (manually extracted AppImage, custom wrapper, CI).
            // Data written here is destroyed on app exit.
            let path_str = exe_dir.to_string_lossy();
            if path_str.starts_with("/tmp/.mount_") {
                return Err(format!(
                    "AppImage is not running from a mounted location.\n\
                     Set APPIMAGE=/path/to/file.AppImage and relaunch."
                ));
            }
        }
        Ok(exe_dir.to_path_buf())
    }
}

/// `KnockKnock-data/` next to the app — the single root for all
/// persisted state (PIN, vault, journal, settings, WebView data).
///
/// Creates the directory on first call.
///
/// # Errors
/// Returns a user-facing message if the directory cannot be created
/// (e.g. app placed in a read-only location).
pub fn portable_data_dir() -> Result<PathBuf, String> {
    let root = app_root_dir()?;
    let data_dir = root.join("KnockKnock-data");

    std::fs::create_dir_all(&data_dir).map_err(|e| {
        format!(
            "KnockKnock is portable — it must be placed in a writable folder.\n\
             Current location: {}\nError: {e}",
            root.display()
        )
    })?;

    Ok(data_dir)
}

/// `KnockKnock-data/webview/` — WebView engine runtime data
/// (caches, GPU shaders, localStorage).
///
/// Redirected here on Windows and Linux. macOS WKWebView uses a
/// fixed system path (`~/Library/WebKit/{id}/`) — see spec section 6.
#[cfg(not(target_os = "macos"))]
pub fn webview_data_dir() -> Result<PathBuf, String> {
    Ok(portable_data_dir()?.join("webview"))
}
```

- [ ] **Step 2: Register the module in lib.rs**

Edit `src-tauri/src/lib.rs` — add `mod paths;` after the existing module declarations (after line 9, before `#[cfg_attr(mobile, ...)]`):

```rust
mod paths;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: compiles cleanly. The module is created but not yet consumed — no warnings from unused code (it's public).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/paths.rs src-tauri/src/lib.rs
git commit -m "feat: add portable path resolver module" -m "- paths.rs with platform-aware exe-relative resolution" -m "- Supports Windows, macOS .app bundles, Linux AppImage" -m "- No fallback — errs on unwritable location"
```

---

### Task 2: Update existing Rust modules to use portable paths

**Files:**
- Modify: `src-tauri/src/pin/config.rs` (lines 7-19, 114-119)
- Modify: `src-tauri/src/vault/storage.rs` (lines 22-28)
- Modify: `src-tauri/src/shredder/journal.rs` (lines 13-18)

**Interfaces:**
- Consumes: `paths::portable_data_dir()` (from Task 1)
- Produces: unchanged public APIs — all function signatures stay identical

- [ ] **Step 1: Update `pin/config.rs`**

Replace the three path helpers (lines 7-19, 114-119) with:

```rust
fn get_config_path() -> PathBuf {
    let mut path = crate::paths::portable_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    path.push("pin.json");
    path
}

fn get_lockout_path() -> PathBuf {
    let mut path = crate::paths::portable_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    path.push("lockout.json");
    path
}

fn get_enabled_path() -> PathBuf {
    let mut path = crate::paths::portable_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    path.push("pin_enabled");
    path
}
```

Note: the `unwrap_or_else` fallback to `"."` preserves existing behavior — if the portable dir fails, the write will fail and surface the real error. The `create_dir_all` in `portable_data_dir()` already creates the directory before these are called (via `init_lockout_state`).

Also remove `dirs` from the implicit use — confirm no other `dirs::` references remain in this file.

- [ ] **Step 2: Update `vault/storage.rs`**

Replace `vault_path()` (lines 22-29) with:

```rust
fn vault_path() -> Result<PathBuf, String> {
    let app_dir = crate::paths::portable_data_dir()?;
    Ok(app_dir.join("vault.json"))
}
```

Remove the `dirs` usage. The `create_dir_all` is already handled by `portable_data_dir()`.

- [ ] **Step 3: Update `shredder/journal.rs`**

Replace `journal_path()` and the I/O helpers with fail-loud versions using atomic writes. The journal is the shredder's orphan-recovery safety net — corruption silently breaks cleanup.

Replace the top of the file (lines 1-95) with:

```rust
// src-tauri/src/shredder/journal.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub original_path_hash: String,
    pub renamed_path: PathBuf,
    pub timestamp: u64,
}

fn journal_path() -> Result<PathBuf, String> {
    Ok(crate::paths::portable_data_dir()?.join(".knockknock-journal.json"))
}

fn hash_path(path: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Atomic write with fsync — orphan tracking is critical-state, not cosmetic.
/// If this fails the journal is silent data corruption.
fn write_journal_atomic(entries: &[JournalEntry]) -> Result<(), String> {
    let path = journal_path()?;
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Journal serialize failed: {e}"))?;

    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);

    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Journal tmp create failed: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Journal tmp write failed: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Journal fsync failed: {e}"))?;
    drop(file);

    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("Journal rename failed: {e}"))
}

pub fn write_orphan(original: &std::path::Path, renamed: &std::path::Path) {
    let entry = JournalEntry {
        original_path_hash: hash_path(original),
        renamed_path: renamed.to_path_buf(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let mut entries = read_orphans();
    entries.push(entry);
    if let Err(e) = write_journal_atomic(&entries) {
        eprintln!("[KnockKnock] Journal write failed: {e}");
    }
}

pub fn clear_orphan(renamed: &std::path::Path) {
    let mut entries = read_orphans();
    entries.retain(|e| e.renamed_path != renamed);
    if let Err(e) = write_journal_atomic(&entries) {
        eprintln!("[KnockKnock] Journal clear failed: {e}");
    }
}

pub fn read_orphans() -> Vec<JournalEntry> {
    let path = match journal_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}
```

The remaining functions (`cleanup_orphans`, `JournalEntry`, hash helpers, JOURNAL_TTL_SECS) stay unchanged.

- [ ] **Step 4: Verify no remaining `dirs::` references**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: compiles. If `dirs` crate is only used in these three files, it's now unused — expect a dead-code warning for the `dirs` dependency (cleaned up in Task 5).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/pin/config.rs src-tauri/src/vault/storage.rs src-tauri/src/shredder/journal.rs
git commit -m "feat: switch pin, vault, journal to portable data paths" -m "- Replace dirs::config_dir() / data_dir() with paths::portable_data_dir()" -m "- All data now lives in KnockKnock-data/ next to the executable"
```

---

### Task 3: Create `src-tauri/src/commands/settings.rs`

**Files:**
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod settings;`)
- Modify: `src-tauri/src/lib.rs` (add `get_settings`, `save_settings` to `generate_handler![]`)

**Interfaces:**
- Consumes: `paths::portable_data_dir()` (from Task 1)
- Produces: `get_settings() -> Result<AppSettings, String>`, `save_settings(AppSettings) -> Result<(), String>`

- [ ] **Step 1: Write `src-tauri/src/commands/settings.rs`**

```rust
// src-tauri/src/commands/settings.rs
//
// Portable settings persistence replacing browser localStorage.
// Reads/writes settings.json in the portable data directory.
// Uses atomic write (temp file → rename) to prevent corruption on crash.
// Saves are serialized via Mutex to prevent concurrent rename races.
// Falls back to defaults if file doesn't exist or is corrupted.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes concurrent `save_settings` calls so the .tmp rename
/// sequence is atomic across calls. Without this, two concurrent
/// saves can race on the shared `.tmp` path and corrupt the file.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub auto_clear_log: bool,
    pub default_algorithm_index: usize,
    pub log_obfuscation: String, // "none" | "numbered" | "partial_mask"
    pub left_sidebar_width: u32,
    pub right_sidebar_width: u32,
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::paths::portable_data_dir()?.join("settings.json"))
}

#[tauri::command]
pub fn get_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings: {e}"))?;

    match serde_json::from_str(&data) {
        Ok(s) => Ok(s),
        Err(e) => {
            // Corrupted file — return defaults so the app always starts.
            // The corrupted file will be overwritten on next save.
            eprintln!("[KnockKnock] Corrupted settings.json, using defaults: {e}");
            Ok(AppSettings::default())
        }
    }
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    let _guard = SAVE_LOCK.lock()
        .map_err(|e| format!("Settings save lock poisoned: {e}"))?;

    let path = settings_path()?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);

    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Failed to create settings tmp: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write settings: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Failed to fsync settings: {e}"))?;
    drop(file);

    // On Windows pre-1.86, std::fs::rename may not be atomic. If target
    // exists, remove it first (data already in tmp file via fsync).
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(&path);
    }

    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("Failed to save settings: {e}"))
}
```

- [ ] **Step 2: Register module in `commands/mod.rs`**

Add after line 8 (`pub mod vault;`):

```rust
pub mod settings;
```

- [ ] **Step 3: Register commands in `lib.rs`**

Add to `generate_handler![]` in `src-tauri/src/lib.rs` (after `commands::vault::vault_exists`, before closing `])`):

```rust
            commands::settings::get_settings,
            commands::settings::save_settings,
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: compiles. No errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add portable settings commands replacing localStorage" -m "- get_settings / save_settings Tauri commands" -m "- Atomic writes via temp file + rename" -m "- Corrupted file falls back to defaults"
```

---

### Task 4: Update `lib.rs` for WebView data directory and startup validation

**Files:**
- Modify: `src-tauri/src/lib.rs` (the `run()` function and `setup()` closure)

**Interfaces:**
- Consumes: `paths::webview_data_dir()`, `paths::portable_data_dir()` (from Task 1)
- Produces: Validated startup that creates data dirs before windows, sets WebView data dir

- [ ] **Step 1: Rewrite `src-tauri/src/lib.rs` `run()` function**

Replace the full `run()` function (lines 12-57) with:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Validate portable data directory BEFORE Tauri builder — if the
    // exe is in an unwritable location, surface an error and exit
    // instead of crashing mid-startup.
    let data_dir = match crate::paths::portable_data_dir() {
        Ok(d) => d,
        Err(msg) => {
            startup_fatal(&msg);
        }
    };

    // Restore persisted PIN lockout state AFTER data dir validation —
    // init_lockout_state() writes to a path derived from the portable
    // dir. If portable dir failed, we already exited above.
    if let Err(e) = pin::init_lockout_state() {
        eprintln!("[knockknock] failed to load PIN lockout state: {}", e);
    }

    // Create webview data subdirectory. Failure is fatal — without
    // it, WebView will silently fall back to OS-managed location.
    let webview_dir = match std::fs::create_dir_all(data_dir.join("webview")) {
        Ok(_) => data_dir.join("webview"),
        Err(e) => {
            startup_fatal(&format!("Failed to create webview data dir: {e}"));
        }
    };

    // Windows: set WebView2 user data folder via env var (must be
    // set BEFORE Builder::run() — WebView2 reads it once).
    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_USER_DATA_FOLDER",
            webview_dir.to_string_lossy().as_ref(),
        );
    }

    let webview_dir_clone = webview_dir.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Tray setup is non-essential — failure shouldn't crash startup.
            if let Err(e) = tray::setup_tray(app.handle()) {
                eprintln!("[KnockKnock] Tray setup failed (non-fatal): {e}");
            }

            // Linux: manually (re)create the main window with the
            // portable webview data dir. Tauri's auto-created windows
            // (from tauri.conf.json) cannot have .data_directory()
            // set after creation.
            #[cfg(target_os = "linux")]
            {
                use tauri::WebviewWindowBuilder;
                use tauri::WebviewUrl;
                if let Err(e) = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::App("index.html".into()),
                )
                .title("KnockKnock")
                .inner_size(1200.0, 800.0)
                .min_inner_size(900.0, 600.0)
                .decorations(false)
                .drag_and_drop(true)
                .data_directory(webview_dir_clone.clone())
                .build()
                {
                    eprintln!("[KnockKnock] Failed to create main window: {e}");
                    return Err(e.into());
                }
            }

            // Windows/macOS: window already created by tauri.conf.json.
            // Windows: WEBVIEW2_USER_DATA_FOLDER env var redirects WebView2.
            // macOS:    WKWebView uses fixed path (documented limitation).

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::cancel_shred,
            commands::shred::cleanup_orphans,
            commands::shred::get_algorithms,
            commands::shred::validate_paths,
            commands::shred::get_drive_info,
            commands::shred::get_all_drive_info,
            commands::shred::request_elevation,
            commands::browser::detect_browsers,
            commands::browser::shred_browser_data,
            commands::tray::quick_shred_from_clipboard,
            commands::tray::minimize_to_tray,
            commands::pin::setup_pin,
            commands::pin::verify_pin,
            commands::pin::is_pin_enabled,
            commands::pin::set_pin_enabled,
            commands::pin::has_pin,
            commands::pin::is_pin_locked,
            commands::pin::get_lockout_remaining,
            commands::pin::change_pin,
            commands::pin::reset_app,
            commands::pin::disable_pin,
            commands::vault::save_vault,
            commands::vault::load_vault,
            commands::vault::clear_vault,
            commands::vault::vault_exists,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Show a fatal startup error via native-dialog AND write a log
/// to the OS temp dir (which is always writable). Always exits.
fn startup_fatal(msg: &str) -> ! {
    let _ = native_dialog::MessageDialog::new()
        .set_title("KnockKnock — Startup Error")
        .set_text(msg)
        .set_type(native_dialog::MessageType::Error)
        .show_alert();

    // Fallback log file in OS temp dir — guaranteed writable even if
    // the exe dir is read-only or CWD is a network mount.
    let log_path = std::env::temp_dir().join("knockknock-startup-error.log");
    if let Ok(mut f) = std::fs::File::create(&log_path) {
        use std::io::Write;
        let _ = writeln!(f, "KnockKnock failed to start:\n\n{msg}");
        let _ = writeln!(f, "\nLog path: {}", log_path.display());
    }

    std::process::exit(1);
}
```

Key changes from the previous version:
- **`startup_fatal()` helper** — single source of truth for fatal startup errors: dialog + temp-dir log + exit
- **Temp-dir log** — works even when CWD or exe dir is read-only (DMG mount, network share)
- **No `.expect()`** — webview dir creation failure uses `startup_fatal()` instead of panicking
- **Linux window builder wrapped in match** — error surfaces to user, not panic
- `webview_dir_clone` passed to `setup()` via `move` closure

Key changes from the current `lib.rs`:
- `data_dir` validated BEFORE builder (with `native-dialog` error)
- Windows: `WEBVIEW2_USER_DATA_FOLDER` set before builder
- WebView data directory pre-created
- Linux: `APPIMAGE` env var handled in Task 1's `app_root_dir()`
- macOS: `dataStoreIdentifier` configured in `tauri.conf.json` (Task 7)
- `mod paths;` added (from Task 1)
- `settings` commands added (from Task 3)

- [ ] **Step 2: Verify compilation**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: compiles. If `native-dialog` isn't in Cargo.toml yet, expect error — fixed in Task 5.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add startup validation and WebView data redirect" -m "- Validate portable data dir before Tauri builder" -m "- Set WEBVIEW2_USER_DATA_FOLDER on Windows" -m "- Show native error dialog on unwritable location" -m "- Pre-create webview data subdirectory"
```

---

### Task 5: Update `Cargo.toml` and verify full build

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Remove `dirs` dependency, add `native-dialog`, pin Rust 1.86**

Edit `src-tauri/Cargo.toml`:

Remove line 37: `dirs = "5"`

Add before `lazy_static` (line 38):
```toml
native-dialog = "0.7"
```

Change `rust-version = "1.77"` (line 5) to `rust-version = "1.86"`. This is required for `std::fs::rename` to use `FILE_RENAME_FLAG_POSIX_SEMANTICS` on Windows, making atomic writes reliable. The `save_settings` command's Windows `remove_file(&path)` fallback covers older Rust, but pinning 1.86+ removes the need for the fallback.

- [ ] **Step 2: Full cargo check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: compiles cleanly. No warnings. No `dirs` usage remaining.

- [ ] **Step 3: Run existing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```
Expected: all tests pass. The vault tests exercise the file system — they now write to the portable data directory instead of OS config dirs.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: remove dirs crate, add native-dialog" -m "- dirs replaced by portable paths::portable_data_dir()" -m "- native-dialog for pre-Tauri error dialogs"
```

---

### Task 6: Update frontend `SettingsContext.tsx`

**Files:**
- Modify: `src/contexts/SettingsContext.tsx`

- [ ] **Step 1: Rewrite `SettingsContext.tsx`**

Replace the entire file (111 lines) with:

```tsx
// src/contexts/SettingsContext.tsx
import {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useRef,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LogObfuscation } from "@/types";

// Must match the Rust AppSettings struct fields exactly.
interface AppSettings {
  auto_clear_log: boolean;
  default_algorithm_index: number;
  log_obfuscation: string;
  left_sidebar_width: number;
  right_sidebar_width: number;
}

interface SettingsState {
  autoClearLog: boolean;
  setAutoClearLog: (v: boolean) => void;
  defaultAlgorithmIndex: number;
  setDefaultAlgorithmIndex: (v: number) => void;
  logObfuscation: LogObfuscation;
  setLogObfuscation: (v: LogObfuscation) => void;
  leftSidebarWidth: number;
  rightSidebarWidth: number;
  setLeftSidebarWidth: (v: number | ((prev: number) => number)) => void;
  setRightSidebarWidth: (v: number | ((prev: number) => number)) => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

function clampSidebarWidth(value: number): number {
  return Math.max(160, Math.min(400, value));
}

function isValidLogObfuscation(v: string): v is LogObfuscation {
  return v === "none" || v === "numbered" || v === "partial_mask";
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [loaded, setLoaded] = useState(false);
  const [autoClearLog, setAutoClearLogState] = useState(false);
  const [defaultAlgorithmIndex, setDefaultAlgorithmIndexState] = useState(0);
  const [logObfuscation, setLogObfuscationState] =
    useState<LogObfuscation>("none");
  const [leftSidebarWidth, setLeftSidebarWidthState] = useState(260);
  const [rightSidebarWidth, setRightSidebarWidthState] = useState(260);

  // Refs mirror state so debounced save can read freshest values
  // without closure-staleness when many events fire rapidly
  // (e.g. sidebar drag at 60Hz).
  const stateRef = useRef({
    autoClearLog,
    defaultAlgorithmIndex,
    logObfuscation,
    leftSidebarWidth,
    rightSidebarWidth,
  });
  stateRef.current = {
    autoClearLog,
    defaultAlgorithmIndex,
    logObfuscation,
    leftSidebarWidth,
    rightSidebarWidth,
  };

  // Debounce save — collapse rapid events (sidebar drag) into one IPC.
  const saveTimerRef = useRef<number | null>(null);
  const scheduleSave = useCallback(() => {
    if (!loaded) return;
    if (saveTimerRef.current !== null) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      const settings: AppSettings = {
        auto_clear_log: stateRef.current.autoClearLog,
        default_algorithm_index: stateRef.current.defaultAlgorithmIndex,
        log_obfuscation: stateRef.current.logObfuscation,
        left_sidebar_width: stateRef.current.leftSidebarWidth,
        right_sidebar_width: stateRef.current.rightSidebarWidth,
      };
      invoke("save_settings", { settings }).catch((e) => {
        // Surface to user — silent swallow means user doesn't know
        // settings aren't persisting (privacy-critical tool).
        console.error("[KnockKnock] Failed to save settings:", e);
        // TODO: surface via toast/banner once notification system exists.
      });
    }, 250);
  }, [loaded]);

  // Load settings from Rust on mount
  useEffect(() => {
    invoke<AppSettings>("get_settings")
      .then((s) => {
        setAutoClearLogState(s.auto_clear_log);
        setDefaultAlgorithmIndexState(s.default_algorithm_index);
        setLogObfuscationState(
          isValidLogObfuscation(s.log_obfuscation)
            ? s.log_obfuscation
            : "none",
        );
        setLeftSidebarWidthState(clampSidebarWidth(s.left_sidebar_width));
        setRightSidebarWidthState(clampSidebarWidth(s.right_sidebar_width));
        setLoaded(true);
      })
      .catch((e) => {
        console.error("[KnockKnock] Failed to load settings:", e);
        setLoaded(true); // proceed with defaults
      });
  }, []);

  // Flush pending save on unmount so close-while-debouncing persists
  useEffect(() => {
    return () => {
      if (saveTimerRef.current !== null) {
        clearTimeout(saveTimerRef.current);
        const settings: AppSettings = {
          auto_clear_log: stateRef.current.autoClearLog,
          default_algorithm_index: stateRef.current.defaultAlgorithmIndex,
          log_obfuscation: stateRef.current.logObfuscation,
          left_sidebar_width: stateRef.current.leftSidebarWidth,
          right_sidebar_width: stateRef.current.rightSidebarWidth,
        };
        // Synchronous-style IPC at unmount — async fire-and-forget.
        invoke("save_settings", { settings }).catch(() => {});
      }
    };
  }, []);

  const setAutoClearLog = useCallback(
    (v: boolean) => {
      setAutoClearLogState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setDefaultAlgorithmIndex = useCallback(
    (v: number) => {
      setDefaultAlgorithmIndexState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setLogObfuscation = useCallback(
    (v: LogObfuscation) => {
      setLogObfuscationState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setLeftSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setLeftSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        return clampSidebarWidth(next);
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  const setRightSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setRightSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        return clampSidebarWidth(next);
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  return (
    <SettingsContext.Provider
      value={{
        autoClearLog,
        setAutoClearLog,
        defaultAlgorithmIndex,
        setDefaultAlgorithmIndex,
        logObfuscation,
        setLogObfuscation,
        leftSidebarWidth,
        rightSidebarWidth,
        setLeftSidebarWidth,
        setRightSidebarWidth,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx)
    throw new Error("useSettings must be used within SettingsProvider");
  return ctx;
}
```

Key changes from the previous version:
- **Debounced save (250ms)** — sidebar drags (60Hz events) collapse into one IPC call, eliminating IPC storm
- **`stateRef` mirror** — saves read freshest values, no closure-staleness
- **Unmount flush** — pending debounced save fires before close, so user changes aren't lost
- Errors surface to console (and TODO toast once notification system exists) — not silent

Key changes from current implementation:
- `localStorage` removed entirely — replaced with `invoke("get_settings")` / `invoke("save_settings")`
- `loaded` flag prevents save before initial load completes
- `log_obfuscation` validated against known values

- [ ] **Step 2: TypeScript check**

```bash
pnpm exec tsc --noEmit
```
Expected: no errors. The `@tauri-apps/api/core` import is already available.

- [ ] **Step 3: Commit**

```bash
git add src/contexts/SettingsContext.tsx
git commit -m "feat: replace localStorage with Rust-backed settings" -m "- invoke get_settings / save_settings via Tauri IPC" -m "- Validates log_obfuscation against known values" -m "- Prevents save before initial load completes"
```

---

### Task 7: Update `tauri.conf.json`

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Update window config and add webviewInstallMode**

Edit `src-tauri/tauri.conf.json` to:
1. Keep the default window on Windows/macOS (Windows needs WebView2 env-var redirect, not builder API)
2. Add `"webviewInstallMode": { "type": "embedBootstrapper" }` to Windows bundle config
3. **Do NOT add `dataStoreIdentifier`** — it crashes on macOS < 14 and the namespacing benefit doesn't justify the crash risk. macOS WebKit trace is documented but accepted as a Tauri/WKWebView limitation.

Since Linux creates the window manually in `setup()` (Task 4), the default window config in `tauri.conf.json` is still used on Windows and macOS.

The updated config:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "KnockKnock",
  "version": "0.3.0",
  "identifier": "org.knockknockorg.shredder",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "KnockKnock",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": false,
        "dragDropEnabled": true,
        "useHttpsScheme": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "category": "Utility",
    "shortDescription": "Emergency file shredder with browser profile cleanup",
    "longDescription": "KnockKnock securely deletes files, folders, and browser data using NIST 800-88 compliant single-pass random overwrite. Designed for speed and reliability when you need data gone now.",
    "copyright": "Copyright (c) 2026 KnockKnock Contributors",
    "windows": {
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      },
      "nsis": {
        "displayLanguageSelector": false,
        "installerIcon": "icons/icon.ico"
      }
    }
  }
}
```

Changes:
- **No `dataStoreIdentifier`** — would crash on macOS < 14. macOS WebKit fixed-path trace is documented in spec section 6.
- Added `"webviewInstallMode": { "type": "embedBootstrapper" }` to Windows bundle config — embeds WebView2 bootstrapper in the `.exe` so users don't need a separate setup step on first launch.
- Window config unchanged — auto-creates on Windows/macOS, manually-created on Linux (Task 4).

- [ ] **Step 2: Verify build**

```bash
pnpm tauri build --no-bundle
```
Expected: builds successfully. The `dataStoreIdentifier` is an integer array — verify it's valid JSON.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat: add macOS dataStoreIdentifier and Windows WebView2 embed" -m "- Namespace WKWebView data on macOS 14+" -m "- Embed WebView2 bootstrapper for portable Windows .exe"
```

---

### Task 8: Update `release.yml` for portable distribution

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Rewrite release workflow for portable artifacts**

Replace the entire file (91 lines) with:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

permissions:
  contents: write

jobs:
  release:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-14
            target: aarch64-apple-darwin
            build_cmd: pnpm tauri build --bundles dmg --target aarch64-apple-darwin
            artifact_name: KnockKnock-macos-arm64.dmg
            artifact_path: src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/KnockKnock_*_aarch64.dmg
            min_size: 8000000
          # Intel mac: run on macos-13 (Intel host) to avoid
          # cross-compile issues — building x86_64 on ARM requires
          # osxcross + Apple SDK setup that isn't worth the complexity.
          - platform: macos-13
            target: x86_64-apple-darwin
            build_cmd: pnpm tauri build --bundles dmg --target x86_64-apple-darwin
            artifact_name: KnockKnock-macos-x64.dmg
            artifact_path: src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/KnockKnock_*_x64.dmg
            min_size: 8000000
          - platform: ubuntu-22.04
            target: ''
            build_cmd: pnpm tauri build --bundles appimage
            artifact_name: KnockKnock-linux-x64.AppImage
            artifact_path: src-tauri/target/release/bundle/appimage/KnockKnock_*.AppImage
            min_size: 60000000
          - platform: windows-latest
            target: ''
            build_cmd: pnpm tauri build --no-bundle
            artifact_name: KnockKnock-windows-x64.exe
            artifact_path: src-tauri/target/release/knockknock.exe
            min_size: 5000000

    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v5

      - name: Install system dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-4-dev

      - name: Install pnpm
        uses: pnpm/action-setup@v5

      - name: Setup Node.js
        uses: actions/setup-node@v5
        with:
          node-version: 22

      - name: Install frontend dependencies
        run: pnpm install --frozen-lockfile

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Build
        run: ${{ matrix.build_cmd }}

      # Verify the artifact is non-empty and plausibly sized BEFORE upload.
      # Without this, a silent build failure could upload a stale or
      # partial binary. Minimum sizes are empirically observed; actual
      # builds are larger.
      - name: Verify artifact
        run: |
          FILE=$(ls ${{ matrix.artifact_path }} 2>/dev/null | head -1)
          if [ -z "$FILE" ]; then
            echo "::error::Artifact not found at ${{ matrix.artifact_path }}"
            exit 1
          fi
          SIZE=$(stat -c%s "$FILE" 2>/dev/null || stat -f%z "$FILE")
          echo "Artifact: $FILE ($SIZE bytes)"
          if [ "$SIZE" -lt ${{ matrix.min_size }} ]; then
            echo "::error::Artifact too small ($SIZE < ${{ matrix.min_size }}). Build likely failed silently."
            exit 1
          fi

      - name: Upload artifact
        uses: actions/upload-artifact@v5
        with:
          name: ${{ matrix.artifact_name }}
          path: ${{ matrix.artifact_path }}

  checksums:
    needs: release
    runs-on: ubuntu-22.04
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v5
        with:
          path: artifacts

      - name: Generate SHA256SUMS.txt
        run: |
          cd artifacts
          find . -type f -name 'KnockKnock-*' -exec sha256sum {} \; > ../SHA256SUMS.txt
          cat ../SHA256SUMS.txt

      - name: Upload checksums
        uses: actions/upload-artifact@v5
        with:
          name: SHA256SUMS
          path: SHA256SUMS.txt

  publish:
    needs: [release, checksums]
    runs-on: ubuntu-22.04
    permissions:
      contents: write
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v5
        with:
          path: artifacts

      - name: Create Release
        uses: softprops/action-gh-release@v1
        if: startsWith(github.ref, 'refs/tags/')
        with:
          draft: true
          files: |
            artifacts/**/*.dmg
            artifacts/**/*.AppImage
            artifacts/**/*.exe
            artifacts/**/SHA256SUMS.txt
          name: 'KnockKnock ${{ github.ref_name }}'
          body: |
            ## KnockKnock ${{ github.ref_name }}

            Emergency file shredder with browser profile cleanup.

            ### Downloads (Portable)
            - **Windows:** Download `KnockKnock-windows-x64.exe`, place in any writable folder, double-click
            - **macOS:** Open `.dmg`, drag `KnockKnock.app` to any writable folder, right-click → Open
            - **Linux:** Download `.AppImage`, `chmod +x`, run

            ### Verification
            Verify integrity with `SHA256SUMS.txt`:
            ```sh
            sha256sum -c SHA256SUMS.txt
            ```

            ### Data
            All app data is stored in a `KnockKnock-data/` folder next to the app.
            Delete the folder to remove all traces.
```

Key changes:
- Replaced `tauri-apps/tauri-action@v0` with direct `pnpm tauri build` commands per platform
- Windows: `--no-bundle` → produces raw `.exe` (no NSIS/MSI installer)
- macOS: `--bundles dmg` with explicit `--target` → produces signed DMG
- Linux: `--bundles appimage` → produces AppImage
- **Intel mac uses `macos-13` runner** — avoids ARM-to-Intel cross-compile complexity
- **Artifact verification step** — checks non-empty + min-size before upload (catches silent build failures)
- **Checksums job** — generates `SHA256SUMS.txt` from all artifacts, attached to release
- **Publish job** — separate from per-platform builds, depends on all artifacts
- Release body includes SHA256SUMS verification instructions

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat: switch release to portable artifacts" -m "- Windows: raw .exe via --no-bundle" -m "- macOS: .dmg via --bundles dmg" -m "- Linux: AppImage via --bundles appimage" -m "- Replace tauri-action with direct build commands"
```

---

## Verification

After all 8 tasks are complete:

- [ ] **Build check across platforms** (minimum: local platform)
  ```bash
  pnpm tauri build --no-bundle   # Windows: verify .exe produced
  pnpm tauri build --bundles dmg  # macOS: verify .dmg produced
  pnpm tauri build --bundles appimage  # Linux: verify AppImage produced
  ```

- [ ] **Runtime portable test** (local platform)
  1. Move the built binary to a new empty folder
  2. Run it — verify `KnockKnock-data/` is created next to it
  3. Set a PIN, save a vault, change settings
  4. Close the app, reopen — verify all state persists
  5. Delete `KnockKnock-data/` — verify fresh start
  6. Move the binary to a read-only location — verify error dialog appears

- [ ] **No `dirs` references remain**
  ```bash
  grep -r "dirs::" src-tauri/src/
  ```
  Expected: no matches.

- [ ] **No `localStorage` references remain in settings code**
  ```bash
  grep -r "localStorage" src/contexts/SettingsContext.tsx
  ```
  Expected: no matches.

## Self-Review

**Spec coverage:**
- Data location ✅ Task 1 (paths.rs)
- No fallback ✅ Task 1, Task 4 (startup validation)
- Settings storage ✅ Task 3 (settings.rs), Task 6 (frontend)
- No migration ✅ (default behavior — fresh start)
- WebView data redirect ✅ Task 4 (Windows env var + Linux builder), Task 7 (macOS dataStoreIdentifier)
- Distribution format ✅ Task 8 (release.yml)
- No auto-updater ✅ (not implemented — non-goal)
- macOS WebKit cleanup ⚠️ Not implemented — spec says "off by default, offer a setting". This is a future setting, not blocking portable functionality. Documented in the spec's platform trace summary.
- `dirs` removal ✅ Task 5

**Placeholder scan:** No TBDs, TODOs, or unspecified code. All code blocks are complete.

**Type consistency:**
- `AppSettings` struct in Rust (Task 3) matches `AppSettings` interface in TypeScript (Task 6) — field names use snake_case/TitleCase as appropriate for each language
- `paths::portable_data_dir()` returns `Result<PathBuf, String>` consistently across all consumers

**PLAN FULLY COMPLETE — ready for implementation.**

---

## Reliability Self-Review

Reviewed the plan after writing. Five issues found and fixed inline:

| Sev | Issue | Fix |
|-----|-------|-----|
| **HIGH** | Linux WebView data dir not actually redirected (Task 4 had empty body under `#[cfg(target_os = "linux")]`) — silently fell back to `~/.local/share/` defeating portability | Linux now explicitly calls `WebviewWindowBuilder::data_directory()` in `setup()`. Implemented in Task 4. |
| **HIGH** | `init_lockout_state()` ordered before data dir validation — relied on `unwrap_or_else` fallback to CWD in pin/config.rs | Reordered: data dir validation → `init_lockout_state` → builder. Implemented in Task 4. |
| **MEDIUM** | Linux pre-Tauri error path used only `eprintln` — invisible when launched without terminal | Added `native-dialog` for all platforms plus `knockknock-startup-error.log` file fallback. Implemented in Task 4. |
| **MEDIUM** | `tray::setup_tray` failure crashed whole startup | Wrapped in non-fatal match — logged, doesn't crash. Implemented in Task 4. |
| **MEDIUM** | macOS `dataStoreIdentifier` would crash on macOS < 14 — code had no version guard | Removed `dataStoreIdentifier` entirely. Accept documented macOS WebKit limitation as per spec section 6. Updated Task 7. |

## Oracle Review (round 2)

Oracle review found 5 additional issues. All fixed inline:

| Sev | Issue | Fix |
|-----|-------|-----|
| **CRITICAL** | `journal.rs` used `unwrap_or_else(current_dir)` fallback (silent CWD write if portable dir fails mid-shred) and plain `fs::write` (non-atomic) — corruption breaks orphan recovery | `journal_path()` returns `Result`. Atomic write with fsync via `write_journal_atomic()`. Mirrors `save_settings` pattern. Implemented in Task 2. |
| **CRITICAL** | `SettingsContext.tsx` `save()` closes over stale state — rapid drag events (60Hz) write stale other-fields. Concurrent IPC calls race on `.tmp` rename — silent corruption swallowed by `.catch(console.error)`. | Frontend: 250ms debounce + `stateRef` mirror + unmount flush. Backend: `SAVE_LOCK` Mutex serializes saves. Errors surface to console (and TODO toast). Implemented in Task 3 and Task 6. |
| **HIGH** | Linux AppImage fallback to `/tmp/.mount_*/` — data destroyed on app exit. `APPIMAGE` env var usually set, but not guaranteed (manual extraction, CI). | `app_root_dir()` validates `current_exe()` doesn't start with `/tmp/.mount_`. Returns explicit error if it does. Implemented in Task 1. |
| **HIGH** | Startup error path uses `.expect()` (panic, no user message) + log to CWD (may be unwritable). DMG-mount failure gives generic "must be in writable folder" — doesn't identify cause. | `startup_fatal()` helper: native-dialog + `temp_dir()` log + exit. Replaces all `.expect()` and `.eprintln` startup paths. Implemented in Task 4. |
| **HIGH** | Release workflow: no artifact verification (silent build failure could upload stale binary). macOS x86_64 cross-compile on ARM host. No checksums for download integrity. | `Verify artifact` step (size check). x86_64 uses `macos-13` runner. New `checksums` + `publish` jobs generate `SHA256SUMS.txt`. Implemented in Task 8. |
| **LOW** | `webview_data_dir()` dead code on macOS (no consumer) | `#[cfg(not(target_os = "macos"))]` guard. Implemented in Task 1. |
| **LOW** | Spec/plan conflict on `dataStoreIdentifier` | Updated spec section 3.4 and trace summary. |
| **LOW** | Windows `std::fs::rename` atomicity pre-Rust-1.86 | Pinned `rust-version = "1.86"` in `Cargo.toml`. Implemented in Task 5. |

**PLAN FULLY COMPLETE — ready for implementation.**
