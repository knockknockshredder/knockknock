// src/components/shred/FileDropZone.tsx
import { useCallback, useEffect, useState } from "react";
import { FilePlus, FolderPlus, Upload } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useShred } from "@/contexts/ShredContext";
import { cn, isWindows } from "@/lib/utils";
import type { FileMetadata } from "@/types";

const FILE_ACTION_LABEL = "Add files — opens the file picker";
const FOLDER_ACTION_LABEL = "Add folders — opens the folder picker";

interface FileDropZoneProps {
  compact?: boolean;
}

export function FileDropZone({ compact = false }: FileDropZoneProps) {
  const { addFiles, addLogEntry } = useShred();
  const [isDragOver, setIsDragOver] = useState(false);

  const validateAndAdd = useCallback(
    async (paths: string[]) => {
      try {
        const [validFiles, validationErrors]: [FileMetadata[], string[]] = await invoke(
          "validate_paths",
          { paths }
        );
        if (validFiles.length > 0) {
          addFiles(validFiles);
          addLogEntry("info", `Added ${validFiles.length} target(s)`);
        }
        for (const err of validationErrors) {
          addLogEntry("warning", err);
        }
        if (validFiles.length < paths.length) {
          addLogEntry(
            "warning",
            `${paths.length - validFiles.length} target(s) rejected (protected path, network drive, or invalid path)`
          );
        }
      } catch (err) {
        addLogEntry("error", `Validation failed: ${err}`);
      }
    },
    [addFiles, addLogEntry]
  );

  // Tauri native drag-drop
  useEffect(() => {
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        const paths = event.payload.paths;
        if (paths.length > 0) {
          validateAndAdd(paths);
        }
      } else {
        setIsDragOver(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [validateAndAdd]);

  const handleFileClick = async () => {
    try {
      let paths: string[];
      if (isWindows()) {
        // Custom IFileOpenDialog with FOS_NODEREFERENCELINKS so `.lnk`
        // shortcuts return as the link file itself, not their resolved target.
        paths = await invoke<string[]>("open_files_windows");
      } else {
        const selected = await open({
          multiple: true,
          directory: false,
          title: "Select files to add",
        });
        if (!selected) return;
        paths = Array.isArray(selected) ? selected : [selected];
      }
      if (paths.length > 0) {
        await validateAndAdd(paths);
      }
    } catch (err) {
      const msg = String(err);
      // IFileDialog::Show returns HRESULT_FROM_WIN32(ERROR_CANCELLED)
      // (0x800704C7) when the user dismisses the dialog. Treat that as a
      // silent no-op so cancellation doesn't pollute the log.
      if (/cancel/i.test(msg) || /0x800704C7/i.test(msg)) return;
      addLogEntry("error", `File dialog failed: ${msg}`);
    }
  };

  const handleFolderClick = async () => {
    try {
      let paths: string[];
      if (isWindows()) {
        // Same IFileOpenDialog path as files, with FOS_PICKFOLDERS so the
        // user selects folder roots that are preserved as directories.
        paths = await invoke<string[]>("open_folders_windows");
      } else {
        const selected = await open({
          multiple: true,
          directory: true,
          title: "Select folders to add",
        });
        if (!selected) return;
        paths = Array.isArray(selected) ? selected : [selected];
      }
      if (paths.length > 0) {
        await validateAndAdd(paths);
      }
    } catch (err) {
      const msg = String(err);
      // Dismissing the dialog surfaces as ERROR_CANCELLED on Windows; treat
      // it as a silent no-op, matching the file picker behavior.
      if (/cancel/i.test(msg) || /0x800704C7/i.test(msg)) return;
      addLogEntry("error", `Folder dialog failed: ${msg}`);
    }
  };

  if (compact) {
    return (
      <div className="flex flex-col items-stretch gap-1 sm:flex-row sm:items-center sm:gap-1.5">
        <button
          type="button"
          onClick={handleFileClick}
          aria-label={FILE_ACTION_LABEL}
          title={FILE_ACTION_LABEL}
          className="flex items-center justify-center rounded-md border border-border p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <FilePlus size={14} />
        </button>
        <button
          type="button"
          onClick={handleFolderClick}
          aria-label={FOLDER_ACTION_LABEL}
          title={FOLDER_ACTION_LABEL}
          className="flex items-center justify-center rounded-md border border-border p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <FolderPlus size={14} />
        </button>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-3 border-2 border-dashed p-12 transition-colors",
        isDragOver
          ? "border-accent bg-accent/5"
          : "border-border hover:border-muted-foreground"
      )}
    >
      <Upload
        size={32}
        className={cn(
          "transition-colors",
          isDragOver ? "text-accent" : "text-muted-foreground"
        )}
      />
      <p className="text-sm text-muted-foreground">
        Drop files or folders here
      </p>
      <div className="flex flex-col gap-2 sm:flex-row sm:gap-2">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handleFileClick();
          }}
          aria-label={FILE_ACTION_LABEL}
          title={FILE_ACTION_LABEL}
          className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <FilePlus size={14} />
          Add Files
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handleFolderClick();
          }}
          aria-label={FOLDER_ACTION_LABEL}
          title={FOLDER_ACTION_LABEL}
          className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <FolderPlus size={14} />
          Add Folder
        </button>
      </div>
      <p className="text-sm text-muted-foreground">
        Items are added to the review list. Nothing is deleted until you review and confirm the operation.
      </p>
    </div>
  );
}
