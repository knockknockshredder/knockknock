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
  FileMetadata,
} from "@/types";

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  progress: ProgressState | null;
  vaultLoaded: boolean;
  vaultPin: string | null;
  addFiles: (files: FileMetadata[]) => void;
  removeFile: (id: string) => void;
  clearFiles: () => void;
  setAlgorithmIndex: (index: number) => void;
  setIsShredding: (v: boolean) => void;
  addLogEntry: (level: LogEntry["level"], message: string) => void;
  clearLog: () => void;
  setAlgorithms: (algorithms: AlgorithmOption[]) => void;
  setProgress: (progress: ProgressState | null) => void;
  updateFileStatus: (id: string, status: ShredFile["status"], error?: string) => void;
  setVaultPin: (pin: string | null) => void;
  loadVault: (pin: string) => Promise<void>;
  saveVault: (pin: string) => Promise<boolean>;
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
  const lastLoadCompletedAt = useRef<number>(0);

  const addFiles = useCallback((newEntries: FileMetadata[]) => {
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
          is_shortcut: entry.is_shortcut,
          shortcut_target: entry.shortcut_target,
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
    // Tracks whether we got past the initial `vault_exists` probe so the
    // finally block can safely mark the vault as loaded. If the probe
    // itself fails (e.g. IPC error), we leave vaultLoaded false — the
    // auto-save effect will not fire until a future successful load.
    let pastExistsCheck = false;
    try {
      const exists = await invoke<boolean>("vault_exists");
      pastExistsCheck = true;
      console.debug(
        "[vault] loadVault: vault_exists=%s",
        exists,
      );
      if (!exists) {
        console.debug("[vault] no vault on disk — marking loaded");
        return;
      }
      const paths = await invoke<string[]>("load_vault", { pin });
      console.debug(
        "[vault] load_vault returned %d paths",
        paths.length,
      );
      if (paths.length === 0) return;
      const [validFiles] = await invoke<[FileMetadata[], string[]]>(
        "validate_paths",
        { paths }
      );
      console.debug(
        "[vault] validated %d valid files",
        validFiles.length,
      );
      // Stamps the completion time so the auto-save effect can suppress
      // its first run after a vault load — the files just came from disk
      // and don't need to be re-encrypted immediately.
      lastLoadCompletedAt.current = Date.now();
      addFiles(validFiles);
    } catch (err) {
      console.error("[vault] load failed:", err);
      addLogEntry("error", `Failed to restore session: ${err}`);
    } finally {
      if (pastExistsCheck) {
        setVaultLoaded(true);
        setVaultPin(pin);
      }
    }
  }, [addFiles, addLogEntry]);

  // Encrypt the current shred list and persist it. Returns `true` on
  // success and `false` on failure (after logging the error). Callers
  // performing destructive operations (e.g. the shred pipeline) must
  // check the return value — a failed auto-save is non-fatal, but a
  // failed pre-shred checkpoint is a hard abort to avoid data loss.
  const saveVault = useCallback(
    async (pin: string) => {
      try {
        const paths = files.map((f) => f.path);
        console.debug(
          "[vault] saving %d paths to vault",
          paths.length,
        );
        await invoke<void>("save_vault", { paths, pin });
        console.debug("[vault] save succeeded");
        return true;
      } catch (err) {
        console.error("[vault] save failed:", err);
        addLogEntry("error", `Failed to save session: ${err}`);
        return false;
      }
    },
    [files, addLogEntry]
  );

  // Auto-save the vault whenever the file list changes (debounced).
  // Skips the first trigger within 500ms of a vault load to avoid
  // re-saving data that was just deserialized. Suppressed during an
  // active shred (status churn would otherwise trigger a save storm).
  useEffect(() => {
    if (!vaultLoaded || !vaultPin || isShredding) {
      console.debug(
        "[vault] auto-save skipped: vaultLoaded=%s vaultPin=%s isShredding=%s",
        vaultLoaded,
        !!vaultPin,
        isShredding,
      );
      return;
    }
    if (Date.now() - lastLoadCompletedAt.current < 500) {
      console.debug("[vault] auto-save skipped: within load cooldown");
      return;
    }
    console.debug(
      "[vault] auto-save scheduled: %d files",
      files.length,
    );
    // Capture the PIN at effect entry. The setTimeout fires after a 1s
    // debounce; if the user changed PIN during that window the effect
    // was cancelled (cleanup below) AND the latest vaultPin will
    // differ from this snapshot — the re-check at fire time suppresses
    // the stale-PIN save.
    const pinSnapshot = vaultPin;
    const timer = setTimeout(() => {
      if (pinSnapshot === vaultPin) {
        void saveVault(vaultPin);
      }
    }, 1000);
    return () => clearTimeout(timer);
  }, [files, vaultLoaded, vaultPin, isShredding, saveVault]);

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
        vaultPin,
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
        setVaultPin,
        loadVault,
        saveVault,
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