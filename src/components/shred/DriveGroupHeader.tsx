// src/components/shred/DriveGroupHeader.tsx
import {
  HardDrive,
  Usb,
  WifiHigh,
  Question,
  CaretDown,
  CaretRight,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";
import type { DriveInfo, DriveType } from "@/types";

interface DriveGroupHeaderProps {
  drive: DriveInfo;
  fileCount: number;
  isCollapsed: boolean;
  onToggle: () => void;
}

const DRIVE_TYPE_LABELS: Record<DriveType, string> = {
  ssd: "SSD",
  hdd: "HDD",
  network: "Network",
  usb_ssd: "USB (SSD)",
  usb_hdd: "USB (HDD)",
  unknown: "Unknown",
};

const DRIVE_TYPE_ICON: Record<DriveType, ReactNode> = {
  ssd: <HardDrive size={14} weight="duotone" />,
  hdd: <HardDrive size={14} weight="duotone" />,
  network: <WifiHigh size={14} weight="duotone" />,
  usb_ssd: <Usb size={14} weight="duotone" />,
  usb_hdd: <Usb size={14} weight="duotone" />,
  unknown: <Question size={14} weight="duotone" />,
};

/**
 * Collapsible header row shown above a group of files that share the
 * same drive / mount point.
 *
 * If the label or capacity is unavailable, the relevant cells fall back
 * to an em-dash rather than displaying zero or empty values.
 */
export function DriveGroupHeader({
  drive,
  fileCount,
  isCollapsed,
  onToggle,
}: DriveGroupHeaderProps) {
  const capacityLabel =
    drive.total_bytes > 0
      ? `${formatBytes(drive.free_bytes)} / ${formatBytes(drive.total_bytes)}`
      : null;

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={!isCollapsed}
      className="flex w-full items-center gap-2 border-b border-border bg-elevated/40 px-4 py-2 text-left font-mono text-xs text-muted-foreground transition-colors hover:bg-elevated hover:text-foreground"
    >
      {isCollapsed ? <CaretRight size={12} /> : <CaretDown size={12} />}
      {DRIVE_TYPE_ICON[drive.drive_type]}
      <span className="font-semibold text-foreground">{drive.drive_letter}</span>
      <span className="text-muted-foreground">
        ({DRIVE_TYPE_LABELS[drive.drive_type]})
      </span>
      {drive.label && drive.label !== "Local Disk" && (
        <span className="truncate text-muted-foreground">— {drive.label}</span>
      )}
      <span className="ml-auto text-muted-foreground">
        {fileCount} file{fileCount === 1 ? "" : "s"}
      </span>
      {capacityLabel && (
        <span className="hidden text-muted-foreground sm:inline">
          · {capacityLabel}
        </span>
      )}
    </button>
  );
}

/** Binary IEC formatter (KiB, MiB, GiB, TiB). */
function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${units[unitIndex]}`;
}