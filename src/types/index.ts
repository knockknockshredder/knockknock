// src/types/index.ts

export type Section = "shred" | "browser" | "settings";

export interface ShredFile {
  id: string;
  path: string;
  name: string;
  size: number;
  status: "pending" | "shredding" | "done" | "error";
  error?: string;
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

export interface ShredReport {
  total_files: number;
  successful: number;
  failed: number;
  skipped: number;
  errors: Array<{ path: string; error: string }>;
  total_bytes_shredded: number;
  duration_secs: number;
}
