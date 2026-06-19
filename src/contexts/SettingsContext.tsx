// src/contexts/SettingsContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";

interface SettingsState {
  autoClearLog: boolean;
  setAutoClearLog: (v: boolean) => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [autoClearLog, setAutoClearLog] = useState(false);

  return (
    <SettingsContext.Provider value={{ autoClearLog, setAutoClearLog }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings must be used within SettingsProvider");
  return ctx;
}
