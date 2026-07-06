// src/components/layout/OperationLog.tsx
import { useEffect, useRef, useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
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
  const { logEntries, clearLog } = useShred();
  const { activeSection } = useNavigation();
  const [collapsed, setCollapsed] = useState(false);
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
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger
                render={
                  <button
                    type="button"
                    onClick={clearLog}
                    className="h-3 w-3 rounded-full bg-red-500 hover:bg-red-400 transition-colors"
                  />
                }
              />
              <TooltipContent>Clear log</TooltipContent>
            </Tooltip>
          </TooltipProvider>

          {!collapsed ? (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger
                  render={
                    <button
                      type="button"
                      onClick={() => setCollapsed(true)}
                      className="h-3 w-3 rounded-full bg-amber-500 hover:bg-amber-400 transition-colors"
                    />
                  }
                />
                <TooltipContent>Collapse log</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          ) : (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger
                  render={
                    <button
                      type="button"
                      onClick={() => setCollapsed(false)}
                      className="h-3 w-3 rounded-full bg-green-500 hover:bg-green-400 transition-colors"
                    />
                  }
                />
                <TooltipContent>Expand log</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
        </div>
      </div>
      {!collapsed && (
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
      )}
    </div>
  );
}
