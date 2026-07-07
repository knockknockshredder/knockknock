// src/hooks/useBrowserDetection.ts
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

// Module-level guard persists across StrictMode double-invoke
let hasScanned = false;

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();

  useEffect(() => {
    if (hasScanned) return;
    hasScanned = true;
    let cancelled = false;

    async function scan() {
      setIsScanning(true);
      addLogEntry("info", "Scanning for installed browsers...");

      try {
        const browsers = await invoke<DetectedBrowser[]>("detect_browsers");
        if (cancelled) return;

        setBrowsers(browsers);
        setIsScanning(false);

        // Build human-friendly browser list
        const browserNames = browsers.map((b) => b.name);
        const profileCount = browsers.reduce((sum, b) => sum + b.profiles.length, 0);
        addLogEntry(
          "success",
          `Found ${browserNames.join(", ")} (${profileCount} profile${profileCount !== 1 ? "s" : ""})`
        );
      } catch (err) {
        if (cancelled) return;
        setIsScanning(false);
        addLogEntry("error", `Browser scan failed: ${err}`);
      }
    }

    scan();
    return () => { cancelled = true; };
  }, [setBrowsers, setIsScanning, addLogEntry]);
}