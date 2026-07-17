# Portable App Design

**Date:** 2026-07-17
**Status:** Draft
**Goal:** Make KnockKnock fully portable — downloading the binary is all that's needed to run on all platforms, with zero OS traces (or documented, mitigated exceptions).

---

## 1. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Data location | `KnockKnock-data/` folder next to executable | Portable. Delete folder = zero trace. |
| Unwritable exe location | Error dialog, refuse to start | No hidden fallback data dirs. Portable contract is strict. |
| Settings storage | Rust-backed `settings.json` in portable data folder | Replaces localStorage. Full control, no WebView dependency. |
| Existing user data | No migration, start fresh | App is v0.3.0. Old OS data stays where it is. |
| WebView data | Redirect to `KnockKnock-data/webview/` where possible; macOS documented limitation | Windows/Linux: fully redirected. macOS: WKWebView fixed path, mitigated. |
| Distribution format | Windows: raw `.exe`. macOS: `.dmg`. Linux: `.AppImage` | Matches each platform's native portable format. |
| Auto-updater | Not included | Scope: portability only. Add later if needed. |
| macOS WebKit cleanup | Optionally delete `~/Library/WebKit/{bundle-id}/` on quit | Mitigates the WKWebView trace. Off by default. |

---

## 2. Data Layout

```
<wherever-you-put-the-app>/
├── knockknock.exe              # Windows: single executable
├── KnockKnock.app/             # macOS: .app bundle (binary at Contents/MacOS/knockknock)
├── KnockKnock.AppImage         # Linux: single-file AppImage
└── KnockKnock-data/            # Created on first launch, deleted by user for cleanup
    ├── webview/                # WebView engine runtime (Windows + Linux)
    ├── pin.json                # bcrypt PIN hash
    ├── lockout.json            # Failed attempts + lockout timestamp
    ├── pin_enabled             # Plain text flag ("1" / "0")
    ├── vault.json              # AES-256-GCM encrypted shred list
    ├── .knockknock-journal.json # Orphan recovery tracking
    └── settings.json           # UI preferences (new — replaces localStorage)
```

**macOS note:** Data is created NEXT TO the `.app` bundle, not inside it. The binary is 3 levels deep inside the bundle (`Contents/MacOS/knockknock`), so the resolver walks up 4 levels to reach the directory containing the `.app`.

### Path resolution

```rust
/// Returns the directory CONTAINING the app.
/// - Windows/Linux: parent of the exe (knockknock.exe → <folder>/)
/// - macOS: parent of the .app bundle (Contents/MacOS/knockknock → 4 levels up → <folder>/)
fn app_root_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate executable: {e}"))?;
    let exe_dir = exe.parent()
        .ok_or("Executable has no parent directory")?;

    #[cfg(target_os = "macos")]
    {
        // Binary is at: <app_root>/KnockKnock.app/Contents/MacOS/knockknock
        // Walk up 3 more levels: MacOS → Contents → KnockKnock.app → app_root
        let root = exe_dir.parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .ok_or("Cannot resolve .app bundle root")?;
        Ok(root.to_path_buf())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(exe_dir.to_path_buf())
    }
}

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
```

