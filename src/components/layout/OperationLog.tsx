// src/components/layout/OperationLog.tsx
import { useEffect, useRef, useState } from "react";
import { CaretDown, CaretUp, Trash } from "@phosphor-icons/react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useShred } from "@/contexts/ShredContext";
import { useNavigation } from "@/contexts/NavigationContext";
import { cn } from "@/lib/utils";
import type { LogEntry } from "@/types";

function formatTime(date: Date): string {
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function logColor(level: LogEntry["level"]): string {
  switch (level) {
    case "success":
      return "text-green-500";
    case "error":
      return "text-red-500";
    case "warning":
      return "text-amber-500";
    case "command":
      return "text-cyan-400";
    default:
      return "text-foreground";
  }
}

export function OperationLog() {
  const { logEntries, clearLog } = useShred();
  const { activeSection } = useNavigation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [collapsed, setCollapsed] = useState(false);

  const handleScroll = () => {
    const viewport = scrollRef.current?.querySelector(
      '[data-slot="scroll-area-viewport"]'
    ) as HTMLDivElement | null;
    if (!viewport) return;
    setIsAtBottom(viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 20);
  };

  useEffect(() => {
    const viewport = scrollRef.current?.querySelector(
      '[data-slot="scroll-area-viewport"]'
    ) as HTMLDivElement | null;
    if (isAtBottom && viewport) {
      viewport.scrollTop = viewport.scrollHeight;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [logEntries.length, isAtBottom]);

  const emptyMessage =
    activeSection === "home"
      ? "No operations yet. Drop files or select browser profiles to begin."
      : "No log entries.";

  return (
    <div className="border-t border-border bg-surface">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs text-muted-foreground">operation.log</span>
          <span className="font-mono text-xs text-muted-foreground">({logEntries.length})</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={clearLog}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
            title="Clear log"
          >
            <Trash size={14} />
          </button>
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
            title={collapsed ? "Expand log" : "Collapse log"}
          >
            {collapsed ? <CaretUp size={14} /> : <CaretDown size={14} />}
          </button>
        </div>
      </div>
      {!collapsed && (
        <div ref={scrollRef} onScroll={handleScroll}>
          <ScrollArea className="h-[180px] border-t border-border px-4 pb-4">
            {logEntries.length === 0 ? (
              <p className="py-4 text-center font-mono text-xs text-muted-foreground">
                {emptyMessage}
              </p>
            ) : (
              <div className="flex flex-col gap-0.5">
                {logEntries.map((entry) => (
                  <div
                    key={entry.id}
                    className={cn("font-mono text-xs", logColor(entry.level))}
                  >
                    <span className="text-muted-foreground">[{formatTime(entry.timestamp)}]</span>{" "}
                    {entry.level === "command" ? "> " : ""}
                    {entry.message}
                  </div>
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      )}
    </div>
  );
}
