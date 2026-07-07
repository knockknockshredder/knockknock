// src/hooks/useBrowserDetection.ts
import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();
  const scanIdRef = useRef(0);

  useEffect(() => {
    const id = ++scanIdRef.current;
    let cancelled = false;

    async function scan() {
      setIsScanning(true);
      addLogEntry("info", "Scanning for installed browsers...");

      try {
        const browsers = await invoke<DetectedBrowser[]>("detect_browsers");
        if (cancelled || scanIdRef.current !== id) return;

        setBrowsers(browsers);
        setIsScanning(false);
        addLogEntry(
          "success",
          `Found ${browsers.length} browsers, ${browsers.reduce((sum, b) => sum + b.profiles.length, 0)} profiles`
        );
      } catch (err) {
        if (cancelled || scanIdRef.current !== id) return;
        setIsScanning(false);
        addLogEntry("error", `Browser scan failed: ${err}`);
      }
    }

    scan();
    return () => { cancelled = true; };
  }, [setBrowsers, setIsScanning, addLogEntry]);
}
