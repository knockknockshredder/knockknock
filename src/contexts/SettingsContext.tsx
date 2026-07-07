// src/contexts/SettingsContext.tsx
import {
  createContext,
  useContext,
  useState,
  useEffect,
  type ReactNode,
} from "react";

const STORAGE_KEY = "knockknock-settings";

interface SettingsState {
  autoClearLog: boolean;
  setAutoClearLog: (v: boolean) => void;
  defaultAlgorithmIndex: number;
  setDefaultAlgorithmIndex: (v: number) => void;
  leftSidebarWidth: number;
  rightSidebarWidth: number;
  setLeftSidebarWidth: (v: number | ((prev: number) => number)) => void;
  setRightSidebarWidth: (v: number | ((prev: number) => number)) => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

interface PersistedSettings {
  autoClearLog?: boolean;
  defaultAlgorithmIndex?: number;
  leftSidebarWidth?: number;
  rightSidebarWidth?: number;
}

function loadSettings(): PersistedSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as PersistedSettings) : {};
  } catch {
    return {};
  }
}

function saveSettings(settings: PersistedSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const persisted = loadSettings();
  const [autoClearLog, setAutoClearLog] = useState<boolean>(
    persisted.autoClearLog ?? false
  );
  const [defaultAlgorithmIndex, setDefaultAlgorithmIndex] = useState<number>(
    persisted.defaultAlgorithmIndex ?? 0
  );
  const [leftSidebarWidth, setLeftSidebarWidth] = useState<number>(
    persisted.leftSidebarWidth ?? 260
  );
  const [rightSidebarWidth, setRightSidebarWidth] = useState<number>(
    persisted.rightSidebarWidth ?? 260
  );

  useEffect(() => {
    saveSettings({
      autoClearLog,
      defaultAlgorithmIndex,
      leftSidebarWidth,
      rightSidebarWidth,
    });
  }, [autoClearLog, defaultAlgorithmIndex, leftSidebarWidth, rightSidebarWidth]);

  return (
    <SettingsContext.Provider
      value={{
        autoClearLog,
        setAutoClearLog,
        defaultAlgorithmIndex,
        setDefaultAlgorithmIndex,
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
  if (!ctx) throw new Error("useSettings must be used within SettingsProvider");
  return ctx;
}