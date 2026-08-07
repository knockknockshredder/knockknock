# KnockKnock Architecture

This document describes the current high-level architecture of KnockKnock.

It focuses on component boundaries, destructive-operation flow, persistence, and security-relevant design.

For security assumptions and limitations, see:

* [THREAT_MODEL.md](THREAT_MODEL.md)
* [SECURITY.md](SECURITY.md)
* [LIMITATIONS.md](LIMITATIONS.md)

## Overview

KnockKnock is a Tauri desktop application with:

* a React/TypeScript frontend;
* a Rust backend;
* platform-specific filesystem adapters;
* local PIN and encrypted target persistence;
* browser-profile detection and cleanup;
* drive/media detection;
* system-tray and notification integration.

The frontend handles presentation and user interaction.

The Rust backend is responsible for security-sensitive filesystem and persistence operations.

```text
┌─────────────────────────────────────────────┐
│                React frontend               │
│                                             │
│  Target selection   Browser selection       │
│  Overwrite options  PIN dialogs             │
│  Confirmation       Progress / logs         │
└───────────────────────┬─────────────────────┘
                        │
                     Tauri IPC
                        │
┌───────────────────────▼─────────────────────┐
│                  Rust backend               │
│                                             │
│  commands                                  │
│     │                                       │
│     ├── shredder / root execution           │
│     ├── browser                             │
│     ├── drive                               │
│     ├── PIN                                 │
│     ├── vault                               │
│     ├── settings                            │
│     └── tray / notifications                │
└───────────────────────┬─────────────────────┘
                        │
              OS / filesystem APIs
                        │
┌───────────────────────▼─────────────────────┐
│       Filesystem / storage / browser data   │
└─────────────────────────────────────────────┘
```

## Repository Layout

The primary source layout is:

```text
src/
├── components/
│   ├── browser/
│   ├── layout/
│   ├── settings/
│   ├── shred/
│   └── ui/
├── contexts/
├── hooks/
├── sections/
└── types/

src-tauri/
├── src/
│   ├── browser/
│   ├── commands/
│   ├── drive/
│   ├── notifications/
│   ├── paths.rs
│   ├── pin/
│   ├── shredder/
│   ├── tray/
│   ├── vault/
│   ├── lib.rs
│   └── main.rs
├── Cargo.toml
└── tauri.conf.json
```

## Frontend

The frontend is built with:

* React;
* TypeScript;
* Tailwind CSS;
* Tauri JavaScript APIs.

Its responsibilities include:

* collecting user intent;
* displaying selected targets;
* presenting browser profiles;
* configuring overwrite behavior;
* requesting destructive confirmation;
* requesting PIN authorization;
* displaying progress and errors;
* managing frontend state.

The frontend should not be considered the final security boundary for filesystem operations.

Security-sensitive checks must also exist in the Rust backend.

## Frontend State

The current frontend separates major state areas into contexts.

### Shred state

Tracks information including:

* selected file/folder targets;
* algorithm selection;
* execution state;
* operation logs;
* progress;
* persisted-target state;
* active vault PIN state.

### Browser state

Tracks:

* detected browsers;
* detected profiles;
* profile selection;
* browser scan state.

### Settings state

Tracks user-configurable application settings such as:

* operation-log behavior;
* path display/masking.

### Navigation state

Tracks application navigation and layout state.

## Tauri IPC

The frontend calls Rust through registered Tauri commands.

The backend currently exposes command groups covering areas including:

### Shredding

* execute selected roots;
* request cancellation;
* recover interrupted cleanup;
* return available overwrite modes;
* validate paths and persisted targets;
* obtain drive information;
* open Windows file/folder pickers;
* request elevation.

### Browsers

* detect installed browser profiles;
* process selected browser profile data.

### PIN

* configure a PIN;
* verify a PIN;
* enable/disable PIN protection;
* inspect lockout state;
* change a PIN;
* reset application protection.

### Persisted targets

* save target state;
* load target state;
* clear persisted target state;
* check whether persisted state exists.

### Settings

* load application settings;
* save application settings.

### Tray and notifications

* synchronize tray state;
* minimize to tray;
* send local notifications.

Security-sensitive operations must validate backend inputs rather than relying on frontend presentation state.

## Rust Backend Modules

The backend root registers these major modules:

```text
browser
commands
drive
notifications
paths
pin
shredder
tray
vault
```

### `commands`

Defines the Tauri-facing command layer.

Its role is to:

* accept typed IPC requests;
* invoke the appropriate backend subsystem;
* convert internal results/errors into frontend-facing results.

### `shredder`

Contains the core destructive filesystem engine.

Current submodules include:

```text
algorithms
cancel
errors
journal
logging
platform
progress
root_execution
traits
types
validation
verification
```

