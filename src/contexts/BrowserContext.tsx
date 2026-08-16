// src/contexts/BrowserContext.tsx
import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BrowserRunningState, DetectedBrowser } from "@/types";
import { useShred } from "@/contexts/ShredContext";

/** Lightweight running-state poll cadence (running state only — no discovery). */
const RUNNING_STATE_POLL_MS = 5000;

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

  // Latest browser list for the interval callback without restarting the
  // interval on every state change (including running-state flips).
  const browsersRef = useRef(browsers);
  useEffect(() => {
    browsersRef.current = browsers;
  }, [browsers]);

  const hasBrowsers = browsers.length > 0;

  // Lightweight running-state watcher: while browsers are known, refresh
  // ONLY their running state via the dedicated backend command. Never calls
  // full installed-browser discovery, never logs routine polling, never
  // lets requests overlap. A refresh failure or omitted requested browser
  // makes a cached `closed` state `unknown`; `running` and `unknown` remain
  // unchanged. Browser identities/profiles/selection are preserved — only
  // `runningState` is updated.
  useEffect(() => {
    if (!hasBrowsers) return;
    let disposed = false;
    let inFlight = false;

    const refreshRunningStates = async () => {
      if (inFlight) return;
      inFlight = true;
      const requestedBrowsers = browsersRef.current;
      const requestedStateById = new Map(
        requestedBrowsers.map((b) => [b.id, b.runningState])
      );
      const requests = requestedBrowsers.map((b) => ({
        browserId: b.id,
        profilePaths: b.profiles.map((p) => p.path),
      }));
      try {
        const states = await invoke<BrowserRunningState[]>(
          "check_browser_running_states",
          { requests }
        );
        if (disposed) return;
        const stateById = new Map(states.map((s) => [s.browserId, s.state]));
        setBrowsers((prev) =>
          prev.map((b) => {
            if (stateById.has(b.id)) {
              return { ...b, runningState: stateById.get(b.id)! };
            }
            return requestedStateById.get(b.id) === "closed"
              ? { ...b, runningState: "unknown" }
              : b;
          })
        );
      } catch {
        if (disposed) return;
        // Transient failure: only requested browsers cached as closed become
        // unknown; running and unknown remain unchanged. Never fall back to a
        // full discovery scan and never spam the log.
        setBrowsers((prev) =>
          prev.map((b) =>
            requestedStateById.get(b.id) === "closed"
              ? { ...b, runningState: "unknown" }
              : b
          )
        );
      } finally {
        inFlight = false;
      }
    };

    const interval = setInterval(refreshRunningStates, RUNNING_STATE_POLL_MS);
    return () => {
      disposed = true;
      clearInterval(interval);
    };
  }, [hasBrowsers]);

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
      const browserNames = browsers.map((b) => b.name);
      const profileCount = browsers.reduce((sum, b) => sum + b.profiles.length, 0);
      addLogEntry(
        "success",
        `Found ${browserNames.join(", ")} (${profileCount} profile${profileCount !== 1 ? "s" : ""})`
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
