// src/components/shred/FileListItem.tsx
import { useState } from "react";
import {
  X,
  CheckCircle,
  Folder,
  Spinner,
  WarningCircle,
} from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import type { ShredFile } from "@/types";
import { ElevationPrompt } from "@/components/settings/ElevationPrompt";

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

/**
 * Detect whether a shred error looks like a permission/ACL denial that
 * could be resolved by re-launching the app as administrator. We match
 * either the structured PermissionDenied variant or the localized
 * "Access is denied" message that surfaces on Windows when the OS
 * refuses access to a protected path.
 */
function isPermissionDeniedError(error: string | undefined): boolean {
  if (!error) return false;
  return (
    error.includes("Permission denied") ||
    error.includes("PermissionDenied") ||
    error.includes("Access is denied") ||
    error.includes("File locked")
  );
}

export function FileListItem({ file }: { file: ShredFile }) {
  const { removeFile } = useShred();
  const [elevationOpen, setElevationOpen] = useState(false);

  const isDirectory = file.kind === "directory";
  // Retained failed roots carry a root_status from applyRootResults. A
  // blocked or missing target was never executed, so elevation cannot fix
  // it — the Retry-as-admin offer is only valid for execution failures.
  const hasExecutionResult = file.root_status !== undefined;
  const showElevation =
    file.status === "error" &&
    hasExecutionResult &&
    isPermissionDeniedError(file.error);

  const firstChildError = file.child_errors?.[0];

  return (
    <div className="flex items-center gap-3 border-b border-border bg-surface px-4 py-2 hover:bg-elevated">
      <StatusIcon status={file.status} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1 min-w-0">
          {isDirectory && (
            <Folder size={14} className="shrink-0 text-muted-foreground" />
          )}
          <p className="truncate font-mono text-sm text-foreground">{file.name}</p>
          {file.is_shortcut && (
            <span
              title={
                file.shortcut_target
                  ? `Shortcut to: ${file.shortcut_target}`
                  : "Shortcut"
              }
              className="shrink-0 text-amber-500"
            >
              ⚠️
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isDirectory ? (
            <p className="font-mono text-xs text-muted-foreground">folder</p>
          ) : (
            <p className="font-mono text-xs text-muted-foreground">
              {file.size > 0
                ? file.size > 1073741824
                  ? `${(file.size / 1073741824).toFixed(2)} GB`
                  : `${(file.size / 1048576).toFixed(1)} MB`
                : "—"}
            </p>
          )}
          {file.status === "error" && !hasExecutionResult && file.error && (
            <p className="truncate text-xs font-medium text-red-500">
              {file.error}
            </p>
          )}
          {file.status === "error" &&
            hasExecutionResult &&
            (firstChildError ? (
              <>
                <p className="truncate text-xs text-red-500">
                  <span className="font-medium uppercase">{file.root_status}</span>
                  : {firstChildError.message}
                </p>
                {firstChildError.actionable && (
                  <p className="truncate text-xs text-red-500/80">
                    {firstChildError.actionable}
                  </p>
                )}
              </>
            ) : (
              file.error && (
                <p className="truncate text-xs text-red-500">{file.error}</p>
              )
            ))}
          {showElevation && (
            <button
              type="button"
              onClick={() => setElevationOpen(true)}
              className="shrink-0 font-mono text-xs uppercase tracking-wider text-amber-500 transition-colors hover:text-amber-400"
            >
              Retry as admin
            </button>
          )}
        </div>
      </div>
      {(file.status === "pending" || file.status === "error") && (
        <button
          type="button"
          onClick={() => removeFile(file.id)}
          aria-label={`Remove ${file.name}`}
          className="p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
        >
          <X size={14} />
        </button>
      )}
      <ElevationPrompt
        open={elevationOpen}
        onOpenChange={setElevationOpen}
        errorMessage={file.error}
      />
    </div>
  );
}
