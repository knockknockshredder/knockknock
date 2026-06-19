// src/components/shred/ShredButton.tsx
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

interface ShredButtonProps {
  onClick: () => void;
}

export function ShredButton({ onClick }: ShredButtonProps) {
  const { files, isShredding } = useShred();
  const pendingFiles = files.filter((f) => f.status === "pending");
  const disabled = pendingFiles.length === 0 || isShredding;
  const totalSize = pendingFiles.reduce((sum, f) => sum + f.size, 0);

  return (
    <div className="flex flex-col items-center gap-2">
      <button
        onClick={onClick}
        disabled={disabled}
        className={cn(
          "w-full max-w-[400px] border-2 px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider transition-colors",
          disabled
            ? "cursor-not-allowed border-border text-muted-foreground opacity-40"
            : "border-destructive text-destructive hover:border-red-500 hover:bg-red-500 hover:text-background"
        )}
      >
        Shred Files
      </button>
      {pendingFiles.length > 0 && (
        <p className="font-mono text-xs text-muted-foreground">
          {pendingFiles.length} file{pendingFiles.length !== 1 ? "s" : ""}
          {totalSize > 0
            ? totalSize > 1073741824
              ? `, ${(totalSize / 1073741824).toFixed(2)} GB`
              : `, ${(totalSize / 1048576).toFixed(1)} MB`
            : ""}{" "}
          — this action is irreversible
        </p>
      )}
    </div>
  );
}
