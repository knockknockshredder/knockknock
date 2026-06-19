# Security Policy

## Reporting Vulnerabilities

**Do NOT open a public GitHub issue for security vulnerabilities.**

Email: [INSERT EMAIL]

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

Response time: 48 hours for initial acknowledgment.

## Scope

### In Scope

- Remote code execution
- Shredding unintended files (path traversal, symlink attacks)
- Bypassing PIN protection
- Data leakage (logging sensitive paths, crash dumps containing file names)
- Privilege escalation
- Browser detection false positives (shredding wrong browser data)

### Out of Scope

- SSD wear-leveling limitations (documented, not a bug)
- Journaling filesystem metadata retention (documented limitation)
- Social engineering
- Physical access attacks

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Older releases | No |

## Security Design Principles

1. **Fail loud, never silent** — all errors surfaced to user
2. **Confirm before destroy** — every shred requires explicit user confirmation
3. **No stealth** — app is visible in taskbar, system tray, and process list
4. **No network** — shredding operations are 100% local, no data leaves the device
5. **Audit trail** — optional logging of all shred operations (user-controlled)
6. **Minimal permissions** — runs as normal user, only requests elevation when needed

## What We Will Never Do

- Add features designed to evade forensic analysis
- Implement "stealth mode" or process hiding
- Send any file paths or data to external servers
- Bundle telemetry or analytics
- Add backdoors or remote access capabilities
