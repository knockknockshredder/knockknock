# KnockKnock Threat Model

This document describes what KnockKnock is designed to protect, what security properties it attempts to provide, and what is intentionally outside its scope.

It should be read together with:

* [SECURITY.md](SECURITY.md)
* [LIMITATIONS.md](LIMITATIONS.md)
* [ARCHITECTURE.md](ARCHITECTURE.md)

## Scope

KnockKnock is a local desktop utility for deliberate deletion of user-selected local data.

Its security model primarily concerns:

* selecting the correct target;
* avoiding unintended path redirection;
* performing the configured local overwrite/deletion workflow;
* preserving the integrity and confidentiality of the persisted target list;
* requiring configured user authorization;
* handling interruption and partial failure predictably.

KnockKnock is **not** a whole-device sanitization product.

## Protected Assets

The main assets considered by this threat model are:

### User-selected file and folder targets

KnockKnock should operate only on targets explicitly selected by the user and accepted by its validation/execution layers.

### Target identity

A path that was reviewed by the user must not silently become a different destructive target because of:

* symlink replacement;
* junction or reparse-point behavior;
* path traversal;
* directory-tree substitution;
* race conditions between validation and execution.

### Persisted target metadata

When persistence is enabled, KnockKnock stores selected target paths and target types for future sessions.

This information may itself be sensitive.

### PIN-related state

The locally configured PIN, verifier, lockout state, and cryptographic key-derivation inputs must be handled consistently with the application's authentication design.

### Application-controlled logs and settings

Target information should not be exposed through KnockKnock-controlled logging when the user has selected a masked logging mode.

## Security Goals

### G1 — Correct target execution

KnockKnock should delete only the file, folder, or local browser-profile data that the user selected and confirmed.

### G2 — Path-redirection resistance

Links and other filesystem mechanisms must not be able to redirect a destructive operation to an unintended target.

### G3 — Explicit destructive authorization

Destructive execution should require explicit user action and any configured PIN gate.

### G4 — Fail-closed behavior

If KnockKnock cannot safely determine or persist the state required for a destructive operation, it should stop rather than silently weaken the safety model.

### G5 — Visible failure

A failed destructive operation must not be reported as successful.

### G6 — Persisted target confidentiality and integrity

The saved target list should not be stored as ordinary readable plaintext when PIN-backed persistence is active, and modifications must be detected by authenticated encryption.

### G7 — Predictable interruption handling

Cancellation, process interruption, or partial cleanup must not silently create an apparently successful result.

### G8 — Local operation

The destructive filesystem pipeline should not depend on a remote service or transmit target file contents or paths to an external service.

## Non-Goals

KnockKnock does not claim to:

### Defeat every forensic recovery technique

KnockKnock cannot guarantee that every physical or historical copy of selected information becomes unrecoverable.

### Sanitize an entire storage device

File-level deletion is different from device-level media sanitization.

### Control SSD firmware

KnockKnock cannot directly control flash translation layers, wear leveling, spare blocks, over-provisioned cells, or storage-controller remapping.

### Issue storage-deallocation requests

KnockKnock performs no mount-wide TRIM/fstrim. Storage deallocation is left to the operating system and is independent of KnockKnock.

### Remove copies outside the selected local target

KnockKnock does not remove:

* backups it did not create;
* cloud copies;
* synchronized copies on other devices;
* remote browser-account data;
* snapshots outside its control.

### Remove every operating-system record

Operating systems and filesystems may retain:

* filesystem metadata;
* journal entries;
* indexing records;
* crash information;
* other system-managed records.

### Protect an already-compromised unlocked system

If an attacker already controls the operating system, user session, kernel, or KnockKnock process, this threat model no longer provides strong guarantees.

### Replace full-disk encryption

KnockKnock is not a substitute for BitLocker, FileVault, LUKS, or equivalent storage encryption.

### Provide stealth

KnockKnock does not attempt to conceal:

* its executable;
* its process;
* its UI;
* its tray presence;
* its filesystem activity.

### Provide legal or compliance guarantees

KnockKnock does not determine whether a deletion is lawful and does not itself establish compliance with GDPR, HIPAA, PCI DSS, or another regulatory framework.

## Adversary Classes

### A1 — Ordinary logical file recovery

An attacker attempts to recover a conventionally deleted file from accessible filesystem/storage space.

KnockKnock's overwrite pipeline is intended to reduce this form of recoverability on storage where logical overwrite maps meaningfully to the underlying data.

### A2 — Local user with access to KnockKnock application data

An attacker obtains files from `KnockKnock-data/` and attempts to inspect persisted target information or bypass application-level PIN protections.

