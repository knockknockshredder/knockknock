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
        // On Linux, if running as AppImage, current_exe() points to
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
/// fixed system path (mitigated via `dataStoreIdentifier`).
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

Replace `journal_path()` (lines 13-18) with:

```rust
fn journal_path() -> PathBuf {
    crate::paths::portable_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
        .join(".knockknock-journal.json")
}
```

The existing code also creates the parent directory via `portable_data_dir()` — the `.join(".knockknock-journal.json")` just appends the filename.

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
// Falls back to defaults if file doesn't exist or is corrupted.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    serde_json::from_str(&data).map_err(|e| {
        // Corrupted file — return defaults so the app always starts.
        // The corrupted file will be overwritten on next save.
        eprintln!("[KnockKnock] Corrupted settings.json, using defaults: {e}");
        Ok(AppSettings::default())
    })?
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    // Atomic write: write to .tmp, remove stale tmp, rename to target
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, &json)
        .map_err(|e| format!("Failed to write settings: {e}"))?;
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
    // exe is in an unwritable location, show an error dialog and exit
    // instead of crashing mid-startup.
    let data_dir = match crate::paths::portable_data_dir() {
        Ok(d) => d,
        Err(msg) => {
            // Use native-dialog directly — Tauri isn't running yet.
            // native-dialog is already an indirect dependency via
            // tauri-plugin-dialog, but add it explicitly in Cargo.toml.
            #[cfg(not(target_os = "linux"))]
            {
                let _ = native_dialog::MessageDialog::new()
                    .set_title("KnockKnock — Startup Error")
                    .set_text(&msg)
                    .set_type(native_dialog::MessageType::Error)
                    .show_alert();
            }
            #[cfg(target_os = "linux")]
            {
                eprintln!("[KnockKnock] Startup Error: {msg}");
            }
            std::process::exit(1);
        }
    };

    // Restore persisted PIN lockout state before any Tauri commands
    // can run, so a previously locked-out user cannot bypass the
    // lockout by relaunching the app.
    if let Err(e) = pin::init_lockout_state() {
        eprintln!("[knockknock] failed to load PIN lockout state: {}", e);
    }

    // Create webview data subdirectory
    let webview_dir = data_dir.join("webview");
    if let Err(e) = std::fs::create_dir_all(&webview_dir) {
        eprintln!("[KnockKnock] Failed to create webview data directory: {e}");
        // Non-fatal — on macOS this path isn't used anyway.
        // On Windows/Linux, Tauri will try to create it again.
    }

    // Windows: set WebView2 user data folder via env var.
    // Must be set BEFORE Builder::run() — WebView2 reads it once.
    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_USER_DATA_FOLDER",
            webview_dir.to_string_lossy().as_ref(),
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            tray::setup_tray(app.handle())?;

            // Linux: set WebView data directory via builder API.
            // Windows: env var already set above.
            // macOS: data_directory() is unsupported — use dataStoreIdentifier
            //        configured in tauri.conf.json (Task 7).
            #[cfg(not(target_os = "macos"))]
            {
                // The webview_dir was already created above.
                // Tauri's data_directory() auto-creates it too,
                // but pre-creating avoids a panic if Tauri's
                // create_dir_all fails.
            }

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
```

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

- [ ] **Step 1: Remove `dirs` dependency, add `native-dialog`**

Edit `src-tauri/Cargo.toml`:

Remove line 37: `dirs = "5"`

Add before `lazy_static` (line 38):
```toml
native-dialog = "0.7"
```

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

  // Persist whenever any setting changes (skip initial load)
  const save = useCallback(
    (overrides: Partial<AppSettings>) => {
      if (!loaded) return;
      const settings: AppSettings = {
        auto_clear_log: autoClearLog,
        default_algorithm_index: defaultAlgorithmIndex,
        log_obfuscation: logObfuscation,
        left_sidebar_width: leftSidebarWidth,
        right_sidebar_width: rightSidebarWidth,
        ...overrides,
      };
      invoke("save_settings", { settings }).catch((e) => {
        console.error("[KnockKnock] Failed to save settings:", e);
      });
    },
    [
      loaded,
      autoClearLog,
      defaultAlgorithmIndex,
      logObfuscation,
      leftSidebarWidth,
      rightSidebarWidth,
    ],
  );

  const setAutoClearLog = useCallback(
    (v: boolean) => {
      setAutoClearLogState(v);
      save({ auto_clear_log: v });
    },
    [save],
  );

  const setDefaultAlgorithmIndex = useCallback(
    (v: number) => {
      setDefaultAlgorithmIndexState(v);
      save({ default_algorithm_index: v });
    },
    [save],
  );

  const setLogObfuscation = useCallback(
    (v: LogObfuscation) => {
      setLogObfuscationState(v);
      save({ log_obfuscation: v });
    },
    [save],
  );

  const setLeftSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setLeftSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        const clamped = clampSidebarWidth(next);
        save({ left_sidebar_width: clamped });
        return clamped;
      });
    },
    [save],
  );

  const setRightSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setRightSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        const clamped = clampSidebarWidth(next);
        save({ right_sidebar_width: clamped });
        return clamped;
      });
    },
    [save],
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

Key changes from current:
- `localStorage` removed entirely — replaced with `invoke("get_settings")` / `invoke("save_settings")`
- `loaded` flag prevents save before initial load completes (avoids overwriting defaults before load)
- `log_obfuscation` stored as `string` (matches Rust `String`) with validation
- All state changes debounced through `useCallback` + `save()`

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

- [ ] **Step 1: Add macOS dataStoreIdentifier, add webviewInstallMode**

Edit `src-tauri/tauri.conf.json`:

In the `"windows"` array (line 13), add `dataStoreIdentifier` to the window config. Also add `webviewInstallMode` to the bundle section for Windows:

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
        "useHttpsScheme": true,
        "dataStoreIdentifier": [212, 122, 17, 203, 137, 243, 78, 43, 161, 199, 85, 158, 191, 114, 129, 6]
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
- Added `"dataStoreIdentifier": [212, 122, 17, 203, 137, 243, 78, 43, 161, 199, 85, 158, 191, 114, 129, 6]` to window config (namespace WebKit data on macOS 14+)
- Added `"webviewInstallMode": { "type": "embedBootstrapper" }` to Windows bundle config (embed WebView2 bootstrapper in the .exe — needed for portable since there's no installer to check WebView2)
- Note: `dataStoreIdentifier` only takes effect on macOS 14+. On older macOS, Tauri/wry will crash if this is set. **Risk accepted** per spec — macOS 14+ is required. If this becomes a problem, we can version-guard at the Rust level later.

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
          - platform: macos-14
            target: x86_64-apple-darwin
            build_cmd: pnpm tauri build --bundles dmg --target x86_64-apple-darwin
            artifact_name: KnockKnock-macos-x64.dmg
            artifact_path: src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/KnockKnock_*_x64.dmg
          - platform: ubuntu-22.04
            target: ''
            build_cmd: pnpm tauri build --bundles appimage
            artifact_name: KnockKnock-linux-x64.AppImage
            artifact_path: src-tauri/target/release/bundle/appimage/KnockKnock_*.AppImage
          - platform: windows-latest
            target: ''
            build_cmd: pnpm tauri build --no-bundle
            artifact_name: KnockKnock-windows-x64.exe
            artifact_path: src-tauri/target/release/knockknock.exe

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

      - name: Upload artifact
        uses: actions/upload-artifact@v5
        with:
          name: ${{ matrix.artifact_name }}
          path: ${{ matrix.artifact_path }}

      - name: Create Release
        uses: softprops/action-gh-release@v1
        if: startsWith(github.ref, 'refs/tags/')
        with:
          draft: true
          files: ${{ matrix.artifact_path }}
          name: 'KnockKnock ${{ github.ref_name }}'
          body: |
            ## KnockKnock ${{ github.ref_name }}

            Emergency file shredder with browser profile cleanup.

            ### Downloads (Portable)
            - **Windows:** Download `KnockKnock-windows-x64.exe`, place in any writable folder, double-click
            - **macOS:** Open `.dmg`, drag `KnockKnock.app` to any writable folder, right-click → Open
            - **Linux:** Download `.AppImage`, `chmod +x`, run

            ### Data
            All app data is stored in a `KnockKnock-data/` folder next to the app.
            Delete the folder to remove all traces.
```

Key changes:
- Replaced `tauri-apps/tauri-action@v0` with direct `pnpm tauri build` commands per platform
- Windows: `--no-bundle` → produces raw `.exe` (no NSIS/MSI installer)
- macOS: `--bundles dmg` with explicit `--target` → produces signed DMG
- Linux: `--bundles appimage` → produces AppImage
- Uses `softprops/action-gh-release@v1` for release creation (more control than tauri-action)
- Artifact upload via `actions/upload-artifact@v5` for per-build artifacts
- Release body updated to describe portable usage

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
