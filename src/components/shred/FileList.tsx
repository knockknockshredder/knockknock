// src/components/shred/FileList.tsx
import { useEffect, useRef } from "react";
import { useShred } from "@/contexts/ShredContext";
import { FileListItem } from "./FileListItem";
import { ScrollArea } from "@/components/ui/scroll-area";

export function FileList() {
  const { files } = useShred();
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when files are added
  useEffect(() => {
    if (scrollRef.current) {
      const viewport = scrollRef.current.querySelector(
        '[data-slot="scroll-area-viewport"]'
      ) as HTMLDivElement | null;
      if (viewport) {
        viewport.scrollTop = viewport.scrollHeight;
      }
    }
    // Intentionally depend on files.length only — status updates mid-shred
    // must not yank the scroll position to the bottom.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [files.length]);

  if (files.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        No files selected
      </p>
    );
  }

  return (
    <div ref={scrollRef} className="h-full">
      <ScrollArea className="h-full rounded border border-border">
        {files.map((file) => (
          <FileListItem key={file.id} file={file} />
        ))}
      </ScrollArea>
    </div>
  );
}
