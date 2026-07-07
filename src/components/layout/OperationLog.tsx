// src/components/layout/OperationLog.tsx
import { useEffect, useRef, useState } from "react";
import {
  Terminal,
  AnimatedSpan,
} from "@/components/ui/terminal";
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
  const { logEntries } = useShred();
  const { activeSection } = useNavigation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);

  const handleScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    setIsAtBottom(el.scrollHeight - el.scrollTop - el.clientHeight < 20);
  };

  useEffect(() => {
    if (isAtBottom && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logEntries, isAtBottom]);

  const emptyMessage =
    activeSection === "home"
      ? "No operations yet. Drop files or select browser profiles to begin."
      : "No log entries.";

  return (
    <div className="border-t border-border bg-surface">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
        <span className="font-mono text-xs text-muted-foreground">operation.log</span>
        <span className="font-mono text-xs text-muted-foreground">({logEntries.length})</span>
      </div>
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="h-[180px] overflow-auto border-t border-border px-4 pb-4"
      >
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
                  <span className="text-muted-foreground">[{formatTime(entry.timestamp)}]</span>{" "}
                  {entry.level === "command" ? "> " : ""}
                  {entry.message}
                </span>
              </AnimatedSpan>
            ))}
          </Terminal>
        )}
      </div>
    </div>
  );
}
