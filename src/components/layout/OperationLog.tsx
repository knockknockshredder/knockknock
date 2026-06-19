// src/components/layout/OperationLog.tsx
import { useState } from "react";
import { CaretDown, CaretUp, Trash } from "@phosphor-icons/react";
import {
  Terminal,
  AnimatedSpan,
} from "@/components/ui/terminal";
import { useShred } from "@/contexts/ShredContext";
import { useNavigation } from "@/contexts/NavigationContext";
import { cn } from "@/lib/utils";
import type { LogEntry } from "@/types";

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
  const [collapsed, setCollapsed] = useState(false);

  const emptyMessage =
    activeSection === "shred"
      ? "No operations yet. Drop files to begin."
      : activeSection === "browser"
        ? "No browser operations yet."
        : "No log entries.";

  return (
    <div className="border-t border-border bg-surface">
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs text-muted-foreground">
            operation.log
          </span>
          <span className="font-mono text-xs text-muted-foreground">
            ({logEntries.length})
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={clearLog}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
            title="Clear log"
          >
            <Trash size={14} />
          </button>
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
          >
            {collapsed ? <CaretUp size={14} /> : <CaretDown size={14} />}
          </button>
        </div>
      </div>
      {!collapsed && (
        <div className="h-[180px] overflow-auto border-t border-border px-4 pb-4">
          {logEntries.length === 0 ? (
            <p className="py-4 text-center font-mono text-xs text-muted-foreground">
              {emptyMessage}
            </p>
          ) : (
            <Terminal sequence={false} className="max-w-none border-0 bg-transparent p-0">
              {logEntries.map((entry) => (
                <AnimatedSpan
                  key={entry.id}
                  delay={0}
                  className={cn("font-mono text-xs", logColor(entry.level))}
                >
                  <span>
                    {entry.level === "command" ? "> " : ""}
                    {entry.message}
                  </span>
                </AnimatedSpan>
              ))}
            </Terminal>
          )}
        </div>
      )}
    </div>
  );
}
