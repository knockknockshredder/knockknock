# KnockKnock

**Emergency file shredder with browser profile cleanup.**

> One button. Gone forever.

## What It Does

KnockKnock securely deletes files, folders, and browser data using NIST 800-88 compliant single-pass random overwrite. Designed for speed and reliability when you need data gone *now*.

### Features

- **One-click shredding** — drag, drop, confirm, done
- **Browser profile cleanup** — auto-detects installed browsers, shreds profiles/history/cache/cookies
- **Cross-platform** — Windows, macOS, Linux
- **Fast** — single-pass overwrite (NIST 800-88 Clear), not 35-pass Gutmann
- **SSD-aware** — detects drive type, applies TRIM after shredding
- **Verification** — read-back confirmation that data was actually overwritten
- **PIN protection** — optional PIN to prevent accidental shredding

### Supported Browsers

Chrome, Firefox, Edge, Safari, Brave, Opera, Vivaldi — auto-detected per platform.

## Install

*Coming soon.*

## Usage

*Coming soon.*

## How It Works

1. **Validate** — confirms path exists, checks for system files, network drives
2. **Detect media** — SSD vs HDD (different strategies)
3. **Overwrite** — single-pass random data (NIST 800-88 Clear)
4. **Verify** — reads back sample blocks to confirm overwrite
5. **Rename** — random filename to obliterate directory entry
6. **Truncate** — sets file size to 0
7. **Delete** — removes file entry

### SSD Limitations

SSDs use wear leveling — the drive controller maps logical blocks to different physical cells. Multi-pass shredding is largely ineffective because old data may persist in cells you can't reach. KnockKnock performs single-pass + TRIM on SSDs, but **full-disk encryption (BitLocker/FileVault/LUKS) is the only reliable SSD protection.**

### Journaling Filesystems

NTFS, APFS, and ext4 journals may retain traces of file metadata. KnockKnock shreds file data in-place but cannot erase filesystem journals from user space. For maximum security, use full-disk encryption.

## Tech Stack

- **Backend:** Rust (Tauri 2.x)
- **Frontend:** React + TypeScript + Tailwind CSS
- **Shredding:** Platform-native APIs (Windows: `FILE_FLAG_WRITE_THROUGH`, macOS: `fcntl(F_NOCACHE)`, Linux: `O_DIRECT`)
- **Binary size:** ~8–12 MB

## Development

```bash
pnpm dev          # Development server (Tauri + React hot reload)
pnpm build        # Production build
pnpm tauri dev    # Run Tauri desktop app in dev mode
pnpm tauri build  # Build desktop app for distribution
```

## Support

If KnockKnock saved you time or protected your privacy, consider supporting the project:

- **GitHub Sponsors** — [Sponsor](https://github.com/sponsors/YOUR_USERNAME)
- **Buy Me a Coffee** — [buymeacoffee.com/knockknock](https://buymeacoffee.com/knockknock)
- **Star the repo** — free and helps with visibility

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
