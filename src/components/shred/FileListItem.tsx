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
 * Detect whether an execution error is a permission/ACL denial that may
 * benefit from re-launching the app as administrator. File-lock errors are
 * intentionally excluded: elevation does not release handles held by another
 * process.
 */
function isPermissionDeniedError(error: string | undefined): boolean {
  if (!error) return false;
  return (
    error.includes("Permission denied") ||
    error.includes("PermissionDenied") ||
    error.includes("Access is denied")
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

  // A root whose final write check did not pass is not a clean success: it
  // was removed, so this is a warning — never a plain "Success" and never an
  // execution error (M3).
  const writeCheckFailed =
    hasExecutionResult &&
    (file.write_check === "failed" ||
      file.child_errors?.some((error) => error.stage === "verify"));

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
            (writeCheckFailed ? (
              <>
                <p className="truncate text-xs font-medium text-amber-500">
                  Deletion completed, but the requested write check did not pass.
                </p>
                {firstChildError?.actionable && (
                  <p className="truncate text-xs text-amber-500/80">
                    {firstChildError.actionable}
                  </p>
                )}
              </>
            ) : firstChildError ? (
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
