// src/components/shred/FileDropZone.tsx
import { useEffect, useState } from "react";
import { Plus, Upload } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useShred } from "@/contexts/ShredContext";
import { cn, isWindows } from "@/lib/utils";
import type { FileMetadata } from "@/types";

interface FileDropZoneProps {
  compact?: boolean;
}

export function FileDropZone({ compact = false }: FileDropZoneProps) {
  const { addFiles, addLogEntry } = useShred();
  const [isDragOver, setIsDragOver] = useState(false);

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
  }, []);

  const validateAndAdd = async (paths: string[]) => {
    try {
      const [validFiles, validationErrors]: [FileMetadata[], string[]] = await invoke("validate_paths", { paths });
      if (validFiles.length > 0) {
        addFiles(validFiles);
        addLogEntry("info", `Added ${validFiles.length} file(s)`);
      }
      for (const err of validationErrors) {
        addLogEntry("warning", err);
      }
      if (validFiles.length < paths.length) {
        addLogEntry(
          "warning",
          `${paths.length - validFiles.length} file(s) rejected (system file, network drive, or invalid path)`
        );
      }
    } catch (err) {
      addLogEntry("error", `Validation failed: ${err}`);
    }
  };

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
          title: "Select files to shred",
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
      const selected = await open({
        multiple: true,
        directory: true,
        title: "Select folders to shred",
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        await validateAndAdd(paths);
      }
    } catch (err) {
      addLogEntry("error", `Folder dialog failed: ${err}`);
    }
  };

  if (compact) {
    return (
      <button
        type="button"
        onClick={handleFileClick}
        className="text-muted-foreground hover:text-foreground transition-colors"
        title="Add files"
      >
        <Plus size={14} />
      </button>
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
      <div className="flex gap-2">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handleFileClick();
          }}
          className="rounded-md border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:bg-muted transition-colors"
        >
          Add Files
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handleFolderClick();
          }}
          className="rounded-md border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:bg-muted transition-colors"
        >
          Add Folder
        </button>
      </div>
    </div>
  );
}
