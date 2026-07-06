// src/hooks/useBrowserDetection.ts
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();

  useEffect(() => {
    let cancelled = false;

    async function scan() {
      setIsScanning(true);
      addLogEntry("info", "Scanning for installed browsers...");

      try {
        const browsers = await invoke<DetectedBrowser[]>("detect_browsers");
        if (cancelled) return;

        setBrowsers(browsers);
        setIsScanning(false);
        addLogEntry(
          "success",
          `Found ${browsers.length} browsers, ${browsers.reduce((sum, b) => sum + b.profiles.length, 0)} profiles`
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
