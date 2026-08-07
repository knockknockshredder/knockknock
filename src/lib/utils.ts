import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Extract the platform-appropriate drive / mount key from a path.
 *
 * Windows: drive letter (`"C:"`, `"D:"`) or `"Network"` for UNC paths.
 * Unix: first path segment (`"/Users"`, `"/var"`), or `"/"` for the
 * filesystem root.
 *
 * This intentionally mirrors the keys produced by the Rust
 * `detect_drive_info` so the frontend can match a `DriveInfo` to the
 * correct group without a separate lookup table.
 */
export function getDriveKey(path: string): string {
  if (path.length >= 2 && path[1] === ":") {
    return path.substring(0, 2).toUpperCase();
  }
  if (path.startsWith("\\\\") || path.startsWith("//")) {
    return "Network";
  }
  if (path.startsWith("/")) {
    const parts = path.split("/").filter(Boolean);
    return parts.length > 0 ? `/${parts[0]}` : "/";
  }
  return "Unknown";
}

/**
 * Detect whether the app is running on Windows. Used to gate platform-
 * specific code paths (e.g. calling the custom `open_files_windows`
 * Tauri command instead of the cross-platform dialog plugin).
 */
export function isWindows(): boolean {
  return /Windows NT/i.test(navigator.userAgent)
}
