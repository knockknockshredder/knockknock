// src/hooks/useBrowserDetection.ts
import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();
  const hasScanned = useRef(false);

  useEffect(() => {
    if (hasScanned.current) return;
    hasScanned.current = true;

    async function scan() {
      setIsScanning(true);
      addLogEntry("info", "Scanning for installed browsers...");

      try {
        const browsers = await invoke<DetectedBrowser[]>("detect_browsers");

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
        setIsScanning(false);
        addLogEntry("error", `Browser scan failed: ${err}`);
      }
    }

    scan();
  }, [setBrowsers, setIsScanning, addLogEntry]);
}