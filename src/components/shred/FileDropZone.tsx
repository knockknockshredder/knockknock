// src/components/shred/FileDropZone.tsx
import { useCallback, useEffect, useState } from "react";
import { Plus, Upload } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

interface FileMetadata {
  path: string;
  name: string;
  size: number;
}

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
      const validFiles: FileMetadata[] = await invoke("validate_paths", { paths });
      if (validFiles.length > 0) {
        addFiles(validFiles);
        addLogEntry("info", `Added ${validFiles.length} file(s)`);
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

  const handleClick = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: "Select files to shred",
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        await validateAndAdd(paths);
      }
    } catch (err) {
      addLogEntry("error", `File dialog failed: ${err}`);
    }
  };

  if (compact) {
    return (
      <button
        type="button"
        onClick={handleClick}
        className="text-muted-foreground hover:text-foreground transition-colors"
        title="Add files"
      >
        <Plus size={14} />
      </button>
    );
  }

  return (
    <div
      onClick={handleClick}
      className={cn(
        "flex cursor-pointer flex-col items-center justify-center gap-3 rounded border-2 border-dashed p-12 transition-colors",
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
        Drop files here or click to browse
      </p>
    </div>
  );
}