**No fallback.** If the location is unwritable (e.g. `C:\Program Files\` or `/Applications/`), the app displays an error dialog and exits.

---

## 3. Rust Changes

### 3.1 New module: `src-tauri/src/paths.rs`

Centralized portable path resolver. Provides two functions:

- `portable_data_dir() -> Result<PathBuf, String>` — app data root (`KnockKnock-data/`)
- `webview_data_dir() -> Result<PathBuf, String>` — WebView subfolder (`KnockKnock-data/webview/`)

### 3.2 Modules updated

| File | Change | Old API | New API |
|------|--------|---------|---------|
| `src/pin/config.rs` | Replace config dir | `dirs::config_dir()` + `KnockKnock/` | `paths::portable_data_dir()` |
| `src/vault/storage.rs` | Replace config dir | `dirs::config_dir()` + `KnockKnock/` | `paths::portable_data_dir()` |
| `src/shredder/journal.rs` | Replace data dir | `dirs::data_dir()` + `KnockKnock/` | `paths::portable_data_dir()` |
| `src/lib.rs` | Set WebView data dir | None (OS default) | `webview_data_dir()` via builder API |

### 3.3 New module: `src-tauri/src/commands/settings.rs`

Two Tauri commands replacing localStorage:

```rust
#[tauri::command]
fn get_settings() -> Result<AppSettings, String> {
    // If file doesn't exist or is corrupted, return defaults.
    // Corrupted file gets overwritten on next save.
    // Never fails — always returns valid settings.
}

#[tauri::command]
fn save_settings(settings: AppSettings) -> Result<(), String> {
    // Atomic write: write to .tmp, then rename (prevents corruption on crash).
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub auto_clear_log: bool,
    pub default_algorithm_index: usize,
    pub log_obfuscation: bool,
    pub left_sidebar_width: u32,
    pub right_sidebar_width: u32,
}
```

Registered in `generate_handler![]` in `lib.rs`.

### 3.4 WebView data directory — platform-specific

**Windows (WebView2):** Set `WEBVIEW2_USER_DATA_FOLDER` env var to `webview_data_dir()` before Tauri builder runs. WebView2 fully supports custom user data folders via this env var or the builder API's `.data_directory()`.

**Linux (WebKitGTK):** Use `WebviewWindowBuilder::data_directory()` with absolute path. Fully supported.

**macOS (WKWebView):** `data_directory()` is **unsupported** — WKWebView always uses `~/Library/WebKit/{bundle-id}/`. Mitigation:
- Scope via `dataStoreIdentifier` (macOS 14.0+) — use a fixed UUID `[0xD4, 0x7A, 0x11, 0xCB, 0x89, 0xF3, 0x4E, 0x2B, 0xA1, 0xC7, 0x55, 0x9E, 0xBF, 0x72, 0x81, 0x06]` to namespace WebKit data. Prevents collision with any other app using the same bundle ID.
- Offer a setting to delete `~/Library/WebKit/{bundle-id}/` on app quit (off by default)
- Document the limitation in README

### 3.5 `Cargo.toml`

- Remove `dirs` crate dependency
- No new dependencies needed

---

## 4. Frontend Changes

### 4.1 `src/contexts/SettingsContext.tsx`

Replace all `localStorage.getItem('knockknock-settings')` and `localStorage.setItem(...)` with:

```typescript
import { invoke } from '@tauri-apps/api/core';

// Load
const settings = await invoke<AppSettings>('get_settings');

// Save
await invoke('save_settings', { settings });
```

The context provider structure stays the same — only the persistence backend changes.

### 4.2 Type definitions

Add `AppSettings` interface matching the Rust struct. No new dependencies.

---

## 5. Packaging & Distribution

### 5.1 Build commands

CI-driven, not config-driven. `tauri.conf.json` `bundle.targets` stays `"all"`.

| Platform | Build command | Artifact path |
|----------|--------------|---------------|
| Windows | `pnpm tauri build --no-bundle` | `src-tauri/target/release/knockknock.exe` |
| macOS | `pnpm tauri build --bundles dmg` | `src-tauri/target/release/bundle/dmg/KnockKnock.dmg` |
| Linux | `pnpm tauri build --bundles appimage` | `src-tauri/target/release/bundle/appimage/KnockKnock.AppImage` |

### 5.2 Windows: No installer

NSIS/MSI installers are not produced. The raw `.exe` is distributed directly. WebView2 bootstrapper is embedded (`webviewInstallMode: "embedBootstrapper"`). Users download `knockknock.exe`, place it in any writable folder, double-click.

### 5.3 macOS: `.dmg` wrapping `.app`

The `.app` bundle is wrapped in a `.dmg` for distribution (prevents browser corruption of `.app` bundles, provides a familiar mount-and-drag UX). Users drag the `.app` anywhere, data creates next to it.

### 5.4 Linux: AppImage

Single-file AppImage bundles all dependencies (GTK3, WebKitGTK). Users `chmod +x` and run. Data creates next to the AppImage.

### 5.5 GitHub Actions (`release.yml`)

Update the build matrix to use platform-specific build commands (see 5.1). The `tauri-apps/tauri-action@v0` action can be replaced with manual `pnpm tauri build` commands for finer control.

### 5.6 Release artifacts

| Platform | Artifact name pattern | Size |
|----------|----------------------|------|
| Windows | `KnockKnock-windows-x64.exe` | ~8 MB |
| macOS (Intel) | `KnockKnock-macos-x64.dmg` | ~10 MB |
| macOS (ARM) | `KnockKnock-macos-arm64.dmg` | ~10 MB |
| Linux | `KnockKnock-linux-x64.AppImage` | ~80 MB (bundles WebKitGTK) |

---

## 6. Platform Trace Summary

| Trace | Windows | Linux | macOS | Mitigation |
|-------|---------|-------|-------|------------|
| App data files | ✅ Portable | ✅ Portable | ✅ Portable | In `KnockKnock-data/` next to binary |
| WebView engine data | ✅ Portable | ✅ Portable | ⚠️ `~/Library/WebKit/{id}/` | Scoped via `dataStoreIdentifier`. Optional quit-cleanup setting. |
| Registry / package DB | ✅ None | ✅ None | ✅ None | No installer used |
| WebView2 runtime | ⚠️ OS-level component | N/A | N/A | One-time system install. Not app-specific. |

---

## 7. Non-Goals

- Auto-updater
- Migration from v0.3.0 installed version
- Installer-based distribution (NSIS, MSI, deb, rpm)
- macOS notarization (out of scope; users accept Gatekeeper warning)
