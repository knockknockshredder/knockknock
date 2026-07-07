// src/components/layout/LeftSidebar.tsx
import { useState } from "react";
import { BrowserCard } from "@/components/browser/BrowserCard";
import { BrowserWarning } from "@/components/browser/BrowserWarning";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useBrowser } from "@/contexts/BrowserContext";

export function LeftSidebar() {
  const { browsers, isScanning, selectAllProfiles, deselectAllProfiles } = useBrowser();
  const [acknowledgedBrowsers, setAcknowledgedBrowsers] = useState<Set<string>>(new Set());
  const runningBrowsers = browsers.filter((b) => b.isRunning);

  const handleAcknowledge = (browserId: string) => {
    setAcknowledgedBrowsers((prev) => new Set(prev).add(browserId));
  };

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
        <div className="flex gap-2">
          <button
            type="button"
            onClick={selectAllAll}
            className="font-mono text-xs text-accent hover:underline"
          >
            Select all
          </button>
          <button
            type="button"
            onClick={deselectAllAll}
            className="font-mono text-xs text-muted-foreground hover:underline"
          >
            Deselect all
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
                onAcknowledge={() => handleAcknowledge(b.id)}
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