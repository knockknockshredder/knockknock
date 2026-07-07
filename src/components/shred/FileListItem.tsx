// src/components/shred/FileListItem.tsx
import { X, CheckCircle, Spinner, WarningCircle } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import type { ShredFile } from "@/types";

function StatusIcon({ status }: { status: ShredFile["status"] }) {
  switch (status) {
    case "pending":
      return <span className="text-muted-foreground">—</span>;
    case "shredding":
      return <Spinner size={16} className="animate-spin text-accent" />;
    case "done":
      return <CheckCircle size={16} className="text-green-500" />;
    case "error":
      return <WarningCircle size={16} className="text-red-500" />;
  }
}

export function FileListItem({ file }: { file: ShredFile }) {
  const { removeFile } = useShred();

  return (
    <div className="flex items-center gap-3 border-b border-border bg-surface px-4 py-2 hover:bg-elevated">
      <StatusIcon status={file.status} />
      <div className="min-w-0 flex-1">
        <p className="truncate font-mono text-sm text-foreground">{file.name}</p>
        <div className="flex items-center gap-2">
          <p className="font-mono text-xs text-muted-foreground">
            {file.size > 0
              ? file.size > 1073741824
                ? `${(file.size / 1073741824).toFixed(2)} GB`
                : `${(file.size / 1048576).toFixed(1)} MB`
              : "—"}
          </p>
          {file.error && (
            <p className="truncate text-xs text-red-500">{file.error}</p>
          )}
        </div>
      </div>
      {file.status === "pending" && (
        <button
          onClick={() => removeFile(file.id)}
          aria-label={`Remove ${file.name}`}
          className="p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
}
