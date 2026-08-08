# KnockKnock

[![Release](https://img.shields.io/github/v/release/knockknockshredder/knockknock)](https://github.com/knockknockshredder/knockknock/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)](#install)

**Open-source local data deletion utility for files, folders, and selected browser data.**

> Prepare sensitive local data for deliberate, controlled deletion.

KnockKnock is a cross-platform desktop application for overwriting and deleting user-selected local data. It supports storage-aware deletion methods, final-state write checks, persistent encrypted target lists, browser profile cleanup, and storage-aware handling for HDDs and SSDs.

---

## Table of Contents

- [Important Limitations](#important-limitations)
- [Features](#features)
- [Supported Browsers](#supported-browsers)
- [Deletion Methods](#deletion-methods)
- [Write Check](#write-check)
- [Install](#install)
- [Usage](#usage)
- [PIN Protection](#pin-protection)
- [Encrypted Target List](#encrypted-target-list)
- [System Tray](#system-tray)
- [How Deletion Works](#how-deletion-works)
- [Cancellation Behavior](#cancellation-behavior)
- [SSD Limitations](#ssd-limitations)
- [Journaling Filesystems](#journaling-filesystems)
- [Safety Design](#safety-design)
- [Tech Stack](#tech-stack)
- [Project Structure](#project-structure)
- [Development](#development)
- [Architecture](#architecture)
- [Responsible Use](#responsible-use)
- [Data Loss Warning](#data-loss-warning)
- [Security](#security)
- [Support](#support)
- [Contributing](#contributing)
- [License](#license)

---

## Important Limitations

KnockKnock performs **file-level deletion**, not whole-device sanitization.

This distinction matters:

- **SSDs use wear leveling and block remapping.** Software operating at the filesystem level cannot reliably overwrite every physical flash cell that may previously have contained a file.
- **Filesystem journals may retain metadata.** Filesystems such as NTFS, APFS, and ext4 may preserve metadata outside the file data directly controlled by KnockKnock.
- **KnockKnock performs no mount-wide TRIM/fstrim.** Storage deallocation is left to the operating system and is independent of KnockKnock. The drive controller ultimately manages the underlying flash.
- **KnockKnock has no Undo.** Once processing begins, already processed targets are not restored by cancelling the operation.

> **Recommended:** for sensitive data stored on modern SSDs, full-disk encryption such as BitLocker, FileVault, or LUKS provides substantially stronger protection than relying on file-level overwrite alone — particularly when encryption was enabled before the sensitive data was written.

---

## Features

- **Files and folders** — add individual files or directory targets for deletion.
- **Deletion methods** — Automatic (one logical overwrite pass) and Legacy 3-pass (HDD-only compatibility).
- **Write checks** — off, spot, or full final-state read-back after the overwrite.
- **Browser cleanup** — detects supported browser profiles and allows selected local browser data to be included.
- **Cross-platform** — Windows, macOS, and Linux.
- **Storage-aware behavior** — media detection gates the Legacy 3-pass method to confirmed magnetic HDD storage.
- **Encrypted target list** — persists pending targets encrypted with AES-256-GCM when PIN protection is enabled.
- **PIN protection** — locally stored bcrypt-hashed PIN with a persistent 3-attempt / 5-minute lockout.
- **System tray** — quick access to application actions without keeping the main window open.
- **Cancellation handling** — stops further overwrite work while preserving destructive cleanup semantics for already processed targets.
- **Crash recovery** — journal-based recovery for interrupted deletion operations.
- **Hard-link blocking** — targets with multiple hard links are refused.
- **Log path masking** — optional numbered or partially masked file paths in operation logs.
- **Real-time progress** — per-file progress, speed, and ETA.
- **Administrator elevation** — can request elevated privileges on Windows when an operation fails because of insufficient permissions.

---

## Supported Browsers

KnockKnock can detect supported installations and profiles for:

- Chrome, including Beta and Canary
- Firefox
- Edge, including Beta
- Brave, including Beta
- Opera, including Next
- Vivaldi
- Safari
- Tor Browser
- Chromium
- Internet Explorer

Browser cleanup can target selected local data types such as cache, cookies, history, passwords, and extensions.

> KnockKnock checks for browser lock files and warns when a browser appears to be running before destructive cleanup proceeds.

---

## Deletion Methods

KnockKnock offers two deletion methods. Both overwrite the file's data and then remove its filesystem entry.

| Method            | Passes | Behavior                                                                       |
| ----------------- | -----: | ------------------------------------------------------------------------------ |
| **Automatic**     |      1 | One pseudorandom logical overwrite pass before removal. Works on any local storage. |
| **Legacy 3-pass** |      3 | Fixed zeros → ones → random sequence. Available only for confirmed magnetic HDD storage. |

- **Automatic** is the default and recommended method. It performs one pseudorandom logical overwrite pass before removal and requires no media classification.
- **Legacy 3-pass** is a compatibility mode for confirmed magnetic HDD storage. It writes the fixed zeros → ones → random sequence, reflecting the historical DoD 5220.22-M three-pass practice. It is presented as a compatibility option only — not as a certification or as a claim of physical-erasure assurance. At preflight, the storage of every distinct volume in the batch is validated; if any volume is not confirmed as a magnetic HDD, the whole batch is refused before any data is changed.

KnockKnock exposes no arbitrary pass counts, pattern selection, or pass-repeat options. The pass sequence is fixed per method.

> KnockKnock's design is informed by modern media-sanitization guidance, including NIST SP 800-88 Rev. 2, but KnockKnock performs file-level local deletion and does not claim whole-device sanitization certification or compliance.

---

## Write Check

The write check reads back data from the final logical file range after the last overwrite pass. It checks the write result — that the expected data can be read back through the same logical storage interface.

- **Off** — skips read-back after the overwrite.
- **Spot** — checks the final overwrite at distributed locations. Small files are checked in full.
- **Full** — reads back the entire final logical file range.

> The write check runs once, after the last pass. It verifies the write result, not physical-media erasure. It does not prove that inaccessible physical blocks, filesystem metadata, or other copies of the data no longer exist.

---

## Install

### Download Prebuilt Binaries

Download the latest release from the [Releases page](https://github.com/knockknockshredder/knockknock/releases/latest).

| Platform                | Release file                    | Run                                                         |
| ----------------------- | ------------------------------- | ----------------------------------------------------------- |
| **Windows x64**         | `KnockKnock-windows-x64.exe`    | Place in a writable folder and run                          |
| **macOS Apple Silicon** | `KnockKnock-macos-arm64.dmg`    | Open the DMG and drag `KnockKnock.app` to a writable folder |
| **Linux x64**           | `KnockKnock-linux-x64.AppImage` | Make executable and run                                     |

> **Note:** current Windows and macOS builds are unsigned, so the operating system may display a security warning.

#### Windows

Run the `.exe` directly. No installer is required.

Windows SmartScreen may warn about the current unsigned build.

#### macOS

1. Open the `.dmg`.
2. Drag `KnockKnock.app` to a writable folder.
3. On first launch, macOS Gatekeeper may display a warning because the current build is unsigned.

#### Linux

```bash
chmod +x KnockKnock-linux-x64.AppImage
./KnockKnock-linux-x64.AppImage
```

---

## Usage

### Files and Folders

1. **Launch KnockKnock.**
2. **Add targets** by dragging files or folders into the application or using the file/folder picker.
3. **Review the target list.**
4. **Choose a deletion method.**
5. **Choose a write-check level (off, spot, or full) where applicable.**
6. **Confirm the destructive operation.**
7. KnockKnock processes each target and reports success or failure.

> Symlinks are rejected as a safety measure.

### Browser Cleanup

1. KnockKnock detects supported installed browsers and profiles.
2. Select the browser profile and local data types you want to process.
3. Review the selection before confirming.
4. KnockKnock warns if a browser appears to be running.
5. Selected browser data is processed using the same local deletion pipeline as other targets.

> Browser cleanup is destructive. Review selected profiles and data types carefully before confirming.

---

## PIN Protection

PIN protection can be enabled in **Settings**.

The PIN is hashed with bcrypt and stored locally. After three failed attempts, PIN entry is locked for five minutes. The lockout state persists across application restarts.

> The application lockout is intended to slow interactive guessing through KnockKnock. It should not be treated as a substitute for operating-system or full-disk encryption.

---

## Encrypted Target List

When PIN protection is enabled, KnockKnock can persist the pending target list between application sessions.

The target list is encrypted using:

- AES-256-GCM
- PBKDF2-SHA256 key derivation
- 1,000,000 PBKDF2 iterations

After unlocking KnockKnock with the configured PIN, persisted targets can be restored.

> The encrypted vault protects **KnockKnock's persisted target information**. It does not move the target files into an encrypted container or encrypt the contents of those files.

---

## System Tray

KnockKnock can remain available from the system tray for quick access to supported actions without keeping the main window open.

Tray functionality includes quick access to pending deletion operations and clipboard-related actions.

---

## How Deletion Works

A successful local file deletion generally follows this logical pipeline:

1. **Validate** — confirm the target exists and apply path-safety checks.
2. **Block hard links** — targets with multiple hard links are refused; the check is repeated against the already-open handle before any write.
3. **Classify storage** — for the Legacy 3-pass method only, confirm every distinct volume in the batch is a magnetic HDD before anything is changed.
4. **Open target** — open the file for destructive writes.
5. **Overwrite** — write the method's fixed pass sequence.
6. **Sync** — flush the overwritten data.
7. **Write check** — optionally read back the final logical range (spot or full).
8. **Journal** — record the pending rename for crash recovery.
9. **Rename** — replace the original filename with a randomized name.
10. **Sync parent** — flush the directory change.
11. **Delete** — remove the filesystem entry.
12. **Sync parent** — flush the directory change again.
13. **Clear journal** — remove the completed cleanup record.
14. **Report** — return structured per-target results.

Zero-length files skip the overwrite, sync, and write-check stages and proceed directly from journal to rename to deletion.

> Exact low-level behavior varies by platform, storage device, target type, and failure condition.

---

## Cancellation Behavior

Cancelling an active operation does **not** undo work that has already occurred.

KnockKnock stops at the next safe boundary: the file currently being processed completes its overwrite passes, optional final check, and destructive cleanup before Stop takes effect. No further file or target is started. On large files, cancellation can take until the active file completes.

> Do not use cancellation as a recovery mechanism.

---

## SSD Limitations

SSDs do not behave like magnetic hard drives.

Their controllers use wear leveling and logical-to-physical block remapping. As a result, rewriting a logical file does not guarantee that every physical flash cell previously associated with that file has been overwritten. Extra overwrite passes cannot fix this: the controller may place later writes in different physical locations.

KnockKnock performs no mount-wide TRIM/fstrim and issues no storage-deallocation requests. Storage deallocation, where the operating system performs it, is independent of KnockKnock.

Neither file-level overwrite nor operating-system deallocation provides a universal physical-erasure guarantee on SSD hardware.

> For stronger protection of sensitive data on SSDs, use full-disk encryption such as BitLocker, FileVault, or LUKS.

---

## Journaling Filesystems

Filesystems including NTFS, APFS, and ext4 may retain file metadata in journals or other filesystem structures outside the target file itself.

KnockKnock operates on the selected file data and filesystem entry. It cannot erase every filesystem-maintained record from normal application space.

---

## Safety Design

KnockKnock includes multiple safeguards around destructive filesystem operations:

- Path validation before processing.
- Protection for known critical system paths.
- Protection for the application's own binary directory.
- Rejection of symlinks.
- Rejection of detected network drives.
- Blocking of hard-linked targets (preflight plus open-handle recheck).
- HDD-only validation for the Legacy 3-pass method.
- Destructive cleanup handling after cancellation.
- Journal-based recovery of interrupted cleanup operations.
- Persistent PIN lockout state.
- Authenticated AES-256-GCM encryption for persisted target data.
- Explicit user confirmation before destructive operations.

> These protections reduce the risk of unintended deletion but do not replace careful review of the selected targets.

---

## Tech Stack

- **Backend:** Rust + Tauri 2.x
- **Frontend:** React 19 + TypeScript + Tailwind CSS 4
- **Vault encryption:** AES-256-GCM
- **Key derivation:** PBKDF2-SHA256
- **Random overwrite stream:** ChaCha20
- **PIN hashing:** bcrypt
- **File I/O:** platform-specific filesystem operations for Windows, macOS, and Linux

---

## Project Structure

```text
KnockKnock/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── shredder/
│   │   │   ├── engine.rs
│   │   │   ├── platform/
│   │   │   ├── verification.rs
│   │   │   ├── validation.rs
│   │   │   ├── journal.rs
│   │   │   ├── cancel.rs
│   │   │   ├── progress.rs
│   │   │   ├── logging.rs
│   │   │   ├── errors.rs
│   │   │   └── root_execution/
│   │   ├── browser/
│   │   ├── drive/
│   │   ├── pin/
│   │   ├── vault/
│   │   ├── tray/
│   │   └── commands/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── components/
│   ├── contexts/
│   ├── hooks/
│   ├── sections/
│   └── types/
├── package.json
└── README.md
```

---

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) v18+
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) stable toolchain

Platform-specific build dependencies are also required.

#### Windows

Visual Studio Build Tools with the C++ workload.

#### macOS

Xcode Command Line Tools:

```bash
xcode-select --install
```

#### Linux

For Debian/Ubuntu-based development environments:

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-4-dev
```

### Setup

```bash
git clone https://github.com/knockknockshredder/knockknock.git
cd knockknock
pnpm install
```

### Commands

```bash
pnpm dev
pnpm tauri dev
pnpm build
pnpm tauri build
pnpm test
pnpm lint
```

- `pnpm dev` — start the Vite frontend development server.
- `pnpm tauri dev` — run the desktop application in development mode.
- `pnpm build` — build the frontend.
- `pnpm tauri build` — build the desktop application.
- `pnpm test` — run the configured Rust and frontend test suites.
- `pnpm lint` — run the configured static/type checks.

---

## Architecture

The application separates the React frontend from the Rust/Tauri backend.

The shredding engine provides policy-driven deletion methods, final-state write checks, platform-specific I/O, validation, journaling, cancellation handling, and progress reporting.

Frontend commands are sent to the Rust backend through Tauri IPC, while progress updates are emitted back to the UI as application events.

```text
React UI
   │
   │ Tauri invoke
   ▼
Rust command layer
   │
   ▼
Shredding / browser / vault / drive modules
   │
   │ progress events
   ▼
React UI
```

---

## Responsible Use

KnockKnock is intended for legitimate privacy, security, and authorized data-disposal purposes.

Examples include:

- disposing of personal files before transferring or retiring storage;
- removing sensitive local data from shared workstations;
- clearing selected local browser data;
- supporting authorized organizational data-disposal workflows;
- protecting sensitive personal or professional information.

You are responsible for the targets you select and for complying with applicable law.

Do not use KnockKnock to:

- delete data you do not have authority to delete;
- intentionally destroy evidence unlawfully;
- obstruct justice;
- perform any other unlawful activity.

> The software does not itself establish compliance with GDPR, HIPAA, PCI DSS, or any other legal or industry framework.

---

## Data Loss Warning

> **KnockKnock performs destructive operations and has no Undo function.**

Once a target has been processed, KnockKnock cannot restore it.

File-level overwrite and deletion also do not constitute a guarantee that every physical remnant, filesystem record, or separate copy of the information has ceased to exist.

**Review every target carefully before confirming an operation.**

---

## Security

Security vulnerabilities should not be reported through public issues.

See [SECURITY.md](SECURITY.md) for the project's vulnerability reporting policy and security design principles.

---

## Support

If KnockKnock is useful to you:

- **Star the repository** — helps others discover the project.
- **Report bugs** — through [GitHub Issues](https://github.com/knockknockshredder/knockknock/issues).
- **Contribute** — by following [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Contributing

Contributions are welcome.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, development conventions, testing guidance, and vulnerability-reporting rules.

---

## License

KnockKnock is released under the [MIT License](LICENSE).

The MIT License permits personal and commercial use, modification, distribution, and sublicensing subject to the terms of the license.

---

**Website:** [knockknockapp.org](https://knockknockapp.org)
