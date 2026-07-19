import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Detect whether the app is running on Windows. Used to gate platform-
 * specific code paths (e.g. calling the custom `open_files_windows`
 * Tauri command instead of the cross-platform dialog plugin).
 */
export function isWindows(): boolean {
  return /Windows NT/i.test(navigator.userAgent)
}
