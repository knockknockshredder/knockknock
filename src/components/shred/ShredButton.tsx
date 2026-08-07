// src/components/shred/ShredButton.tsx
import { cn } from "@/lib/utils";
import type { ProgressState } from "@/types";

interface ShredButtonProps {
  fileCount: number;
  folderCount: number;
  profileCount: number;
  isShredding: boolean;
  onClick: () => void;
  onCancel?: () => void;
  progress?: ProgressState | null;
}

export function ShredButton({
  fileCount,
  folderCount,
  profileCount,
  isShredding,
  onClick,
  onCancel,
  progress,
}: ShredButtonProps) {
  const hasFiles = fileCount > 0;
  const hasFolders = folderCount > 0;
  const hasProfiles = profileCount > 0;
  const disabled =
    (!hasFiles && !hasFolders && !hasProfiles) || isShredding;

  if (isShredding) {
    return (
      <div className="flex flex-col items-center gap-2 w-full max-w-[400px]">
        <button
          type="button"
          onClick={onCancel}
          className="w-full border-2 border-amber-500 px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider text-amber-500 hover:bg-amber-500 hover:text-background transition-colors"
        >
          Stop Processing
        </button>
        <p className="font-mono text-xs text-muted-foreground">
          Already processed targets will not be restored.
        </p>
        {progress && (
          <div className="w-full bg-secondary rounded-full h-2">
            <div
              className="bg-amber-500 h-2 rounded-full transition-all duration-300"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
        )}
        {progress && (
          <p className="font-mono text-xs text-muted-foreground">
            {progress.current}/{progress.total} targets ({progress.percent}%)
          </p>
        )}
      </div>
    );
  }

  const countParts: string[] = [];
  if (hasFiles) {
    countParts.push(`${fileCount} file${fileCount !== 1 ? "s" : ""}`);
  }
  if (hasFolders) {
    countParts.push(`${folderCount} folder${folderCount !== 1 ? "s" : ""}`);
  }
  if (hasProfiles) {
    countParts.push(`${profileCount} profile${profileCount !== 1 ? "s" : ""}`);
  }

  let label: string;
  if (countParts.length > 0) {
    const action = hasFiles || hasFolders ? "Delete Selected" : "Clean Selected Browser Data";
    label = `${action} (${countParts.join(" + ")})`;
  } else {
    label = "Nothing to shred";
  }

  return (
    <div className="flex flex-col items-center gap-2">
      <button
        type="button"
        onClick={onClick}
        disabled={disabled}
        className={cn(
          "w-full max-w-[400px] border-2 px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider transition-colors",
          disabled
            ? "cursor-not-allowed border-border text-muted-foreground opacity-40"
            : "border-destructive text-destructive hover:border-red-500 hover:bg-red-500 hover:text-background"
        )}
      >
        {label}
      </button>
      {(hasFiles || hasFolders || hasProfiles) && !isShredding && (
        <p className="font-mono text-xs text-muted-foreground">
          KnockKnock has no Undo
        </p>
      )}
    </div>
  );
}
