// src/sections/ShredSection.tsx
import { FileDropZone } from "@/components/shred/FileDropZone";
import { FileList } from "@/components/shred/FileList";

export function ShredSection() {
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Shred Files</h1>
      <FileDropZone />
      <FileList />
    </div>
  );
}
