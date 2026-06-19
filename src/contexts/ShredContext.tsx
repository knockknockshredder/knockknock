// src/contexts/ShredContext.tsx
import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import type { ShredFile, LogEntry, AlgorithmOption } from "@/types";

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  addFiles: (files: Array<{ path: string; name: string; size: number }>) => void;
  removeFile: (id: string) => void;
  clearFiles: () => void;
  setAlgorithmIndex: (index: number) => void;
  setIsShredding: (v: boolean) => void;
  addLogEntry: (level: LogEntry["level"], message: string) => void;
  clearLog: () => void;
  setAlgorithms: (algorithms: AlgorithmOption[]) => void;
  updateFileStatus: (id: string, status: ShredFile["status"], error?: string) => void;
}

const ShredContext = createContext<ShredState | null>(null);

export function ShredProvider({ children }: { children: ReactNode }) {
  const [files, setFiles] = useState<ShredFile[]>([]);
  const [algorithmIndex, setAlgorithmIndex] = useState(0);
  const [isShredding, setIsShredding] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [algorithms, setAlgorithms] = useState<AlgorithmOption[]>([]);

  const addFiles = useCallback((newEntries: Array<{ path: string; name: string; size: number }>) => {
    const newFiles: ShredFile[] = newEntries
      .filter((entry) => !files.some((f) => f.path === entry.path))
      .map((entry) => ({
        id: crypto.randomUUID(),
        path: entry.path,
        name: entry.name,
        size: entry.size,
        status: "pending" as const,
      }));
    setFiles((prev) => [...prev, ...newFiles]);
  }, [files]);

  const removeFile = useCallback((id: string) => {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const clearFiles = useCallback(() => setFiles([]), []);

  const addLogEntry = useCallback((level: LogEntry["level"], message: string) => {
    setLogEntries((prev) => [
      ...prev,
      { id: crypto.randomUUID(), timestamp: new Date(), level, message },
    ]);
  }, []);

  const clearLog = useCallback(() => setLogEntries([]), []);

  const updateFileStatus = useCallback(
    (id: string, status: ShredFile["status"], error?: string) => {
      setFiles((prev) =>
        prev.map((f) => (f.id === id ? { ...f, status, error } : f))
      );
    },
    []
  );

  return (
    <ShredContext.Provider
      value={{
        files,
        algorithmIndex,
        isShredding,
        logEntries,
        algorithms,
        addFiles,
        removeFile,
        clearFiles,
        setAlgorithmIndex,
        setIsShredding,
        addLogEntry,
        clearLog,
        setAlgorithms,
        updateFileStatus,
      }}
    >
      {children}
    </ShredContext.Provider>
  );
}

export function useShred() {
  const ctx = useContext(ShredContext);
  if (!ctx) throw new Error("useShred must be used within ShredProvider");
  return ctx;
}
