# KnockKnock Limitations

KnockKnock performs deliberate **file-level local deletion**.

It does not provide a universal guarantee that every historical, physical, remote, synchronized, or system-managed copy of selected information is destroyed.

This document describes the most important technical limits.

## File-Level Deletion Is Not Media Sanitization

KnockKnock operates through operating-system and filesystem interfaces against selected files, folders, and supported local browser data.

It does not sanitize an entire disk.

A complete storage-device sanitization procedure operates at a different layer and may involve:

* device-native sanitize commands;
* cryptographic erase;
* whole-device overwrite where appropriate;
* destruction of encryption keys;
* physical destruction;
* organizational verification procedures.

Do not interpret a KnockKnock overwrite mode as certification that an entire device has been sanitized.

## HDDs

Traditional magnetic hard drives generally provide a more direct relationship between logical overwrites and physical sectors than modern flash storage.

KnockKnock can overwrite the selected logical file range before deleting its filesystem entry.

However, even on an HDD, this does not remove:

* copies in backups;
* snapshots;
* duplicate files;
* filesystem journals;
* remote copies;
* application-created copies;
* data stored outside the targeted allocation.

## SSDs and Flash Storage

SSDs use a flash translation layer controlled by the drive.

The controller may perform:

* wear leveling;
* garbage collection;
* block remapping;
* bad-block replacement;
* over-provisioning.

As a result, rewriting the logical blocks associated with a file does not prove that every physical flash cell that previously contained the data was overwritten.

### Multiple overwrite passes

Additional overwrite passes do not solve this problem.

The SSD controller may place later writes in different physical locations.

Multi-pass overwrite should therefore not be interpreted as increasing physical-erasure assurance on SSDs.

### TRIM / deallocation

Where supported, KnockKnock may request TRIM/deallocation as part of its SSD handling.

TRIM is a request through the storage stack indicating that logical blocks are no longer needed.

It does not provide KnockKnock with proof that:

* the flash was erased immediately;
* every historical copy disappeared;
* spare or remapped cells were cleared;
* a forensic laboratory cannot recover any remnant.

## Full-Disk Encryption

For modern SSDs, full-disk encryption enabled **before sensitive information is written** is generally stronger protection than relying on later file-level overwrite alone.

Examples include:

* BitLocker;
* FileVault;
* LUKS.

Full-disk encryption is not a solution to every problem.

It does not remove:

* cloud copies;
* backups;
* synchronized data;
* data already exported elsewhere.

It also provides much less protection when an attacker already controls an unlocked system and has access to active keys.

## Filesystem Journals

Filesystems may record metadata or transactional information outside the contents of the selected file.

Examples include filesystems such as:

* NTFS;
* APFS;
* ext4.

KnockKnock cannot guarantee deletion of every journal entry through ordinary user-space file operations.

## Copy-on-Write Filesystems

Copy-on-write behavior can cause older data blocks to remain after a logical write creates new blocks.

This makes conventional overwrite semantics less predictable.

KnockKnock does not guarantee that file-level overwrite reaches every historical copy created by copy-on-write behavior.

## Snapshots and Previous Versions

Snapshots may preserve an earlier version of a file even after the live filesystem entry has been deleted.

Examples can include:

* APFS snapshots;
* filesystem snapshots;
* Volume Shadow Copies;
* backup-system snapshots.

KnockKnock does not automatically locate or delete external snapshots.

## Backups

KnockKnock operates on selected local targets.

It does not automatically remove copies from:

* external backup drives;
* backup applications;
* NAS devices;
* cloud backups;
* system image backups;
* another computer.

## Cloud and Synchronization

Deleting a local file does not necessarily delete a synchronized or remote copy.

A synchronization client may also recreate local data after KnockKnock has deleted it.

This applies to file synchronization and browser synchronization.

Review the behavior of the relevant synchronization service separately.

## Browser Data

KnockKnock can target supported local browser profile data.

Browser storage is complex.

Data may also exist in:

* synchronized browser accounts;
* other devices;
* SQLite journals or WAL files;
* operating-system credential stores;
* application caches;
* browser-managed recovery/session data.

A running browser may recreate or rewrite profile data during cleanup.

Close the browser before performing browser cleanup whenever possible.

KnockKnock's browser cleanup affects selected local data only. It does not represent deletion from an online account or another device.

## Hard Links

Multiple directory entries may refer to the same underlying file data.

KnockKnock detects hard-link situations and warns the user.

