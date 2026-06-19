// src/sections/BrowserSection.tsx
import { useState } from "react";
import { BrowserCard } from "@/components/browser/BrowserCard";
import { BrowserWarning } from "@/components/browser/BrowserWarning";
import { useBrowser } from "@/contexts/BrowserContext";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

export function BrowserSection() {
  useBrowserDetection();
  const { browsers, isScanning, getSelectedCount } = useBrowser();
  const [acknowledgedBrowsers, setAcknowledgedBrowsers] = useState<Set<string>>(new Set());
  const runningBrowsers = browsers.filter((b) => b.isRunning);
  const selectedCount = getSelectedCount();

  const handleAcknowledge = (browserId: string) => {
    setAcknowledgedBrowsers((prev) => new Set(prev).add(browserId));
  };

  const allAcknowledged = runningBrowsers.every((b) =>
    acknowledgedBrowsers.has(b.id)
  );

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Browser Cleanup</h1>

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
        <div className="flex flex-col gap-4">
          {browsers.map((browser) => (
            <BrowserCard key={browser.id} browser={browser} />
          ))}
        </div>
      )}

      {selectedCount > 0 && (
        <div className="flex flex-col items-center gap-2 pt-4">
          <button
            disabled={!allAcknowledged}
            className="w-full max-w-[400px] border-2 border-destructive px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider text-destructive transition-colors hover:border-red-500 hover:bg-red-500 hover:text-background disabled:cursor-not-allowed disabled:opacity-40"
          >
            Clean {selectedCount} Profile{selectedCount !== 1 ? "s" : ""}
          </button>
          {!allAcknowledged && (
            <p className="font-mono text-xs text-amber-500">
              Acknowledge running browser warnings to proceed
            </p>
          )}
        </div>
      )}
    </div>
  );
}