### `browser`

Contains browser detection, profile discovery, and browser-data cleanup support.

### `drive`

Handles storage/media information used by storage-aware behavior.

### `pin`

Handles:

* PIN configuration;
* PIN verification;
* interactive lockout;
* persisted lockout state.

### `vault`

Handles encrypted persistence of selected target information.

The vault contains target metadata.

It does not contain or encrypt the contents of the target files themselves.

### `paths`

Defines where KnockKnock-managed persistent application data is stored.

### `tray`

Provides system-tray integration and tray actions.

### `notifications`

Provides local application notifications.

## Target Persistence

KnockKnock currently uses a portable-storage design.

Application-managed persistent state is rooted at:

```text
KnockKnock-data/
```

next to the application.

The path resolver creates this directory and treats an unwritable application location as a startup error rather than silently falling back to another storage location.

The directory is used for application-managed persistent state including:

* PIN-related state;
* encrypted target persistence;
* cleanup journal;
* settings;
* WebView data.

This storage model does not imply that the operating system keeps no external records of KnockKnock's execution.

See [LIMITATIONS.md](LIMITATIONS.md).

## Application Startup

At startup, the Rust application:

1. resolves the portable application-data directory;
2. treats failure to obtain a writable data location as fatal;
3. restores persisted PIN lockout state;
4. prepares WebView data storage;
5. initializes the Tauri application;
6. registers tray integration;
7. registers IPC commands;
8. creates or configures the main window according to the platform.

Tray initialization is treated as non-essential and does not prevent startup if it fails.

## PIN and Unlock Flow

When a PIN is configured, the application gates access through PIN verification.

The PIN design includes:

* local PIN verification;
* persistent failed-attempt lockout state;
* use of the PIN in persisted-target encryption/key-derivation workflows;
* explicit rekey behavior when the PIN changes.

Frontend PIN dialogs are user-interface gates.

The Rust backend remains responsible for actual PIN validation.

## Persisted Target Writer

The frontend maintains an encrypted target-persistence workflow coordinated with the backend.

The persistence layer tracks revisions so that target changes can be written in a controlled order.

Important design goals include:

* avoiding interleaved target-list writes;
* avoiding stale writes after PIN changes;
* ensuring a pending target list is durably saved before destructive execution when persistence is active;
* failing the destructive operation if the required final persistence checkpoint fails.

## Destructive Root Execution

Current folder/root execution is designed around selected roots rather than blindly recursively following path strings.

The execution layer is responsible for:

* preserving the selected root identity;
* traversing within the selected root;
* rejecting unsafe redirection;
* collecting per-root results;
* preserving actionable failures.

Platform-specific filesystem adapters provide the native operations required to enforce the execution model.

## Single-File Deletion Pipeline

At a high level, a normal file operation performs:

```text
validate
   ↓
network-storage check
   ↓
hard-link check
   ↓
media detection
   ↓
open target
   ↓
overwrite pass(es)
   ↓
sync
   ↓
optional read-back verification
   ↓
close destructive write handle
   ↓
randomized rename
   ↓
journal pending cleanup
   ↓
truncate
   ↓
TRIM/deallocation handling where applicable
   ↓
delete
   ↓
clear cleanup journal entry
```

Exact platform behavior can differ.

See [LIMITATIONS.md](LIMITATIONS.md) for what this pipeline does and does not guarantee.

## Validation

Validation exists to reduce the risk that destructive execution reaches an unintended target.

Current validation concepts include:

* protected path checks;
* application-path protection;
* link/reparse handling;
* network-filesystem rejection;
* hard-link awareness;
* target existence/type validation.

Validation must be repeated at security-sensitive boundaries when a race between validation and execution could otherwise change target identity.

## Link and Path Safety

Link handling is a security-sensitive part of the architecture.

Folder/root execution uses platform-specific mechanisms intended to prevent links or reparse points from redirecting traversal outside the selected target tree.

The implementation should prefer:

* handle-relative execution;
* no-follow semantics;
* target identity checks;

over validate-by-string-then-reopen patterns.

## Platform I/O

The shredder abstracts operating-system-specific filesystem behavior behind platform adapters.

Responsibilities include operations such as:

* opening a target for destructive writes;
* synchronizing writes;
* randomized rename;
* truncation;
* deletion;
* media detection;
* TRIM/deallocation requests where supported.

Platform differences are intentionally isolated from higher-level algorithm selection.

## Overwrite Modes

Overwrite behavior is implemented behind a common algorithm abstraction.

Current modes include implementations historically named around:

* NIST Clear;
* DoD 5220.22-M;
* random-only overwrite.

These names describe implemented pass/pattern behavior.

