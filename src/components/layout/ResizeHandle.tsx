// src/components/layout/ResizeHandle.tsx
import { useCallback, useEffect } from "react";
import { cn } from "@/lib/utils";

interface ResizeHandleProps {
  onResize: (deltaX: number) => void;
  onReset: () => void;
  side: "left" | "right";
}

export function ResizeHandle({ onResize, onReset, side }: ResizeHandleProps) {
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      e.preventDefault();
      const startX = e.clientX;
      document.body.style.cursor = "col-resize";

      const handleMouseMove = (ev: MouseEvent) => {
        const deltaX = (ev.clientX - startX) / 3;
        onResize(deltaX);
      };

      const handleMouseUp = () => {
        document.body.style.cursor = "";
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [onResize]
  );

  // Cleanup cursor on unmount
  useEffect(() => {
    return () => {
      document.body.style.cursor = "";
    };
  }, []);

  return (
    <div
      onMouseDown={handleMouseDown}
      onDoubleClick={onReset}
      className={cn(
        "h-full shrink-0 cursor-col-resize bg-transparent transition-colors hover:bg-border",
        side === "left" ? "-ml-0.5" : "-mr-0.5"
      )}
      style={{ width: 4 }}
    />
  );
}