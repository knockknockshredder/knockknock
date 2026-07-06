// src/components/layout/RightSidebar.tsx
import { FileDropZone } from "@/components/shred/FileDropZone";
import { FileList } from "@/components/shred/FileList";

export function RightSidebar() {
  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-border">
        <h2 className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Files
        </h2>
      </div>
      <div className="flex-1 overflow-auto p-3">
        <FileDropZone />
        <FileList />
      </div>
    </div>
  );
}