Authenticated encryption and PIN-derived key material are relevant here, but PIN entropy remains an important limitation.

### A3 — Malicious filesystem state

An attacker or concurrent process attempts to replace, redirect, or mutate a selected target through filesystem links or race conditions.

KnockKnock's validation and platform-specific execution layers are intended to resist this class of attack.

### A4 — Process with control of an unlocked user session

A process already running with sufficient privileges may be able to:

* inspect application memory;
* read user-accessible targets;
* interfere with filesystem state;
* manipulate UI interactions;
* access unlocked application state.

KnockKnock does not claim strong isolation against this adversary.

### A5 — Physical or laboratory forensic access

An attacker has specialized hardware, firmware access, storage-level tooling, or forensic capabilities beyond normal filesystem APIs.

KnockKnock does not claim to defeat this adversary across all storage technologies.

## Trust Boundaries

### React frontend

The frontend presents:

* target selection;
* destructive confirmation;
* overwrite settings;
* browser selection;
* PIN interactions;
* status and errors.

It is not the final authority for filesystem safety.

### Tauri IPC boundary

Frontend requests cross into the Rust backend through registered Tauri commands.

Security-sensitive validation must not rely only on frontend state.

### Rust backend

The Rust backend is the authority for:

* path validation;
* destructive execution;
* drive classification;
* browser target operations;
* PIN verification;
* vault persistence;
* journal operations;
* settings persistence.

### Operating system and filesystem

KnockKnock relies on operating-system filesystem semantics and platform APIs.

The threat model assumes the kernel and filesystem implementation are not themselves malicious.

### Storage hardware

KnockKnock does not control internal storage-controller behavior.

This is particularly important for SSDs.

## Primary Threats and Mitigations

| Threat                                                  | Current mitigation                                                |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| Path points to an unsafe or protected target            | Backend path validation and protected-path checks                 |
| Link redirects deletion                                 | Link/reparse handling and no-follow execution safeguards          |
| Directory traversal escapes selected root               | Root-scoped platform execution                                    |
| Target changes between validation and deletion          | Identity checks and handle-relative execution in folder workflows |
| Network filesystem behaves unpredictably                | Detected network filesystems are rejected                         |
| Multiple hard links create ambiguous deletion semantics | Hard links are blocked at preflight and rechecked at execution  |
| Process interruption during cleanup                     | Durable cleanup journal tracks unresolved operations              |
| Authentication bypass                                   | PIN verification gates configured operations                      |
| Repeated interactive PIN guessing                       | Persistent lockout state                                          |
| Persisted target list disclosure/tampering              | Authenticated encryption with PIN-derived keys                    |
| Sensitive paths exposed in KnockKnock log UI            | User-selectable path masking                                      |
| Browser modified while being cleaned                    | Running-browser detection and warning                             |
| SSD overwrite interpreted as physical erasure           | SSD-specific warnings and documented limitations                  |

## PIN Model

The PIN is an application authorization mechanism and an input to persisted-target protection.

A numeric PIN has lower entropy than a strong random password.

Rate limiting and lockout help against interactive attempts through KnockKnock, but should not be interpreted as proof against unlimited offline guessing by an attacker who can copy local application data.

Users with a high-risk threat model should also rely on operating-system account security and full-disk encryption.

## Persisted Target Data

KnockKnock's encrypted target persistence protects target paths and related metadata.

It does **not** encrypt the contents of the target files.

The target files remain wherever the user originally stored them.

## Cancellation Model

Cancellation means:

> stop further destructive processing as soon as the implementation can do so safely.

Cancellation does **not** mean:

> restore data already overwritten or processed.

Cancellation is stop-after-current-file: the file in progress completes its destructive lifecycle and cleanup, and no further file or target is started.

## Crash Recovery Model

KnockKnock's journal exists to track unresolved destructive cleanup.

"Recovery" in this context means recovering the operation's state so cleanup can be handled consistently.

It does not mean reconstructing user data that has already been overwritten.

## Assumptions

This threat model assumes:

* the KnockKnock binary being executed is trusted;
* the operating system has not already been fully compromised;
* the user is authorized to operate on selected targets;
* filesystem APIs behave according to documented platform semantics;
* hardware may have behaviors not visible through logical filesystem APIs;
* the user reviews targets before confirming deletion.

## Updating This Document

Changes affecting any of the following should include a threat-model review:

* path validation;
* link/reparse handling;
* root execution;
* PIN behavior;
* vault encryption;
* browser cleanup;
* privilege elevation;
* cancellation;
* journal/recovery behavior;
* platform I/O;
* remote/network functionality.
