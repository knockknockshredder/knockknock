// src/components/shred/FileList.tsx
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useShred } from "@/contexts/ShredContext";
import { DriveGroupHeader } from "./DriveGroupHeader";
import { FileListItem } from "./FileListItem";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { DriveInfo, ShredFile } from "@/types";

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
function getDriveKey(path: string): string {
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

interface Group {
  key: string;
  files: ShredFile[];
  drive: DriveInfo | undefined;
}

export function FileList() {
  const { files } = useShred();
  const scrollRef = useRef<HTMLDivElement>(null);

  const [driveInfos, setDriveInfos] = useState<Map<string, DriveInfo>>(
    () => new Map(),
  );
  const [collapsedKeys, setCollapsedKeys] = useState<Set<string>>(
    () => new Set(),
  );

  // Fetch drive info whenever the set of distinct drive keys changes.
  // We deliberately depend on the key set (not the full files array)
  // to avoid a fresh round-trip on every status update mid-shred.
  const distinctKeys = useMemo(() => {
    const keys = new Set<string>();
    for (const f of files) keys.add(getDriveKey(f.path));
    return Array.from(keys).sort();
  }, [files]);

  useEffect(() => {
    if (distinctKeys.length === 0) {
      setDriveInfos(new Map());
      return;
    }

    // Hand one representative path per key to the backend so it can
    // resolve each drive exactly once.
    const representativePath = new Map<string, string>();
    for (const f of files) {
      const key = getDriveKey(f.path);
      if (!representativePath.has(key)) {
        representativePath.set(key, f.path);
      }
    }
    const paths = distinctKeys.map(
      (k) => representativePath.get(k) ?? k,
    );

    let cancelled = false;
    invoke<DriveInfo[]>("get_all_drive_info", { paths })
      .then((infos) => {
        if (cancelled) return;
        const map = new Map<string, DriveInfo>();
        for (const info of infos) {
          map.set(info.drive_letter, info);
        }
        setDriveInfos(map);
      })
      .catch((err) => {
        // Drive info is purely cosmetic — fall back to key-only headers.
        console.warn("get_all_drive_info failed:", err);
        if (!cancelled) setDriveInfos(new Map());
      });

    return () => {
      cancelled = true;
    };
  }, [distinctKeys, files]);

  // Group files by drive key, preserving their original order within each
  // group (files are appended in selection order).
  const groups = useMemo<Group[]>(() => {
    const map = new Map<string, ShredFile[]>();
    for (const file of files) {
      const key = getDriveKey(file.path);
      const existing = map.get(key);
      if (existing) existing.push(file);
      else map.set(key, [file]);
    }
    return Array.from(map.entries())
      .map(([key, filesForKey]) => ({
        key,
        files: filesForKey,
        drive: driveInfos.get(key),
      }))
      .sort((a, b) => a.key.localeCompare(b.key));
  }, [files, driveInfos]);

  // Auto-scroll to bottom when the file count grows. Status updates
  // mid-shred must NOT yank the scroll position.
  useEffect(() => {
    if (scrollRef.current) {
      const viewport = scrollRef.current.querySelector(
        '[data-slot="scroll-area-viewport"]',
      ) as HTMLDivElement | null;
      if (viewport) {
        viewport.scrollTop = viewport.scrollHeight;
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [files.length]);

  if (files.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        No files selected
      </p>
    );
  }

  const toggleKey = (key: string) => {
    setCollapsedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <div ref={scrollRef} className="h-full">
      <ScrollArea className="h-full border border-border">
        {groups.map((group) => {
          const isCollapsed = collapsedKeys.has(group.key);
          return (
            <div key={group.key}>
              {group.drive ? (
                <DriveGroupHeader
                  drive={group.drive}
                  fileCount={group.files.length}
                  isCollapsed={isCollapsed}
                  onToggle={() => toggleKey(group.key)}
                />
              ) : (
                <FallbackHeader
                  driveKey={group.key}
                  fileCount={group.files.length}
                  isCollapsed={isCollapsed}
                  onToggle={() => toggleKey(group.key)}
                />
              )}
              {!isCollapsed &&
                group.files.map((file) => (
                  <FileListItem key={file.id} file={file} />
                ))}
            </div>
          );
        })}
      </ScrollArea>
    </div>
  );
}

/**
 * Minimal header used when the backend hasn't returned a DriveInfo yet
 * (or the IPC call failed). Keeps the grouping visible without blocking
 * the list on drive detection.
 */
function FallbackHeader({
  driveKey,
  fileCount,
  isCollapsed,
  onToggle,
}: {
  driveKey: string;
  fileCount: number;
  isCollapsed: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={!isCollapsed}
      className="flex w-full items-center gap-2 border-b border-border bg-elevated/40 px-4 py-2 text-left font-mono text-xs text-muted-foreground transition-colors hover:bg-elevated hover:text-foreground"
    >
      <span className="font-semibold text-foreground">{driveKey}</span>
      <span className="ml-auto text-muted-foreground">
        {fileCount} file{fileCount === 1 ? "" : "s"}
      </span>
      <span className="text-muted-foreground">{isCollapsed ? "▶" : "▼"}</span>
    </button>
  );
}