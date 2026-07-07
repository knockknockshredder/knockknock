// src/components/layout/RightSidebar.tsx
import { Trash } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import { FileList } from "@/components/shred/FileList";
import { FileDropZone } from "@/components/shred/FileDropZone";

export function RightSidebar() {
  const { files, clearFiles } = useShred();

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-border flex items-center justify-between">
        <h2 className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Files
        </h2>
        {files.length > 0 && (
          <div className="flex items-center gap-2">
            <FileDropZone compact />
            <button
              type="button"
              onClick={clearFiles}
              className="text-muted-foreground hover:text-destructive transition-colors"
              title="Remove all files"
            >
              <Trash size={14} />
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 overflow-hidden">
        {files.length === 0 ? (
          <div className="p-3 h-full">
            <FileDropZone />
          </div>
        ) : (
          <FileList />
        )}
      </div>
    </div>
  );
}