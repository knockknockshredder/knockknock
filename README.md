# KnockKnock
[![Release](https://img.shields.io/github/v/release/knockknockshredder/knockknock)](https://github.com/knockknockshredder/knockknock/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)](#install)

**Emergency file shredder with browser profile cleanup.**

> One button. Gone forever. Greet your guests.

## What It Does

KnockKnock securely destroys files, folders, and browser data. Choose from multiple algorithms and verification levels — from a fast single random pass to the full DoD 5220.22-M 3-pass sequence with byte-level read-back verification.

### Features

- **Multiple algorithms** — NIST 800-88 Clear, DoD 5220.22-M, or Random Only
- **Multiple patterns** — Random (cryptographically secure), Zeros, or Ones
- **Verification** — None, Sample (start/middle/end), or Full (every byte)
- **Browser cleanup** — Detects 9+ browsers, shreds profiles/cache/cookies/history/passwords/extensions
- **Cross-platform** — Windows, macOS, Linux
- **SSD-aware** — Detects drive type, applies TRIM after overwrite (with wear-leveling warnings)
- **Encrypted vault** — Persists your shred list encrypted with AES-256-GCM, unlocked by your PIN
- **PIN protection** — bcrypt-hashed PIN with 3-attempt lockout (5 min), survives app restart
- **System tray** — Minimize to tray for quick access, clipboard shredding
- **Cancel safely** — Mid-shred cancellation still renames, truncates, and deletes
- **Orphan recovery** — Journal-based crash recovery for interrupted shreds
- **Hard link detection** — Warns before shredding files with multiple hard links
- **Log obfuscation** — Hide file paths in logs (numbered or partial mask modes)
- **Real-time progress** — Speed and ETA per file via live progress events
- **Administrator elevation** — UAC elevation on Windows for locked files

### Supported Browsers

Chrome (incl. Beta, Canary), Firefox, Edge (incl. Beta), Brave (incl. Beta), Opera (incl. Next), Vivaldi, Safari, Tor Browser, Chromium, Internet Explorer — auto-detected per platform, with lock-file detection for running browsers.

### Supported Algorithms

| Algorithm | Default passes | Max passes | Pattern | Description |
|-----------|---------------|------------|---------|-------------|
| **NIST 800-88 Clear** | 1 | 35 | Random, Zeros, Ones | NIST SP 800-88 Clear standard. Single-pass overwrite with any pattern. |
| **DoD 5220.22-M** | 3 | 7 | Fixed sequence | US DoD 5220.22-M. Fixed 3-pass: zeros, ones, random. Passes > 3 repeat. |
| **Random Only** | 1 | 35 | Random | Fastest. Cryptographically secure random data only (ChaCha20). |

### Verification Levels

- **None** — No read-back verification (fastest)
- **Sample** — Checks start, middle, end blocks (recommended default)
- **Full** — Every byte verified (slowest, maximum assurance)

## Install

### Download Prebuilt Binaries

KnockKnock is a **portable app** — no installer required. Download the latest release for your platform from the [Releases page](https://github.com/knockknockshredder/knockknock/releases/latest):

| Platform | File | Run |
|----------|------|-----|
| **Windows** | `KnockKnock-windows-x64.exe` | Place in any writable folder, double-click |
| **macOS (Apple Silicon)** | `KnockKnock-macos-arm64.dmg` | Open `.dmg`, drag `KnockKnock.app` to any writable folder, right-click → Open |
| **Linux (any distro)** | `KnockKnock-linux-x64.AppImage` | `chmod +x` and run |

All app data is stored in a `KnockKnock-data/` folder next to the app. Delete the folder to remove all traces.

### Windows

Run the `.exe` directly — no installation needed. Windows SmartScreen may show a warning; click **More info** → **Run anyway** (the app is unsigned for now).

### macOS

1. Open the `.dmg` file
2. Drag `KnockKnock.app` to any writable folder (Desktop, Downloads, Applications — all work)
3. On first launch, right-click → Open (macOS Gatekeeper warning for unsigned apps)

### Linux

```bash
chmod +x KnockKnock-linux-x64.AppImage
./KnockKnock-linux-x64.AppImage
```

## Usage

1. **Launch KnockKnock** — the main window appears with a file drop zone
2. **Add files** — drag and drop files/folders, or click to browse (symlinks are rejected for safety)
3. **Select algorithm** — NIST 800-88 Clear (default), DoD 5220.22-M, or Random Only
4. **Configure** — Choose pattern, passes, and verification level
5. **Confirm** — Review the file list, then click **Shred**
6. **Done** — Files are overwritten, verified, renamed, truncated, and deleted

### Browser Cleanup

1. KnockKnock auto-detects installed browsers
2. Select which browser profiles and data types (cache, cookies, history, passwords, extensions) to clean
3. Click **Shred** — data is securely wiped with the same shredding algorithm
4. **Lock file detection** — warns if a browser is running and requires explicit confirmation

### PIN Protection

Enable PIN protection in **Settings**. The PIN is hashed with bcrypt and stored locally. After 3 failed attempts, PIN entry is locked for 5 minutes (persists across app restarts — relaunching doesn't reset the counter).

### Encrypted Vault (PIN Feature)

When PIN protection is enabled, your pending shred list is encrypted with AES-256-GCM (PBKDF2-SHA256 key derivation, 1M iterations) and saved to disk. Unlock with your PIN on next launch to restore your session.

### System Tray

Minimize to system tray for quick access. Right-click the tray icon for options, including quick-shred from clipboard.

## How It Works

Every shred operation follows this exact pipeline:

1. **Validate** — Confirms path exists, rejects system files, network drives, symlinks, empty paths
2. **Hard link check** — Warns if file has multiple hard links
3. **Detect media** — SSD vs HDD (different strategies)
4. **Overwrite** — Algorithm-driven passes with selected pattern
5. **Verify** — Read-back confirmation (none / sample / full)
6. **Rename** — Random filename to obliterate directory entry
7. **Truncate** — Sets file size to 0
8. **TRIM (SSDs)** — Issues ATA TRIM before deletion
9. **Delete** — Removes file entry
10. **Journal** — Records orphaned files for crash recovery
11. **Report** — Returns success/failure with per-file details

### Cancellation Safety

Cancelling mid-shred still runs the cleanup pipeline (rename → truncate → delete). A partially overwritten file never remains under its original name on disk.

### SSD Limitations

SSDs use wear leveling — the drive controller maps logical blocks to different physical cells. Multi-pass shredding is largely ineffective because old data may persist in cells you can't reach. KnockKnock performs single-pass + TRIM on SSDs, but **full-disk encryption (BitLocker/FileVault/LUKS) is the only reliable SSD protection.**

### Journaling Filesystems

NTFS, APFS, and ext4 journals may retain traces of file metadata. KnockKnock shreds file data in-place but cannot erase filesystem journals from user space. For maximum security, use full-disk encryption.

## Tech Stack

- **Backend:** Rust (Tauri 2.x)
- **Frontend:** React 19 + TypeScript + Tailwind CSS 4
- **Cryptography:** AES-256-GCM (aes-gcm), PBKDF2-SHA256 (pbkdf2 + sha2), ChaCha20 stream cipher (chacha20), bcrypt
- **Shredding:** Platform-native I/O (Windows: `FILE_FLAG_WRITE_THROUGH`, macOS: `fcntl(F_NOCACHE)`, Linux: `O_DIRECT`)
- **Binary size:** ~8–12 MB

## Project Structure

```
KnockKnock/
├── src-tauri/               # Rust backend
│   ├── src/
│   │   ├── main.rs           # Thin passthrough → lib.rs
│   │   ├── lib.rs            # Tauri app setup, plugin registration
│   │   ├── shredder/         # File shredding engine
│   │   │   ├── algorithms/   # Shredding algorithm implementations
│   │   │   │   ├── nist_clear.rs    # NIST 800-88 Clear
│   │   │   │   ├── dod_522022m.rs   # DoD 5220.22-M (3-pass)
│   │   │   │   ├── random_only.rs   # Random Only (fastest)
│   │   │   │   └── common.rs        # Shared write_pass buffer logic
│   │   │   ├── platform/     # OS-specific I/O (Win/macOS/Linux)
│   │   │   ├── verification.rs      # None / Sample / Full verification
│   │   │   ├── validation.rs        # Path validation, system file protection
│   │   │   ├── journal.rs           # Orphan crash recovery
│   │   │   ├── cancel.rs            # Global cancellation token
│   │   │   ├── progress.rs          # Tauri event-based progress reporting
│   │   │   ├── logging.rs           # Log obfuscation (numbered/partial mask)
│   │   │   └── errors.rs            # Typed shred errors
│   │   ├── browser/          # Browser detection + cleanup
│   │   │   ├── detection.rs  # Running process detection
│   │   │   ├── paths.rs      # Browser path definitions per OS
│   │   │   └── types.rs      # Browser, profile, data type types
│   │   ├── drive/            # Drive type detection (SSD/HDD/Network/USB)
│   │   ├── pin/              # PIN protection (bcrypt + lockout)
│   │   ├── vault/            # Encrypted session persistence (AES-256-GCM)
│   │   ├── tray/             # System tray
│   │   └── commands/         # Tauri IPC command handlers
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                     # React frontend
│   ├── components/           # UI components
│   │   ├── shred/            # Shred-specific: FileDropZone, AlgorithmSelector, etc.
│   │   ├── browser/          # Browser cards, profile items, warnings
│   │   ├── settings/         # PIN setup/verify, toggle settings, elevation
│   │   ├── layout/           # AppShell, sidebars, title bar, operation log
│   │   └── ui/               # shadcn/ui primitives
│   ├── contexts/             # Shred, Browser, Settings, Navigation contexts
│   ├── hooks/                # useBrowserDetection
│   ├── sections/             # ShredSection, SettingsSection
│   └── types/                # TypeScript type definitions
├── package.json
└── README.md
```

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- Platform-specific dependencies:
  - **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++ workload)
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`)
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-4-dev`

### Setup

```bash
git clone https://github.com/knockknockshredder/knockknock.git
cd knockknock
pnpm install
```

### Commands

```bash
pnpm dev            # Start Vite dev server (frontend only)
pnpm tauri dev      # Run desktop app in dev mode (frontend + Rust hot reload)
pnpm build          # Build frontend only
pnpm tauri build    # Build desktop app for distribution
pnpm test           # Run Rust tests + Vitest
pnpm lint           # ESLint + clippy
```

## Architecture

### Shredding Pipeline

The shredding engine is algorithm-agnostic via the `ShredAlgorithm` trait. Each algorithm defines its passes, accepted patterns, and whether it uses a fixed pattern sequence (like DoD 5220.22-M). Verification is pluggable via `VerificationStrategy`. Platform I/O is abstracted behind `PlatformIo` — each OS implements `open_for_shred`, `sync_to_disk`, `rename_random`, `truncate_to_zero`, `delete`, `detect_media_type`, `issue_trim`, and `find_locking_processes`.

### IPC Flow

```
React (invoke) → Tauri command (async) → spawn_blocking → shred pipeline
                                       → Tauri events (emit) → React (listen)
```

Progress events (`shred-progress`) are emitted per-file with speed and ETA, throttled to 100ms to avoid overwhelming the frontend.

### Safety Design

- System file paths are hardcoded and checked via canonical path matching
- The app's own binary directory is protected
- Symlinks are always rejected
- Network drives (UNC paths + mapped drives on Windows, NFS/CIFS on Linux) are refused
- Cancellation preserves the cleanup pipeline (rename → truncate → delete)
- Orphan journal records renamed files before deletion so crashed shreds can be recovered
- PIN lockout state is persisted to disk — relaunching the app does not reset it
- Vault encryption uses AEAD (AES-256-GCM) — tampered or wrong-PIN decryption fails with an authentication error

## Support

If KnockKnock saved you time or protected your privacy, consider supporting the project:

- **Star the repo** — free and helps with visibility
- **Report bugs** — [open an issue](https://github.com/knockknockshredder/knockknock/issues)
- **Contribute** — see [CONTRIBUTING.md](CONTRIBUTING.md)

## Legal Disclaimer

### Intended Use

This software is designed for **legitimate privacy and security purposes only**, including:

- Securely disposing of personal files before selling/donating computers
- Removing sensitive data from shared workstations
- Cleaning browser profiles for privacy protection
- Corporate data disposal compliance (GDPR, HIPAA, PCI-DSS)
- Journalist/activist source protection

### User Responsibility

**You are solely responsible for how you use this software.**

- The authors are not responsible for any data loss, legal consequences, or damages resulting from use of this software
- Ensure you have the legal right to delete the data you target
- Do not use this software to obstruct justice, destroy evidence, or for any illegal purpose
- Comply with all applicable local, state, national, and international laws

### No Warranty

This software is provided "AS IS" without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and noninfringement. See [LICENSE](LICENSE) for full terms.

### Data Loss Warning

**Shredding is permanent and irreversible.** There is no undo. Once a file is shredded, it cannot be recovered by any means. Always double-check your selection before confirming.

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

[MIT License](LICENSE) — free forever for personal use.

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting.

---

**Website:** [knockknockapp.org](https://knockknockapp.org)
