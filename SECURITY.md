# Security Policy

KnockKnock performs destructive local filesystem operations. Security issues that could cause unintended deletion, unauthorized access to persisted target data, or execution against the wrong target are treated as high priority.

For the project's security assumptions and technical limits, also see:

* [THREAT_MODEL.md](THREAT_MODEL.md)
* [LIMITATIONS.md](LIMITATIONS.md)
* [ARCHITECTURE.md](ARCHITECTURE.md)

## Reporting a Vulnerability

**Do not open a public GitHub issue for a suspected security vulnerability.**

Report security issues by email:

**[security@knockknockapp.org](mailto:security@knockknockapp.org)**

Please include, where possible:

* a clear description of the vulnerability;
* affected KnockKnock version;
* operating system and filesystem;
* steps to reproduce using disposable test data;
* expected behavior;
* actual behavior;
* potential impact;
* relevant logs or screenshots with sensitive paths removed;
* a suggested fix, if you have one.

Please avoid testing vulnerabilities against data you cannot afford to lose.

We will review reports as soon as practical. This project does not currently provide a guaranteed response-time or remediation SLA.

## Supported Versions

| Version        | Security support       |
| -------------- | ---------------------- |
| Latest release | Supported              |
| Older releases | Not actively supported |

Users should reproduce security reports against the latest release or current `main` where practical.

## In Scope

Examples of security issues that are in scope include:

### Unintended target deletion

* path traversal;
* incorrect path canonicalization;
* symlink, junction, reparse-point, or shortcut handling that redirects deletion;
* TOCTOU issues that allow a validated target to be replaced with another target;
* directory traversal escaping a selected root;
* deletion of a target other than the one explicitly selected.

### Authorization and persisted target state

* bypassing required PIN gates;
* unauthorized loading or modification of the persisted target list;
* cryptographic integrity failures in persisted target data;
* incorrect PIN-change or rekey behavior that exposes or corrupts persisted targets;
* bypassing configured lockout behavior.

### Privilege boundaries

* unintended privilege escalation;
* elevated execution affecting targets beyond the user's selected scope;
* unsafe handling of privileged filesystem operations.

### Information disclosure

* leakage of sensitive target paths where masking is expected;
* sensitive data included unexpectedly in logs, errors, or crash artifacts controlled by KnockKnock;
* exposure of persisted target metadata or cryptographic material.

### Browser cleanup safety

* incorrect browser/profile detection causing the wrong profile to be targeted;
* cleanup escaping the selected browser profile;
* incorrect running-browser handling that causes unintended deletion outside the chosen scope.

### Release and dependency security

* compromised release artifacts;
* dependency or build-chain issues that introduce unauthorized behavior;
* code execution caused by malicious or incorrectly trusted input.

## Generally Out of Scope

The following are generally not security vulnerabilities by themselves when KnockKnock behaves as documented:

* SSD wear leveling and controller remapping;
* inability to overwrite inaccessible physical flash cells;
* filesystem journal or metadata retention;
* snapshots created outside KnockKnock's control;
* backups or synchronized copies outside the selected local target;
* recovery from another device or cloud service;
* ordinary social engineering;
* limitations of a weak user-selected PIN against offline brute-force attempts;
* physical-device attacks that require an already-compromised operating system or storage controller;
* claims about whether deletion is lawful in a particular jurisdiction.

If KnockKnock behaves differently from its documented limitations, however, please report that inconsistency.

## Security Design Principles

### Explicit target selection

Destructive operations are limited to targets selected through KnockKnock's user-facing workflows.

### Review before execution

Users are given the opportunity to review selected targets before destructive execution.

### Fail closed

Authentication, persistence, target-validation, and destructive-operation failures should not silently fall back to less restrictive behavior.

### Fail visibly

Destructive failures should be surfaced to the user. Errors must not be silently treated as successful deletion.

### Path integrity

KnockKnock uses validation and platform-specific filesystem handling intended to prevent links, path substitution, or traversal from redirecting destructive operations.

### Least privilege

KnockKnock normally runs with the current user's privileges.

Elevation is requested explicitly when an operation fails because of insufficient permissions. Elevated privileges do not guarantee access to files held open or protected by other operating-system mechanisms.

### Local destructive pipeline

File and browser deletion operations do not require a remote service and are intended to operate on local targets.

KnockKnock must not transmit selected file contents or target paths to an external service as part of the deletion pipeline.

### No stealth or evasion

KnockKnock is not designed to hide itself, its process, or its destructive activity from the operating system.

### Authenticated persisted target state

Persisted target information is protected using authenticated encryption. Cryptographic integrity failures must be treated as errors.

### Crash-aware cleanup

Interrupted destructive operations are tracked so that unresolved cleanup work can be surfaced and handled deliberately.

Crash recovery in this context means recovery of the deletion workflow, not recovery of deleted user data.

## What KnockKnock Will Not Add

The project will not intentionally add:

* stealth mode;
* process-name hiding;
* process injection for concealment;
* features designed to evade forensic analysis;
* remote wipe capabilities;
* network-controlled destructive execution;
* unattended destructive triggers intended to activate without user review;
* backdoors;
* hidden telemetry;
* transmission of selected file contents to external servers;
* functionality specifically intended to conceal unlawful destruction of evidence.

## Responsible Disclosure

Please give maintainers a reasonable opportunity to investigate and prepare a fix before publicly disclosing a vulnerability.

When appropriate, a security fix may include:

* a patched release;
* updated documentation;
* regression tests;
* a security advisory;
* credit to the reporter, if requested.

Do not send sensitive or real-world private files as proof of concept. Reproduce issues using disposable data whenever possible.

## Security Is Not a Deletion Guarantee

Security controls inside KnockKnock do not make file-level deletion equivalent to complete storage-device sanitization.

Read [LIMITATIONS.md](LIMITATIONS.md) before relying on KnockKnock for sensitive data disposal.
