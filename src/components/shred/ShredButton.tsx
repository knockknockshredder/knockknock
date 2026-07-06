// src/components/shred/ShredButton.tsx
import { cn } from "@/lib/utils";

interface ShredButtonProps {
  fileCount: number;
  profileCount: number;
  isShredding: boolean;
  onClick: () => void;
}

export function ShredButton({
  fileCount,
  profileCount,
  isShredding,
  onClick,
}: ShredButtonProps) {
  const hasFiles = fileCount > 0;
  const hasProfiles = profileCount > 0;
  const disabled = (!hasFiles && !hasProfiles) || isShredding;

  let label: string;
  if (hasFiles && hasProfiles) {
    label = `Shred Selected (${fileCount} file${fileCount !== 1 ? "s" : ""} + ${profileCount} profile${profileCount !== 1 ? "s" : ""})`;
  } else if (hasFiles) {
    label = `Shred Selected (${fileCount} file${fileCount !== 1 ? "s" : ""})`;
  } else if (hasProfiles) {
    label = `Clean Selected (${profileCount} profile${profileCount !== 1 ? "s" : ""})`;
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
      {(hasFiles || hasProfiles) && !isShredding && (
        <p className="font-mono text-xs text-muted-foreground">
          this action is irreversible
        </p>
      )}
    </div>
  );
}
