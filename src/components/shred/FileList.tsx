// src/components/shred/FileList.tsx
import { useShred } from "@/contexts/ShredContext";
import { FileListItem } from "./FileListItem";
import { ScrollArea } from "@/components/ui/scroll-area";

export function FileList() {
  const { files } = useShred();

  if (files.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        No files selected
      </p>
    );
  }

  return (
    <ScrollArea className="h-[240px] rounded border border-border">
      {files.map((file) => (
        <FileListItem key={file.id} file={file} />
      ))}
    </ScrollArea>
  );
}
