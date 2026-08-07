# Contributing to KnockKnock

Thanks for your interest in contributing to KnockKnock.

KnockKnock performs destructive filesystem operations, so changes that affect target selection, path handling, deletion, browser cleanup, persistence, authentication, or recovery require particular care.

Before making security-sensitive changes, also read:

* [SECURITY.md](SECURITY.md)
* [THREAT_MODEL.md](THREAT_MODEL.md)
* [LIMITATIONS.md](LIMITATIONS.md)
* [ARCHITECTURE.md](ARCHITECTURE.md)

## Prerequisites

The current project uses:

* Node.js 22 recommended to match CI
* pnpm 10.x
* Rust 1.86 or newer
* Tauri 2

Platform-specific build tools are also required.

### Windows

Install Visual Studio Build Tools with the C++ workload.

### macOS

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

### Linux

For Debian/Ubuntu-based development environments:

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-4-dev
```

## Setup

```bash
git clone https://github.com/knockknockshredder/knockknock.git
cd knockknock
pnpm install
pnpm tauri dev
```

## Development Workflow

1. Fork the repository.
2. Create a focused branch for the change.
3. Read the relevant implementation and tests before modifying behavior.
4. Make the smallest change that solves the problem.
5. Add or update tests where behavior changes.
6. Run the project checks.
7. Review any user-facing security or deletion claims for accuracy.
8. Commit with a clear message.
9. Push the branch and open a pull request.

Example:

```bash
git checkout -b fix/path-validation
```

## Required Checks

Before opening a pull request, run:

```bash
pnpm build
pnpm test
pnpm lint
```

The current scripts perform:

* `pnpm build` — TypeScript compilation and frontend production build
* `pnpm test` — Rust tests followed by Vitest frontend tests
* `pnpm lint` — TypeScript type checking with `tsc --noEmit`

For changes affecting native packaging or Tauri integration, also run:

```bash
pnpm tauri build
```

on the relevant platform where practical.

## Destructive-Code Testing

Never test deletion logic against important or irreplaceable data.

For shredding, cleanup, journal, recovery, path-validation, and filesystem tests:

* use temporary directories;
* create disposable test files;
* avoid real user directories;
* avoid system paths;
* avoid mounted network storage;
* make test targets uniquely identifiable;
* verify both the intended target and nearby non-target files after the test.

Tests for failure behavior are as important as success-path tests.

Where applicable, test:

* permission failures;
* locked files;
* symlinks and reparse points;
* hard links;
* interrupted operations;
* invalid or missing targets;
* cancellation;
* partial failures in batches;
* persistence failures;
* unexpected storage types.

## Rust Guidelines

* Rust edition 2021.
* Minimum supported Rust version is currently 1.86.
* Stable Rust only.
* Avoid `unsafe` unless platform integration genuinely requires it.
* Every `unsafe` block must have a clear reason and documented safety assumptions.
* Do not suppress errors from destructive operations.
* Surface actionable failures rather than silently falling back.
* Use typed errors where practical.
* Keep platform-specific behavior isolated in platform modules.
* Every exposed `#[tauri::command]` must be registered in the application's Tauri command handler.
* Zeroize sensitive key material where the existing design requires it.
* Preserve fail-closed behavior around authentication and persisted target state.

## TypeScript / React Guidelines

* Keep TypeScript strict.
* Do not use `as any`, `@ts-ignore`, or `@ts-expect-error` to bypass type problems.
* Use functional React components.
* Match the existing component and context architecture.
* Use Tailwind utilities for styling.
* Preserve accessible names for icon-only controls.
* Update component tests when user-visible behavior changes.

## Destructive UX Guidelines

A destructive action must never become easier to trigger accidentally as a side effect of an unrelated change.

Keep these principles:

* targets must be user-selected;
* the user must be able to review targets before deletion;
* destructive execution requires explicit confirmation;
* cancellation must not be presented as Undo;
* already processed data must not be represented as recoverable by KnockKnock;
* browser cleanup must clearly identify the profile being targeted;
* errors must remain visible and actionable.

Do not introduce:

* hidden destructive execution;
* remote wipe triggers;
* unattended destructive triggers;
* process hiding;
* stealth mode;
* automatic evidence-cleanup behavior;
* network-controlled deletion;
* mechanisms intended to conceal KnockKnock's operation.

## Security and Privacy Wording

User-facing documentation and UI must describe what the software actually does.

Avoid absolute claims such as:

* "gone forever";
* "unrecoverable by any means";
* "removes all traces";
* "forensically unrecoverable";
* "maximum assurance";
* "military-grade";
* universal "secure erase" guarantees.

Do not imply that:

* file-level overwrite equals whole-device sanitization;
* TRIM proves physical erasure;
* multiple passes solve SSD wear leveling;
* read-back verification proves that no other copy exists;
* KnockKnock by itself establishes GDPR, HIPAA, PCI DSS, or other compliance;
* use of AES, ChaCha20, bcrypt, PBKDF2, NIST terminology, or DoD terminology is itself proof of security.

When changing security-related copy, keep the wording consistent with:

* [THREAT_MODEL.md](THREAT_MODEL.md)
* [LIMITATIONS.md](LIMITATIONS.md)

## Scope of Pull Requests

Keep pull requests focused.

Avoid:

* unrelated refactoring;
* dependency upgrades unrelated to the change;
* formatting entire files without need;
* renaming public types during unrelated fixes;
* changing destructive behavior while presenting the PR as documentation-only.

If a behavior change is necessary, describe it explicitly in the pull request.

## Commit Messages

Use concise present-tense subjects.

Good:

```text
Fix path validation for directory roots
```

Avoid:

```text
Fixed some validation stuff
```

Keep the subject reasonably short and use the commit body for important implementation or security details.

## Reporting Bugs

For ordinary bugs, open a GitHub issue and include:

* KnockKnock version;
* operating system and version;
* storage/filesystem information when relevant;
* steps to reproduce;
* expected behavior;
* actual behavior;
* relevant error output;
* whether the issue can be reproduced using disposable test data.

Do not include sensitive real-world file paths unless necessary. Mask them where possible.

## Reporting Security Vulnerabilities

Do **not** open a public GitHub issue for a suspected security vulnerability.

Follow [SECURITY.md](SECURITY.md).

## Documentation Changes

Documentation is part of the security model.

If a pull request changes:

* deletion behavior;
* verification;
* SSD handling;
* path validation;
* browser cleanup;
* PIN behavior;
* encrypted target persistence;
* crash recovery;
* cancellation;
* privilege elevation;

review whether the following also need updating:

* `README.md`
* `SECURITY.md`
* `THREAT_MODEL.md`
* `LIMITATIONS.md`
* `ARCHITECTURE.md`
* relevant UI copy and tests

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
