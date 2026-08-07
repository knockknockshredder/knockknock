// src/components/shred/DeletionMethodSelector.tsx
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Question } from "@phosphor-icons/react";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from "@/components/ui/tooltip";
import { useShred } from "@/contexts/ShredContext";
import { cn, getDriveKey } from "@/lib/utils";
import type { DeletionMethod, DriveInfo } from "@/types";

const METHOD_OPTIONS: ReadonlyArray<{
  value: DeletionMethod;
  title: string;
  tag: string;
  description: string;
}> = [
  {
    value: "automatic",
    title: "Automatic",
    tag: "Recommended",
    description:
      "Storage-aware local deletion. Uses one logical overwrite pass before removal.",
  },
  {
    value: "legacy_three_pass",
    title: "Legacy 3-pass",
    tag: "Advanced",
    description:
      "Fixed zeros → ones → random sequence. Available only for confirmed magnetic HDD storage.",
  },
];

/**
 * Best-effort storage classification for the Legacy availability note.
 * Uses drive info already surfaced by the existing `get_all_drive_info`
 * command; the backend preflight remains authoritative (S7). Any
 * non-HDD or unclassified drive disables Legacy, mirroring the batch
 * rule that every volume must be confirmed magnetic HDD.
 */
type StorageState = "no-files" | "no-info" | "all-hdd" | "ssd-present" | "unknown";

const STORAGE_NOTES: Partial<Record<StorageState, string>> = {
  "all-hdd":
    "All selected targets are on magnetic HDD storage. The Legacy 3-pass method is available for this batch.",
  "ssd-present":
    "Selected targets include solid-state storage. Additional overwrite passes do not overcome SSD wear-leveling or block-remapping limitations.",
  unknown:
    "The storage type of some selected targets is unknown. The Legacy 3-pass method is unavailable unless every target is on confirmed magnetic HDD storage.",
};

export function DeletionMethodSelector() {
  const { files, deletionMethod, setDeletionMethod } = useShred();
  const [driveInfos, setDriveInfos] = useState<DriveInfo[]>([]);

  const pendingFiles = useMemo(
    () => files.filter((file) => file.status === "pending"),
    [files]
  );

  const driveKeys = useMemo(() => {
    const keys = new Set(pendingFiles.map((file) => getDriveKey(file.path)));
    return Array.from(keys).sort();
  }, [pendingFiles]);

  useEffect(() => {
    if (driveKeys.length === 0) {
      setDriveInfos([]);
      return;
    }
    const representativePath = new Map<string, string>();
    for (const file of pendingFiles) {
      const key = getDriveKey(file.path);
      if (!representativePath.has(key)) representativePath.set(key, file.path);
    }
    const paths = driveKeys.map((key) => representativePath.get(key) ?? key);

    let cancelled = false;
    invoke<DriveInfo[]>("get_all_drive_info", { paths })
      .then((infos) => {
        if (!cancelled) setDriveInfos(infos);
      })
      .catch(() => {
        if (!cancelled) setDriveInfos([]);
      });
    return () => {
      cancelled = true;
    };
  }, [driveKeys, pendingFiles]);

  const storageState = useMemo<StorageState>(() => {
    if (driveKeys.length === 0) return "no-files";
    if (driveInfos.length < driveKeys.length) return "no-info";
    const types = new Set(driveInfos.map((info) => info.drive_type));
    const hasSsd = types.has("ssd") || types.has("usb_ssd");
    const hasHdd = types.has("hdd") || types.has("usb_hdd");
    const hasUnknown = types.has("unknown") || types.has("network");
    if (hasSsd) return "ssd-present";
    if (hasUnknown) return "unknown";
    if (hasHdd) return "all-hdd";
    return "no-info";
  }, [driveInfos, driveKeys]);

  const legacyUnavailable =
    storageState === "ssd-present" || storageState === "unknown";
  const storageNote = STORAGE_NOTES[storageState] ?? null;

  return (
    <div className="flex flex-col gap-1.5 w-full">
      <div className="flex items-center gap-1.5">
        <span className="font-mono text-xs text-muted-foreground">
          Deletion Method
        </span>
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger
              render={<span className="inline-flex cursor-help" />}
            >
              <Question size={14} className="text-muted-foreground" />
            </TooltipTrigger>
            <TooltipContent>
              Controls how KnockKnock overwrites the selected logical file
              range before deletion. The Automatic method is storage-aware;
              the Legacy 3-pass method is available only on confirmed
              magnetic HDD storage.
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
      <div className="flex flex-col gap-2">
        {METHOD_OPTIONS.map((option) => {
          const selected = deletionMethod === option.value;
          const disabled =
            option.value === "legacy_three_pass" && legacyUnavailable;
          return (
            <button
              key={option.value}
              type="button"
              onClick={() => setDeletionMethod(option.value)}
              disabled={disabled}
              aria-pressed={selected}
              className={cn(
                "flex flex-col items-start gap-1 border px-3 py-2 text-left transition-colors",
                selected
                  ? "border-accent bg-accent/10"
                  : "border-border hover:bg-elevated",
                disabled && "cursor-not-allowed opacity-50 hover:bg-transparent"
              )}
            >
              <span className="flex items-center gap-2 font-mono text-xs font-semibold text-foreground">
                {selected && (
                  <Check size={12} weight="bold" className="text-accent" />
                )}
                {option.title}
                <span
                  className={cn(
                    "border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider",
                    option.value === "automatic"
                      ? "border-accent/40 bg-accent/10 text-accent"
                      : "border-border text-muted-foreground"
                  )}
                >
                  {option.tag}
                </span>
              </span>
              <span className="text-xs text-muted-foreground">
                {option.description}
              </span>
            </button>
          );
        })}
      </div>
      {storageNote && (
        <p className="font-mono text-xs text-muted-foreground">{storageNote}</p>
      )}
    </div>
  );
}
