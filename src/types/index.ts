// src/types/index.ts

export type Section = "home" | "settings";

export interface ShredFile {
  id: string;
  path: string;
  name: string;
  size: number;
  status: "pending" | "shredding" | "done" | "error";
  error?: string;
  is_shortcut: boolean;
  shortcut_target: string | null;
}

/**
 * Metadata returned by the Rust `validate_paths` command.
 * `is_shortcut` flags `.lnk` shell shortcuts, NTFS symlinks, junctions,
 * and Unix symlinks. `shortcut_target` is the resolved target path when
 * classification found one (null for normal files).
 */
export interface FileMetadata {
  path: string;
  name: string;
  size: number;
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

export interface AlgorithmOption {
  index: number;
  name: string;
  description: string;
  default_passes: number;
  max_passes: number;
  accepted_patterns: string[];
  has_fixed_pattern_sequence: boolean;
}

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
  | { type: "Verifying" }
  | { type: "Renaming" }
  | { type: "Truncating" }
  | { type: "Deleting" }
  | { type: "Complete" }
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