Hard links complicate the meaning of path-level deletion because deleting one directory entry does not necessarily remove every other entry referring to the same underlying object.

Review hard-link warnings carefully.

## Symlinks, Junctions, Reparse Points, and Shortcuts

KnockKnock contains safeguards intended to stop filesystem links from redirecting destructive execution to an unintended target.

Supported link/reparse scenarios are rejected or handled according to the platform execution model.

No link-handling implementation should be treated as a reason to skip target review.

## Network Filesystems

KnockKnock refuses detected network filesystems in destructive execution paths.

Network storage can have:

* remote caching;
* snapshots;
* server-side copies;
* different locking semantics;
* different deletion guarantees.

Unknown or unusual storage drivers may not always classify cleanly as ordinary local HDD or SSD media.

## Virtual, Encrypted, and Layered Storage

Virtual disks, storage pools, RAID systems, encrypted containers, filesystem overlays, and other layered storage systems may hide the behavior of the underlying physical media.

Media-type detection describes what KnockKnock can determine from the operating system; it is not a hardware attestation.

## Verification

KnockKnock's verification modes read back data from the logical file range exposed by the operating system.

### None

No read-back verification.

### Sample

Checks selected portions of the logical file range.

### Full

Reads back the complete logical file range.

A successful verification means that KnockKnock could read back the expected data through the same logical storage interface.

It does **not** prove that:

* no remapped physical block exists;
* no snapshot exists;
* no backup exists;
* no synchronized copy exists;
* filesystem metadata contains no information;
* another device contains no copy.

## Cancellation

Cancellation is not Undo.

Once a target has begun destructive processing:

* some data may already have been overwritten;
* cleanup may continue;
* the target may still be renamed, truncated, and deleted.

KnockKnock cannot restore already processed data.

## Crash and Power Loss

KnockKnock uses journal-based tracking for interrupted cleanup operations.

This is intended to make unresolved operations visible and recoverable at the **workflow** level.

Unexpected conditions can still occur, including:

* power loss;
* filesystem corruption;
* hardware failure;
* process termination;
* storage removal.

No application can guarantee successful completion after every possible hardware failure.

## PIN Protection

PIN protection is an application access and authorization mechanism.

A numeric PIN has limited entropy.

The application's persistent lockout slows repeated interactive attempts through KnockKnock, but an attacker who obtains local application data may have a different offline attack model.

For stronger device-level protection, use:

* a strong operating-system login;
* full-disk encryption;
* secure physical access controls.

## Encrypted Target List

KnockKnock encrypts its persisted target list when PIN-backed persistence is active.

This protects target metadata managed by KnockKnock.

It does **not** encrypt the target files themselves.

## Application Data

KnockKnock stores its own persistent state under:

```text
KnockKnock-data/
```

next to the application according to the current portable-storage design.

This directory contains application-managed state such as:

* PIN-related state;
* persisted target data;
* journal state;
* settings;
* WebView data.

Deleting KnockKnock-managed application data should not be described as "removing all traces."

The operating system or other software may independently retain:

* filesystem metadata;
* execution history;
* crash information;
* indexing records;
* security logs;
* recently-used information;
* backup copies.

## Logs

KnockKnock supports path masking in its own operation log.

This controls KnockKnock's presentation of paths.

It cannot retroactively remove information that may have been recorded independently by:

* the operating system;
* filesystem;
* endpoint-security software;
* crash reporting;
* third-party tools.

## Administrator Elevation

Administrator privileges may resolve permission-related access failures.

Elevation does not:

* release a file held open by another process;
* bypass every operating-system protection;
* make an unsupported filesystem safe;
* guarantee successful deletion.

## Compliance

KnockKnock may be one component in an authorized data-retention or disposal process.

Using KnockKnock does not by itself establish compliance with:

* GDPR;
* HIPAA;
* PCI DSS;
* legal-hold requirements;
* records-retention rules;
* another regulatory framework.

Compliance depends on the full organizational and technical process.

## Security Audit Status

KnockKnock should not be represented as independently security-audited unless and until such an audit has actually occurred and its scope is documented.

Use of established cryptographic primitives is not equivalent to an independent security audit.

## No Universal Irrecoverability Claim

The most important limitation is simple:

> KnockKnock can perform deliberate local overwrite and deletion of selected filesystem targets. It cannot prove that every possible copy or physical remnant of the information has ceased to exist.

For security assumptions, see [THREAT_MODEL.md](THREAT_MODEL.md).
