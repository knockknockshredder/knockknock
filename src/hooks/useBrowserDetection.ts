// src/hooks/useBrowserDetection.ts
import { useEffect } from "react";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

const MOCK_BROWSERS: DetectedBrowser[] = [
  {
    id: "chrome",
    name: "Google Chrome",
    icon: "GoogleChrome",
    isRunning: false,
    profiles: [
      { id: "chrome-default", name: "Default", path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default", size: 524288000, selected: false },
      { id: "chrome-profile1", name: "Profile 1", path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Profile 1", size: 104857600, selected: false },
    ],
  },
  {
    id: "firefox",
    name: "Mozilla Firefox",
    icon: "FirefoxLogo",
    isRunning: false,
    profiles: [
      { id: "firefox-default", name: "default-release", path: "%APPDATA%\\Mozilla\\Firefox\\Profiles\\xxxx.default-release", size: 314572800, selected: false },
    ],
  },
];

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();

  useEffect(() => {
    setIsScanning(true);
    addLogEntry("info", "Scanning for installed browsers...");

    // TODO: Replace with real backend call
    const timeout = setTimeout(() => {
      setBrowsers(MOCK_BROWSERS);
      setIsScanning(false);
      addLogEntry(
        "success",
        `Found ${MOCK_BROWSERS.length} browsers, ${MOCK_BROWSERS.reduce((sum, b) => sum + b.profiles.length, 0)} profiles`
      );
    }, 800);

    return () => clearTimeout(timeout);
  }, [setBrowsers, setIsScanning, addLogEntry]);
}
