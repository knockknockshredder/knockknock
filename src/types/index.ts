// src/types/index.ts

export type Section = "home" | "settings";

export interface ShredFile {
  id: string;
  path: string;
  name: string;
  size: number;
  status: "pending" | "shredding" | "done" | "error";
  error?: string;
  kind: TargetKind;
  is_shortcut: boolean;
  shortcut_target: string | null;
  /** Per-root status from the last typed execution result (retained targets only). */
  root_status?: RootStatus;
  /** Aggregate write-check outcome from the last typed execution result (retained targets only). */
  write_check?: WriteCheckOutcome;
  /** Structured child errors from the last typed execution result (retained targets only). */
  child_errors?: ChildErrorDto[];
}

/**
 * Metadata returned by the Rust `validate_paths` command.
 * `kind` is the classification the shredder will see: `directory` for a
 * preserved folder root, `link` for real filesystem links, `file` for regular
 * files and `.lnk` shell shortcuts (file data). `is_shortcut` flags `.lnk`
 * shell shortcuts, NTFS symlinks, junctions, and Unix symlinks. `shortcut_target`
 * is the resolved target path when classification found one (null for normal
 * files and directories).
 */
export interface FileMetadata {
  path: string;
  name: string;
  size: number;
  kind: TargetKind;
  is_shortcut: boolean;
  shortcut_target: string | null;
}

export type LogLevel = "info" | "success" | "warning" | "error" | "command";

export interface LogEntry {
  id: string;
  timestamp: Date;
  level: LogLevel;
  message: string;
}

/**
 * Deletion method (v2 policy). Mirrors backend `DeletionMethod` snake_case
 * serialization in `src-tauri/src/shredder/types.rs`.
 */
export type DeletionMethod = "automatic" | "legacy_three_pass";

/**
 * Final-state write check mode (v2 policy). Mirrors backend `WriteCheck`
 * snake_case serialization in `src-tauri/src/shredder/types.rs`.
 */
export type WriteCheck = "off" | "spot" | "full";

/**
 * Outcome of the final-state write check. Mirrors backend
 * `WriteCheckOutcome` snake_case serialization.
 */
export type WriteCheckOutcome = "not_run" | "passed" | "failed";

/**
 * Per-file overwrite state. Mirrors backend `OverwriteState` snake_case
 * serialization (engine-internal; surfaced on the frontend only through
 * child error stages).
 */
export type OverwriteStatus = "not_started" | "partial" | "completed";

export interface DetectedBrowser {
  id: string;
  name: string;
  icon: string;
  isRunning: boolean;
  profiles: BrowserProfile[];
}

export interface BrowserProfile {
  id: string;
  name: string;
  path: string;
  size: number;
  selected: boolean;
}

/** Matches backend ShredStatus tagged enum serialization */
export type ShredStatus =
  | { type: "Shredding" }
  | { type: "Complete" }
  | { type: "Warning"; message: string }
  | { type: "Error"; message: string };

export interface ProgressEvent {
  file_path: string;
  file_size: number;
  bytes_written: number;
  current_pass: number;
  total_passes: number;
  speed_bytes_per_sec: number;
  estimated_time_remaining_secs: number;
  status: ShredStatus;
}

export interface ProgressState {
  current: number;
  total: number;
  percent: number;
  currentFile: string;
}

/** Matches backend LogObfuscation enum. Serialized to backend as snake_case string. */
export type LogObfuscation = "none" | "numbered" | "partial_mask";

export interface ShredReport {
  total_files: number;
  successful: number;
  failed: number;
  skipped: number;
  errors: Array<{ path: string; error: string }>;
  total_bytes_shredded: number;
  duration_secs: number;
}

/**
 * Drive classification. Mirrors backend `DriveType` snake_case
 * serialization in `src-tauri/src/drive/mod.rs`.
 */
export type DriveType =
  | "ssd"
  | "hdd"
  | "network"
  | "usb_ssd"
  | "usb_hdd"
  | "unknown";

/** Mirrors backend `DriveInfo` in `src-tauri/src/drive/mod.rs`. */
export interface DriveInfo {
  drive_letter: string;
  drive_type: DriveType;
  label: string;
  total_bytes: number;
  free_bytes: number;
}

export type TargetKind = "file" | "directory" | "link" | "unknown_legacy";

export type TargetAvailability = "ready" | "missing" | "blocked";

export type RootStatus = "destroyed" | "failed" | "cancelled" | "skipped";

export type ExecutionStage =
  | "preflight"
  | "overwrite"
  | "verify"
  | "rename"
  | "truncate"
  | "delete"
  | "directory_remove"
  | "journal"
  | "sync";

export type VaultSchemaSource = "v1" | "v2";

export interface VaultTarget {
  path: string;
  kind: TargetKind;
}

export interface TargetMetadataDto {
  path: string;
  kind: TargetKind;
  availability: TargetAvailability;
  reason: string | null;
  name: string;
  size: number;
}

export interface ExecuteRootRequest {
  target_id: string;
  path: string;
  kind: TargetKind;
}

export interface ExecuteRootsRequest {
  roots: ExecuteRootRequest[];
}

export interface ChildErrorDto {
  path: string;
  stage: ExecutionStage;
  error_type: string;
  message: string;
  actionable: string;
}

export interface RootResultDto {
  target_id: string;
  requested_path: string;
  kind: TargetKind;
  status: RootStatus;
  root_removed: boolean;
  files_destroyed: number;
  directories_removed: number;
  bytes_shredded: number;
  /** Aggregate final-state write check across the root's files. */
  write_check: WriteCheckOutcome;
  errors: ChildErrorDto[];
}

export interface BatchRootResult {
  roots: RootResultDto[];
}