They must not be interpreted as certification of whole-device sanitization.

User-facing descriptions should follow [LIMITATIONS.md](LIMITATIONS.md).

## Verification

Verification is separate from overwrite behavior.

Current verification levels are:

### None

No read-back check.

### Sample

Checks selected portions of the logical range, including the beginning, middle, and end.

### Full

Reads the complete logical overwritten range.

For random overwrite data, the implementation can reproduce the expected ChaCha20 stream for deterministic comparison.

Verification checks logical read-back correctness.

It is not proof of physical-media sanitization.

## Cancellation

Cancellation is coordinated through a shared cancellation token.

Cancellation stops additional overwrite work where possible.

A target that has already entered destructive processing may still proceed through cleanup.

The design intentionally does not present cancellation as data restoration.

## Cleanup Journal

The journal tracks destructive operations that have been renamed but not fully cleaned up.

A typical cleanup sequence is:

```text
rename
   ↓
persist journal entry
   ↓
truncate
   ↓
TRIM/deallocation handling
   ↓
delete
   ↓
remove journal entry
```

The journal exists so that interruption between rename and deletion does not silently leave an unresolved operation with no tracking state.

The journal is a destructive-workflow recovery mechanism.

It is not a deleted-file recovery system.

## Browser Cleanup

Browser support consists of two stages:

```text
browser detection
      ↓
profile discovery / selection
      ↓
explicit destructive confirmation
      ↓
backend browser-data processing
```

Running browsers are detected and surfaced because concurrent browser writes can interfere with cleanup or recreate local data.

Browser cleanup operates on local profile data and does not control remote browser-account synchronization.

## Drive Detection

The drive subsystem attempts to classify storage so KnockKnock can adapt warnings and behavior for different media.

Relevant categories include local magnetic and solid-state storage.

Storage classification is advisory and platform-dependent.

It does not provide proof about the physical storage device behind every virtual or layered filesystem.

## Progress and Errors

The backend emits progress information to the frontend during destructive operations.

Errors are intended to remain typed internally and actionable to the user.

A destructive operation must not be reported as successful merely because part of the pipeline completed.

Per-root results allow failed or unresolved roots to remain visible rather than disappearing from the target list as though deletion had succeeded.

## Privilege Elevation

KnockKnock normally executes with the current user's permissions.

On Windows, the application can request administrator elevation when an operation fails because of insufficient permissions.

Elevation is explicit.

It does not:

* release files locked by another process;
* bypass every operating-system protection;
* change unsupported storage into supported storage.

## Tray and Notifications

Tray support provides convenient access to application actions and local notifications.

Tray-triggered destructive operations should route through the same authorization and confirmation path as equivalent actions in the main UI rather than maintaining a separate destructive implementation.

## Network Boundary

The destructive pipeline is local.

Selected file contents and target paths are not intended to be sent to an external service as part of deletion.

Features that open external links or otherwise use ordinary desktop integration are separate from the destructive data path.

Any future network feature must be reviewed against:

* [SECURITY.md](SECURITY.md)
* [THREAT_MODEL.md](THREAT_MODEL.md)

## Security-Critical Invariants

The architecture should preserve these invariants:

1. The backend, not the frontend alone, validates destructive targets.
2. Link handling must not redirect deletion outside the selected target.
3. Failed destructive operations remain visible as failures.
4. Cancellation is never treated as Undo.
5. The encrypted target store never represents itself as encryption of target-file contents.
6. PIN lockout is not represented as protection against all offline attacks.
7. Storage-aware behavior does not become a claim of physical-erasure certainty.
8. Browser cleanup remains limited to selected local browser data.
9. Elevation remains explicit and least-privilege by default.
10. Destructive execution must not gain a hidden, remote, or unattended alternate path.

## Build and Test Architecture

Frontend tooling is driven by `package.json`.

Primary commands include:

```bash
pnpm build
pnpm test
pnpm lint
pnpm tauri build
```

The test suite currently includes:

* Rust tests;
* Vitest frontend/component tests.

Destructive tests should use disposable temporary data.

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Release Architecture

GitHub Actions builds release artifacts for supported platforms and produces SHA-256 checksums.

Current release packaging includes platform-specific artifacts for:

* Windows x64;
* macOS Apple Silicon;
* Linux x64 AppImage.

Code-signing and notarization status should be documented separately from source-level security and must never be implied when not present.

## Updating This Document

Update `ARCHITECTURE.md` whenever a change materially alters:

* module boundaries;
* target execution;
* path validation;
* persistence;
* PIN behavior;
* browser cleanup;
* cancellation;
* journaling;
* platform storage handling;
* privilege boundaries;
* network behavior;
* release trust model.
