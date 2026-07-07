# KnockKnock

**Emergency file shredder with browser profile cleanup.**

> One button. Gone forever. Greet your guests.

[![Release](https://img.shields.io/github/v/release/knockknockshredder/knockknock)](https://github.com/knockknockshredder/knockknock/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)](#install)

## What It Does

KnockKnock securely deletes files, folders, and browser data using NIST 800-88 compliant single-pass random overwrite. Designed for speed and reliability when you need data gone _now_.

### Features

- **One-click shredding** — drag, drop, confirm, done
- **Browser profile cleanup** — auto-detects installed browsers, shreds profiles/history/cache/cookies
- **Cross-platform** — Windows, macOS, Linux
- **Fast** — single-pass overwrite (NIST 800-88 Clear)
- **SSD-aware** — detects drive type, applies TRIM after shredding
- **Verification** — read-back confirmation that data was actually overwritten
- **PIN protection** — optional PIN lock to prevent accidental shredding
- **System tray** — minimize to tray for quick access

### Supported Browsers

Chrome, Firefox, Edge, Safari, Brave, Opera, Vivaldi — auto-detected per platform.

## Install

### Download Prebuilt Binaries

Download the latest release for your platform from the [Releases page](https://github.com/knockknockshredder/knockknock/releases/latest):

| Platform | File |
|----------|------|
| **Windows** | `KnockKnock_<version>_x64-setup.exe` or `.msi` |
| **macOS (Apple Silicon)** | `KnockKnock_<version>_aarch64.dmg` |
| **macOS (Intel)** | `KnockKnock_<version>_x64.dmg` |
| **Linux (Debian/Ubuntu)** | `knockknock_<version>_amd64.deb` |
| **Linux (AppImage)** | `knockknock_<version>_amd64.AppImage` |

### Windows

Run the `.exe` installer. Windows SmartScreen may show a warning — click "More info" → "Run anyway" (the app is unsigned for now).

Alternatively, install via the `.msi` package for silent/unattended installation:

```powershell
msiexec /i KnockKnock_<version>_x64.msi /quiet
```

### macOS

1. Open the `.dmg` file
2. Drag KnockKnock to Applications
3. On first launch, right-click → Open (macOS Gatekeeper warning for unsigned apps)

For Apple Silicon (M1/M2/M3), use the `aarch64.dmg`. For Intel Macs, use the `x64.dmg`.

### Linux

**Debian/Ubuntu:**
```bash
sudo dpkg -i knockknock_<version>_amd64.deb
```

**AppImage (any distro):**
```bash
chmod +x knockknock_<version>_amd64.AppImage
./knockknock_<version>_amd64.AppImage
```

## Usage

1. **Launch KnockKnock** — the main window appears with a file drop zone
2. **Add files** — drag and drop files/folders onto the window, or click to browse
3. **Select algorithm** — NIST 800-88 Clear (single-pass random) is the default
4. **Confirm** — review the file list, then click **Shred**
5. **Done** — files are overwritten, renamed, truncated, and deleted

### Browser Cleanup

1. Switch to the **Browser** tab
2. KnockKnock auto-detects installed browsers
3. Select which browsers to clean
4. Click **Shred Browser Data** — profiles, cache, cookies, and history are securely wiped

### PIN Protection

Enable PIN protection in **Settings** to prevent accidental shredding. The PIN is hashed with bcrypt and stored locally — it never leaves your device.

### System Tray

Minimize KnockKnock to the system tray for quick access. Right-click the tray icon for options.

## How It Works

Every shred operation follows this exact sequence:

1. **Validate** — confirms path exists, checks for system files, network drives
2. **Detect media** — SSD vs HDD (different strategies)
3. **Overwrite** — single-pass random data (NIST 800-88 Clear)
4. **Verify** — reads back sample blocks (start/middle/end) to confirm overwrite
5. **Rename** — random filename to obliterate directory entry
6. **Truncate** — sets file size to 0
7. **Delete** — removes file entry

### SSD Limitations

SSDs use wear leveling — the drive controller maps logical blocks to different physical cells. Multi-pass shredding is largely ineffective because old data may persist in cells you can't reach. KnockKnock performs single-pass + TRIM on SSDs, but **full-disk encryption (BitLocker/FileVault/LUKS) is the only reliable SSD protection.**

### Journaling Filesystems

NTFS, APFS, and ext4 journals may retain traces of file metadata. KnockKnock shreds file data in-place but cannot erase filesystem journals from user space. For maximum security, use full-disk encryption.

## Tech Stack

- **Backend:** Rust (Tauri 2.x)
- **Frontend:** React 19 + TypeScript + Tailwind CSS 4
- **Shredding:** Platform-native APIs (Windows: `FILE_FLAG_WRITE_THROUGH`, macOS: `fcntl(F_NOCACHE)`, Linux: `O_DIRECT`)
- **Binary size:** ~8–12 MB

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
```

### Project Structure

```
KnockKnock/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs      # Thin passthrough → lib.rs
│   │   ├── lib.rs       # Tauri app setup, plugin registration
│   │   ├── shredder/    # File shredding engine
│   │   ├── browser/     # Browser detection + cleanup
│   │   ├── commands/    # Tauri IPC commands
│   │   ├── pin/         # PIN protection (bcrypt)
│   │   ├── tray/        # System tray
│   │   └── updater/     # Auto-update logic
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                 # React frontend
│   ├── components/      # UI components
│   ├── hooks/           # React hooks
│   ├── utils/           # Frontend utilities
│   ├── types/           # TypeScript types
│   └── App.tsx          # Main app + routes
├── package.json
└── README.md
```

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
