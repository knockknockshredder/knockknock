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

Do not interpret a KnockKnock deletion method as certification that an entire device has been sanitized.

> KnockKnock's design is informed by modern media-sanitization guidance, including NIST SP 800-88 Rev. 2, but KnockKnock performs file-level local deletion and does not claim whole-device sanitization certification or compliance.

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

KnockKnock performs no mount-wide TRIM/fstrim and issues no storage-deallocation requests.

Storage deallocation, where the operating system performs it, is independent of KnockKnock.

TRIM is a request through the storage stack indicating that logical blocks are no longer needed.

It does not provide proof that:

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

KnockKnock blocks browser cleanup while the browser is detected as running by its configured platform policy, with no override. If running state cannot be detected — for example a browser variant that holds no recognizable lock file — cleanup is blocked rather than proceeding on an uncertain state.

Close the browser before performing browser cleanup whenever possible.

KnockKnock's browser cleanup affects selected local data only. It does not represent deletion from an online account or another device.

Browser cleanup is confined to the selected profile: collection skips filesystem-link/reparse entries that it directly inspects, and a profile path that is itself a filesystem link is rejected. Inspection failures are surfaced rather than silently skipped. Collected candidates pass through the secure handle-relative root executor, which enforces component-level no-follow confinement before mutation. On Linux, profile paths are resolved from the home directory or from `XDG_CONFIG_HOME`; execution roots must be inside the home directory, so profile paths resolved from an `XDG_CONFIG_HOME` outside the home directory are blocked by root confinement.

## Hard Links

Multiple directory entries may refer to the same underlying file data.

KnockKnock blocks hard-linked targets: a file with more than one directory entry (link count greater than one) is refused at preflight, and the already-open handle is rechecked before any destructive write. There is no override.

Deleting one directory entry does not remove every other entry referring to the same underlying object, so shredding one name of a multi-link file would silently leave the data reachable through the sibling.

A residual race remains: another process could create an additional hard link after the final check but before deletion. No user-space application can fully close that window without filesystem cooperation.

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

## Write Check

KnockKnock's write check reads back data from the final logical file range after the last overwrite pass. It runs once, at the end of the overwrite.

### Off

No read-back after the overwrite.

### Spot

Checks the final overwrite at deterministic distributed locations; files up to 64 KiB are checked in full.

### Full

Reads back the complete final logical file range.

A successful write check means that KnockKnock could read back the expected data through the same logical storage interface after the overwrite. It checks the write result, not physical-media erasure.

It does **not** prove that:

* no remapped physical block exists;
* no snapshot exists;
* no backup exists;
* no synchronized copy exists;
* filesystem metadata contains no information;
* another device contains no copy.

## Cancellation

Cancellation is not Undo.

Cancellation is stop-after-current-file: the file currently being processed completes its destructive lifecycle — including its cleanup — and no further file or target is started. On large files, the stop takes effect only after the current file completes, so cancellation latency can be significant.

Once a target has begun destructive processing:

* some data may already have been overwritten;
* the file in progress finishes its destructive lifecycle (rename and deletion included);
* files not yet started are not touched.

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
