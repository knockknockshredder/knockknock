// src/contexts/ShredContext.tsx
import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ShredFile,
  LogEntry,
  AlgorithmOption,
  ProgressState,
} from "@/types";

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  progress: ProgressState | null;
  vaultLoaded: boolean;
  addFiles: (files: Array<{ path: string; name: string; size: number }>) => void;
  removeFile: (id: string) => void;
  clearFiles: () => void;
  setAlgorithmIndex: (index: number) => void;
  setIsShredding: (v: boolean) => void;
  addLogEntry: (level: LogEntry["level"], message: string) => void;
  clearLog: () => void;
  setAlgorithms: (algorithms: AlgorithmOption[]) => void;
  setProgress: (progress: ProgressState | null) => void;
  updateFileStatus: (id: string, status: ShredFile["status"], error?: string) => void;
  loadVault: (pin: string) => Promise<void>;
  saveVault: (pin: string) => Promise<void>;
}

const ShredContext = createContext<ShredState | null>(null);

export function ShredProvider({ children }: { children: ReactNode }) {
  const [files, setFiles] = useState<ShredFile[]>([]);
  const [algorithmIndex, setAlgorithmIndex] = useState(0);
  const [isShredding, setIsShredding] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [algorithms, setAlgorithms] = useState<AlgorithmOption[]>([]);
  const [progress, setProgress] = useState<ProgressState | null>(null);
  const [vaultLoaded, setVaultLoaded] = useState(false);
  const [vaultPin, setVaultPin] = useState<string | null>(null);
  const skipNextSave = useRef(false);

  const addFiles = useCallback((newEntries: Array<{ path: string; name: string; size: number }>) => {
    setFiles((prev) => {
      const existingPaths = new Set(prev.map((f) => f.path));
      const newFiles: ShredFile[] = newEntries
        .filter((entry) => !existingPaths.has(entry.path))
        .map((entry) => ({
          id: crypto.randomUUID(),
          path: entry.path,
          name: entry.name,
          size: entry.size,
          status: "pending" as const,
        }));
      return [...prev, ...newFiles];
    });
  }, []);

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

  // Decrypt the on-disk vault with the user's PIN and rehydrate the file
  // list. Files that no longer exist on disk are silently dropped by
  // validate_paths. Failures (wrong PIN, corrupted vault) are logged but
  // do not block app startup — the user can keep adding files normally.
  // On success stores the PIN for future auto-saves.
  const loadVault = useCallback(async (pin: string) => {
    try {
      const exists = await invoke<boolean>("vault_exists");
      if (!exists) {
        setVaultLoaded(true);
        setVaultPin(pin);
        return;
      }
      const paths = await invoke<string[]>("load_vault", { pin });
      if (paths.length === 0) {
        setVaultLoaded(true);
        setVaultPin(pin);
        return;
      }
      const [validFiles] = await invoke<[FileMetadata[], string[]]>(
        "validate_paths",
        { paths }
      );
      // Skip the next auto-save on these files — they just came from the vault.
      skipNextSave.current = true;
      addFiles(validFiles);
      setVaultLoaded(true);
      setVaultPin(pin);
    } catch (err) {
      console.error("Failed to load vault:", err);
      addLogEntry("error", `Failed to restore session: ${err}`);
    }
  }, [addFiles, addLogEntry]);

  // Encrypt the current shred list and persist it. Best-effort: a save
  // failure is logged but never blocks the user's workflow.
  const saveVault = useCallback(
    async (pin: string) => {
      try {
        const paths = files.map((f) => f.path);
        await invoke("save_vault", { paths, pin });
      } catch (err) {
        console.error("Failed to save vault:", err);
        addLogEntry("error", `Failed to save session: ${err}`);
      }
    },
    [files, addLogEntry]
  );

  // Auto-save the vault whenever the file list changes (debounced).
  // Skips the first trigger after vault load to avoid re-saving data
  // that was just deserialized.
  useEffect(() => {
    if (!vaultLoaded || !vaultPin) return;
    if (skipNextSave.current) {
      skipNextSave.current = false;
      return;
    }
    const timer = setTimeout(() => {
      invoke("save_vault", { paths: files.map((f) => f.path), pin: vaultPin })
        .catch((err) => console.error("Auto-save vault failed:", err));
    }, 1000);
    return () => clearTimeout(timer);
  }, [files, vaultLoaded, vaultPin]);

  return (
    <ShredContext.Provider
      value={{
        files,
        algorithmIndex,
        isShredding,
        logEntries,
        algorithms,
        progress,
        vaultLoaded,
        addFiles,
        removeFile,
        clearFiles,
        setAlgorithmIndex,
        setIsShredding,
        addLogEntry,
        clearLog,
        setAlgorithms,
        setProgress,
        updateFileStatus,
        loadVault,
        saveVault,
      }}
    >
      {children}
    </ShredContext.Provider>
  );
}

/** Minimal shape returned by the Rust `validate_paths` command. */
interface FileMetadata {
  path: string;
  name: string;
  size: number;
}

export function useShred() {
  const ctx = useContext(ShredContext);
  if (!ctx) throw new Error("useShred must be used within ShredProvider");
  return ctx;
}