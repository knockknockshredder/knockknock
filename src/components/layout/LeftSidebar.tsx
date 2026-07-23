// src/components/layout/LeftSidebar.tsx
import { useState } from "react";
import { CheckSquare, Square, ArrowClockwise } from "@phosphor-icons/react";
import { BrowserCard } from "@/components/browser/BrowserCard";
import { BrowserWarning } from "@/components/browser/BrowserWarning";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useBrowser } from "@/contexts/BrowserContext";

export function LeftSidebar() {
  const { browsers, isScanning, selectAllProfiles, deselectAllProfiles, rescanBrowsers } = useBrowser();
  const [acknowledgedBrowsers, setAcknowledgedBrowsers] = useState<Set<string>>(new Set());
  const runningBrowsers = browsers.filter((b) => b.isRunning);

  const selectAllAll = () => browsers.forEach((b) => {
    selectAllProfiles(b.id);
  });
  const deselectAllAll = () => browsers.forEach((b) => {
    deselectAllProfiles(b.id);
  });

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-border flex items-center justify-between">
        <h2 className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Browsers
        </h2>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={selectAllAll}
            className="text-muted-foreground hover:text-accent transition-colors"
            title="Select all"
            aria-label="Select all browser profiles"
          >
            <CheckSquare size={14} />
          </button>
          <button
            type="button"
            onClick={deselectAllAll}
            className="text-muted-foreground hover:text-accent transition-colors"
            title="Deselect all"
            aria-label="Deselect all browser profiles"
          >
            <Square size={14} />
          </button>
          <button
            type="button"
            onClick={rescanBrowsers}
            className="text-muted-foreground hover:text-accent transition-colors"
            title="Rescan browsers"
            aria-label="Rescan browsers"
          >
            <ArrowClockwise size={14} />
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-hidden">
        <ScrollArea className="h-full">
          <div className="flex flex-col gap-3 p-3">
            {runningBrowsers.map((b) => (
              <BrowserWarning
                key={b.id}
                browserName={b.name}
                onAcknowledge={() => setAcknowledgedBrowsers(prev => new Set(prev).add(b.id))}
                acknowledged={acknowledgedBrowsers.has(b.id)}
              />
            ))}

            {isScanning ? (
              <p className="text-sm text-muted-foreground">Scanning for browsers...</p>
            ) : browsers.length === 0 ? (
              <p className="text-sm text-muted-foreground">No browsers detected.</p>
            ) : (
              <div className="flex flex-col gap-3">
                {browsers.map((browser) => (
                  <BrowserCard key={browser.id} browser={browser} />
                ))}
              </div>
            )}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}