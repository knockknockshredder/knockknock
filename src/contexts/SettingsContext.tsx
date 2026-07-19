// src/contexts/SettingsContext.tsx
import {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useRef,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LogObfuscation } from "@/types";

// Must match the Rust AppSettings struct fields exactly.
interface AppSettings {
  auto_clear_log: boolean;
  default_algorithm_index: number;
  log_obfuscation: string;
  left_sidebar_width: number;
  right_sidebar_width: number;
}

interface SettingsState {
  autoClearLog: boolean;
  setAutoClearLog: (v: boolean) => void;
  defaultAlgorithmIndex: number;
  setDefaultAlgorithmIndex: (v: number) => void;
  logObfuscation: LogObfuscation;
  setLogObfuscation: (v: LogObfuscation) => void;
  leftSidebarWidth: number;
  rightSidebarWidth: number;
  setLeftSidebarWidth: (v: number | ((prev: number) => number)) => void;
  setRightSidebarWidth: (v: number | ((prev: number) => number)) => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

function clampSidebarWidth(value: number): number {
  return Math.max(160, Math.min(400, value));
}

function isValidLogObfuscation(v: string): v is LogObfuscation {
  return v === "none" || v === "numbered" || v === "partial_mask";
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [loaded, setLoaded] = useState(false);
  const [autoClearLog, setAutoClearLogState] = useState(false);
  const [defaultAlgorithmIndex, setDefaultAlgorithmIndexState] = useState(0);
  const [logObfuscation, setLogObfuscationState] =
    useState<LogObfuscation>("none");
  const [leftSidebarWidth, setLeftSidebarWidthState] = useState(260);
  const [rightSidebarWidth, setRightSidebarWidthState] = useState(260);

  // Refs mirror state so debounced save can read freshest values
  // without closure-staleness when many events fire rapidly
  // (e.g. sidebar drag at 60Hz).
  const stateRef = useRef({
    autoClearLog,
    defaultAlgorithmIndex,
    logObfuscation,
    leftSidebarWidth,
    rightSidebarWidth,
  });
  stateRef.current = {
    autoClearLog,
    defaultAlgorithmIndex,
    logObfuscation,
    leftSidebarWidth,
    rightSidebarWidth,
  };

  // Debounce save — collapse rapid events (sidebar drag) into one IPC.
  const saveTimerRef = useRef<number | null>(null);
  const scheduleSave = useCallback(() => {
    if (!loaded) return;
    if (saveTimerRef.current !== null) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      const settings: AppSettings = {
        auto_clear_log: stateRef.current.autoClearLog,
        default_algorithm_index: stateRef.current.defaultAlgorithmIndex,
        log_obfuscation: stateRef.current.logObfuscation,
        left_sidebar_width: stateRef.current.leftSidebarWidth,
        right_sidebar_width: stateRef.current.rightSidebarWidth,
      };
      invoke("save_settings", { settings }).catch((e) => {
        console.error("[KnockKnock] Failed to save settings:", e);
      });
    }, 250);
  }, [loaded]);

  // Load settings from Rust on mount
  useEffect(() => {
    invoke<AppSettings>("get_settings")
      .then((s) => {
        setAutoClearLogState(s.auto_clear_log);
        setDefaultAlgorithmIndexState(s.default_algorithm_index);
        setLogObfuscationState(
          isValidLogObfuscation(s.log_obfuscation)
            ? s.log_obfuscation
            : "none",
        );
        setLeftSidebarWidthState(clampSidebarWidth(s.left_sidebar_width));
        setRightSidebarWidthState(clampSidebarWidth(s.right_sidebar_width));
        setLoaded(true);
      })
      .catch((e) => {
        console.error("[KnockKnock] Failed to load settings:", e);
        setLoaded(true); // proceed with defaults
      });
  }, []);

  // Flush pending save on unmount so close-while-debouncing persists
  useEffect(() => {
    return () => {
      if (saveTimerRef.current !== null) {
        clearTimeout(saveTimerRef.current);
        const settings: AppSettings = {
          auto_clear_log: stateRef.current.autoClearLog,
          default_algorithm_index: stateRef.current.defaultAlgorithmIndex,
          log_obfuscation: stateRef.current.logObfuscation,
          left_sidebar_width: stateRef.current.leftSidebarWidth,
          right_sidebar_width: stateRef.current.rightSidebarWidth,
        };
        // Synchronous-style IPC at unmount — async fire-and-forget.
        invoke("save_settings", { settings }).catch((e) => {
          console.error("[KnockKnock] Failed to flush settings on close:", e);
        });
      }
    };
  }, []);

  const setAutoClearLog = useCallback(
    (v: boolean) => {
      setAutoClearLogState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setDefaultAlgorithmIndex = useCallback(
    (v: number) => {
      setDefaultAlgorithmIndexState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setLogObfuscation = useCallback(
    (v: LogObfuscation) => {
      setLogObfuscationState(v);
      scheduleSave();
    },
    [scheduleSave],
  );

  const setLeftSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setLeftSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        return clampSidebarWidth(next);
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  const setRightSidebarWidth = useCallback(
    (v: number | ((prev: number) => number)) => {
      setRightSidebarWidthState((prev) => {
        const next = typeof v === "function" ? v(prev) : v;
        return clampSidebarWidth(next);
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  return (
    <SettingsContext.Provider
      value={{
        autoClearLog,
        setAutoClearLog,
        defaultAlgorithmIndex,
        setDefaultAlgorithmIndex,
        logObfuscation,
        setLogObfuscation,
        leftSidebarWidth,
        rightSidebarWidth,
        setLeftSidebarWidth,
        setRightSidebarWidth,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx)
    throw new Error("useSettings must be used within SettingsProvider");
  return ctx;
}
