// src/contexts/BrowserContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DetectedBrowser } from "@/types";
import { useShred } from "@/contexts/ShredContext";

interface BrowserState {
  browsers: DetectedBrowser[];
  isScanning: boolean;
  setBrowsers: (browsers: DetectedBrowser[]) => void;
  setIsScanning: (v: boolean) => void;
  toggleProfile: (browserId: string, profileId: string) => void;
  selectAllProfiles: (browserId: string) => void;
  deselectAllProfiles: (browserId: string) => void;
  getSelectedCount: () => number;
  rescanBrowsers: () => Promise<void>;
}

const BrowserContext = createContext<BrowserState | null>(null);

export function BrowserProvider({ children }: { children: ReactNode }) {
  const [browsers, setBrowsers] = useState<DetectedBrowser[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const { addLogEntry } = useShred();

  const toggleProfile = (browserId: string, profileId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? {
              ...b,
              profiles: b.profiles.map((p) =>
                p.id === profileId ? { ...p, selected: !p.selected } : p
              ),
            }
          : b
      )
    );
  };

  const selectAllProfiles = (browserId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? { ...b, profiles: b.profiles.map((p) => ({ ...p, selected: true })) }
          : b
      )
    );
  };

  const deselectAllProfiles = (browserId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? { ...b, profiles: b.profiles.map((p) => ({ ...p, selected: false })) }
          : b
      )
    );
  };

  const getSelectedCount = () =>
    browsers.reduce(
      (sum, b) => sum + b.profiles.filter((p) => p.selected).length,
      0
    );

  const rescanBrowsers = async () => {
    setIsScanning(true);
    addLogEntry("info", "Rescanning for installed browsers...");
    try {
      const browsers = await invoke<DetectedBrowser[]>("detect_browsers");
      setBrowsers(browsers);
      addLogEntry(
        "success",
        `Rescan complete: found ${browsers.length} browsers, ${browsers.reduce((sum, b) => sum + b.profiles.length, 0)} profiles`
      );
    } catch (err) {
      addLogEntry("error", `Browser rescan failed: ${err}`);
    } finally {
      setIsScanning(false);
    }
  };

  return (
    <BrowserContext.Provider
      value={{
        browsers,
        isScanning,
        setBrowsers,
        setIsScanning,
        toggleProfile,
        selectAllProfiles,
        deselectAllProfiles,
        getSelectedCount,
        rescanBrowsers,
      }}
    >
      {children}
    </BrowserContext.Provider>
  );
}

export function useBrowser() {
  const ctx = useContext(BrowserContext);
  if (!ctx) throw new Error("useBrowser must be used within BrowserProvider");
  return ctx;
}
