// src/contexts/ShredContext.tsx
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AlgorithmOption,
  FileMetadata,
  LogEntry,
  ProgressState,
  ShredFile,
  TargetMetadataDto,
  TargetKind,
  VaultSchemaSource,
  VaultTarget,
} from "@/types";

export type VaultState = "locked" | "loading" | "clean" | "dirty" | "saving" | "error";

interface VaultLoadDto {
  source_schema: VaultSchemaSource;
  migration_required: boolean;
  targets: VaultTarget[];
}

interface VaultSnapshot {
  readonly revision: number;
  readonly pinEpoch: number;
  readonly pin: string;
  readonly targets: readonly VaultTarget[];
}

interface WriterError {
  readonly revision: number;
  readonly pinEpoch: number;
  readonly error: Error;
}

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  progress: ProgressState | null;
  vaultLoaded: boolean;
  vaultPin: string | null;
  vaultState: VaultState;
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
  changeVaultPin: (oldPin: string, newPin: string) => Promise<void>;
  loadVault: (pin: string) => Promise<void>;
  flushVault: () => Promise<void>;
  saveVault: (pin: string) => Promise<boolean>;
}

const ShredContext = createContext<ShredState | null>(null);

function toError(reason: unknown): Error {
  return reason instanceof Error ? reason : new Error(String(reason));
}

function targetKindForFile(file: FileMetadata): TargetKind {
  return file.is_shortcut ? "link" : "file";
}

function readQueuedSnapshot(ref: { current: VaultSnapshot | null }): VaultSnapshot | null {
  return ref.current;
}

export function ShredProvider({ children }: { children: ReactNode }) {
  const [files, setFiles] = useState<ShredFile[]>([]);
  const [algorithmIndex, setAlgorithmIndex] = useState(0);
  const [isShredding, setIsShredding] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [algorithms, setAlgorithms] = useState<AlgorithmOption[]>([]);
  const [progress, setProgress] = useState<ProgressState | null>(null);
  const [vaultLoaded, setVaultLoaded] = useState(false);
  const [vaultPin, setVaultPinState] = useState<string | null>(null);
  const [vaultState, setVaultState] = useState<VaultState>("locked");

  const filesRef = useRef<ShredFile[]>([]);
  const targetKindsRef = useRef<Map<string, TargetKind>>(new Map());
  const suppressFileEffectRef = useRef(false);
  const vaultLoadedRef = useRef(false);
  const vaultPinRef = useRef<string | null>(null);
  const writerLockedRef = useRef(true);
  const loadingRef = useRef(false);
  const revisionRef = useRef(0);
  const persistedRevisionRef = useRef(0);
  const pinEpochRef = useRef(0);
  const queuedSnapshotRef = useRef<VaultSnapshot | null>(null);
  const writerPromiseRef = useRef<Promise<void> | null>(null);
  const writerErrorRef = useRef<WriterError | null>(null);
  const pinChangePromiseRef = useRef<Promise<void> | null>(null);

  const addFiles = useCallback((newEntries: FileMetadata[]) => {
    setFiles((previous) => {
      const existingPaths = new Set(previous.map((file) => file.path));
      const additions: ShredFile[] = [];
      for (const entry of newEntries) {
        if (existingPaths.has(entry.path)) continue;
        existingPaths.add(entry.path);
        targetKindsRef.current.set(entry.path, targetKindForFile(entry));
        additions.push({
          id: crypto.randomUUID(),
          path: entry.path,
          name: entry.name,
          size: entry.size,
          status: "pending",
          is_shortcut: entry.is_shortcut,
          shortcut_target: entry.shortcut_target,
        });
      }
      const next = [...previous, ...additions];
      filesRef.current = next;
      return next;
    });
  }, []);

  const removeFile = useCallback((id: string) => {
    setFiles((previous) => {
      const removed = previous.find((file) => file.id === id);
      if (removed) targetKindsRef.current.delete(removed.path);
      const next = previous.filter((file) => file.id !== id);
      filesRef.current = next;
      return next;
    });
  }, []);

  const clearFiles = useCallback(() => {
    targetKindsRef.current.clear();
    filesRef.current = [];
    setFiles([]);
  }, []);

  const addLogEntry = useCallback((level: LogEntry["level"], message: string) => {
    setLogEntries((previous) => [
      ...previous,
      { id: crypto.randomUUID(), timestamp: new Date(), level, message },
    ]);
  }, []);

  const clearLog = useCallback(() => setLogEntries([]), []);

  const updateFileStatus = useCallback(
    (id: string, status: ShredFile["status"], error?: string) => {
      setFiles((previous) => {
        const next = previous.map((file) =>
          file.id === id ? { ...file, status, error } : file
        );
        filesRef.current = next;
        return next;
      });
    },
    []
  );

  const createSnapshot = useCallback(
    (pin: string, pinEpoch: number, revision: number, source = filesRef.current) => {
      const targets = Object.freeze(
        source.map((file) => ({
          path: file.path,
          kind: targetKindsRef.current.get(file.path) ?? targetKindForFile(file),
        }))
      );
      return Object.freeze({ revision, pinEpoch, pin, targets });
    },
    []
  );

  const runWriter = useCallback(async () => {
    while (queuedSnapshotRef.current) {
      const snapshot = queuedSnapshotRef.current;
      queuedSnapshotRef.current = null;

      if (
        writerLockedRef.current ||
        snapshot.pinEpoch !== pinEpochRef.current ||
        snapshot.pin !== vaultPinRef.current
      ) {
        continue;
      }

      setVaultState("saving");
      try {
        await invoke<void>("save_vault", {
          targets: snapshot.targets.map(({ path, kind }) => ({ path, kind })),
          pin: snapshot.pin,
        });
      } catch (reason) {
        const error = toError(reason);
        if (snapshot.pinEpoch !== pinEpochRef.current) {
          continue;
        }
        const queued = readQueuedSnapshot(queuedSnapshotRef);
        const queuedRevision = queued?.revision;
        if (queuedRevision === undefined || queuedRevision < snapshot.revision) {
          queuedSnapshotRef.current = snapshot;
        }
        writerErrorRef.current = {
          revision: snapshot.revision,
          pinEpoch: snapshot.pinEpoch,
          error,
        };
        setVaultState("error");
        return;
      }

      if (snapshot.pinEpoch !== pinEpochRef.current) continue;

      persistedRevisionRef.current = Math.max(
        persistedRevisionRef.current,
        snapshot.revision
      );
      const writerError = writerErrorRef.current;
      if (
        writerError &&
        writerError.pinEpoch === snapshot.pinEpoch &&
        writerError.revision <= snapshot.revision
      ) {
        writerErrorRef.current = null;
      }

      if (queuedSnapshotRef.current) continue;
      setVaultState(
        persistedRevisionRef.current >= revisionRef.current ? "clean" : "dirty"
      );
    }
  }, []);

  const startWriter = useCallback(
    (forceRetry = false): Promise<void> => {
      if (writerPromiseRef.current) return writerPromiseRef.current;
      const queued = queuedSnapshotRef.current;
      if (!queued) return Promise.resolve();

      const writerError = writerErrorRef.current;
      if (
        !forceRetry &&
        writerError &&
        writerError.pinEpoch === queued.pinEpoch &&
        queued.revision <= writerError.revision
      ) {
        return Promise.resolve();
      }

      let tracked!: Promise<void>;
      tracked = runWriter().finally(() => {
        if (writerPromiseRef.current === tracked) {
          writerPromiseRef.current = null;
        }
      });
      writerPromiseRef.current = tracked;
      return tracked;
    },
    [runWriter]
  );

  const queueSnapshot = useCallback(
    (snapshot: VaultSnapshot, forceRetry = false) => {
      if (
        writerLockedRef.current ||
        snapshot.pinEpoch !== pinEpochRef.current ||
        snapshot.pin !== vaultPinRef.current
      ) {
        return;
      }

      const queued = queuedSnapshotRef.current;
      if (
        !queued ||
        queued.pinEpoch !== snapshot.pinEpoch ||
        queued.revision <= snapshot.revision
      ) {
        queuedSnapshotRef.current = snapshot;
      }

      if (!writerPromiseRef.current) {
        const writerError = writerErrorRef.current;
        if (
          forceRetry ||
          !writerError ||
          snapshot.revision > writerError.revision
        ) {
          void startWriter(forceRetry);
        }
      }
    },
    [startWriter]
  );

  const flushRevision = useCallback(
    async (pinEpoch: number, observedRevision: number) => {
      if (pinEpoch !== pinEpochRef.current) {
        throw new Error("Vault PIN epoch changed before flush completed");
      }
      if (
        writerLockedRef.current ||
        !vaultLoadedRef.current ||
        !vaultPinRef.current
      ) {
        throw new Error("Vault writer is locked");
      }
      if (
        persistedRevisionRef.current >= observedRevision &&
        !writerErrorRef.current
      ) {
        return;
      }

      const queued = queuedSnapshotRef.current;
      if (
        !queued ||
        queued.pinEpoch !== pinEpoch ||
        queued.revision < observedRevision
      ) {
        queueSnapshot(
          createSnapshot(
            vaultPinRef.current,
            pinEpoch,
            observedRevision,
            filesRef.current
          ),
          true
        );
      }

      await startWriter(true);
      if (pinEpoch !== pinEpochRef.current) {
        throw new Error("Vault PIN epoch changed during flush");
      }
      if (
        writerLockedRef.current ||
        !vaultLoadedRef.current ||
        !vaultPinRef.current
      ) {
        throw new Error("Vault writer became locked during flush");
      }
      if (persistedRevisionRef.current < observedRevision) {
        throw (
          writerErrorRef.current?.error ??
          new Error("Vault revision was not persisted")
        );
      }
    },
    [createSnapshot, queueSnapshot, startWriter]
  );

  const setVaultPin = useCallback(
    (pin: string | null) => {
      if (pinChangePromiseRef.current) return;
      if (pin === vaultPinRef.current && !loadingRef.current) return;

      pinEpochRef.current += 1;
      const pinEpoch = pinEpochRef.current;
      vaultPinRef.current = pin;
      setVaultPinState(pin);
      queuedSnapshotRef.current = null;
      writerErrorRef.current = null;

      if (pin === null) {
        writerLockedRef.current = true;
        vaultLoadedRef.current = false;
        setVaultLoaded(false);
        setVaultState("locked");
        return;
      }

      if (vaultLoadedRef.current && !loadingRef.current) {
        writerLockedRef.current = false;
        const revision = ++revisionRef.current;
        setVaultState("dirty");
        queueSnapshot(createSnapshot(pin, pinEpoch, revision), true);
      } else {
        writerLockedRef.current = true;
      }
    },
    [createSnapshot, queueSnapshot]
  );

  const changeVaultPin = useCallback(
    (oldPin: string, newPin: string): Promise<void> => {
      if (pinChangePromiseRef.current) return pinChangePromiseRef.current;

      let tracked!: Promise<void>;
      tracked = (async () => {
        if (
          oldPin !== vaultPinRef.current ||
          writerLockedRef.current ||
          !vaultLoadedRef.current
        ) {
          throw new Error("Vault PIN change requires an unlocked vault");
        }

        const oldEpoch = pinEpochRef.current;
        const observedRevision = ++revisionRef.current;
        queueSnapshot(
          createSnapshot(oldPin, oldEpoch, observedRevision),
          true
        );
        await flushRevision(oldEpoch, observedRevision);
        if (
          oldEpoch !== pinEpochRef.current ||
          oldPin !== vaultPinRef.current
        ) {
          throw new Error("Vault PIN changed before rekey could begin");
        }

        writerLockedRef.current = true;
        pinEpochRef.current += 1;
        const barrierEpoch = pinEpochRef.current;
        queuedSnapshotRef.current = null;
        writerErrorRef.current = null;
        setVaultState("saving");

        try {
          await invoke<void>("change_pin", { oldPin, newPin });
          if (
            barrierEpoch !== pinEpochRef.current ||
            vaultPinRef.current !== oldPin
          ) {
            throw new Error("Vault PIN changed during rekey");
          }

          vaultPinRef.current = newPin;
          setVaultPinState(newPin);
          writerLockedRef.current = false;
          const postRevision = ++revisionRef.current;
          setVaultState("dirty");
          queueSnapshot(
            createSnapshot(newPin, barrierEpoch, postRevision),
            true
          );
          await flushRevision(barrierEpoch, postRevision);
          setVaultState("clean");
        } catch (reason) {
          if (
            barrierEpoch === pinEpochRef.current &&
            vaultPinRef.current === oldPin
          ) {
            writerLockedRef.current = false;
            const recoveryRevision = ++revisionRef.current;
            setVaultState("dirty");
            queueSnapshot(
              createSnapshot(oldPin, barrierEpoch, recoveryRevision),
              true
            );
          }
          throw toError(reason);
        }
      })().finally(() => {
        if (pinChangePromiseRef.current === tracked) {
          pinChangePromiseRef.current = null;
        }
      });
      pinChangePromiseRef.current = tracked;
      return tracked;
    },
    [createSnapshot, flushRevision, queueSnapshot]
  );

  const replaceLoadedFiles = useCallback((metadata: TargetMetadataDto[]) => {
    const next = metadata.map((entry) => ({
      id: crypto.randomUUID(),
      path: entry.path,
      name: entry.name,
      size: entry.size,
      status: entry.availability === "ready" ? ("pending" as const) : ("error" as const),
      error:
        entry.availability === "ready"
          ? undefined
          : entry.reason ?? `Target is ${entry.availability}`,
      is_shortcut: entry.kind === "link",
      shortcut_target: null,
    }));
    targetKindsRef.current = new Map(
      metadata.map((entry) => [entry.path, entry.kind])
    );
    filesRef.current = next;
    suppressFileEffectRef.current = true;
    setFiles(next);
  }, []);

  const loadVault = useCallback(
    async (pin: string) => {
      pinEpochRef.current += 1;
      const pinEpoch = pinEpochRef.current;
      writerLockedRef.current = true;
      loadingRef.current = true;
      queuedSnapshotRef.current = null;
      writerErrorRef.current = null;
      vaultLoadedRef.current = false;
      setVaultLoaded(false);
      setVaultPinState(null);
      setVaultState("loading");

      try {
        const exists = await invoke<boolean>("vault_exists");
        if (pinEpoch !== pinEpochRef.current) return;

        if (!exists) {
          replaceLoadedFiles([]);
          const revision = ++revisionRef.current;
          persistedRevisionRef.current = revision;
          vaultPinRef.current = pin;
          vaultLoadedRef.current = true;
          writerLockedRef.current = false;
          loadingRef.current = false;
          setVaultPinState(pin);
          setVaultLoaded(true);
          setVaultState("clean");
          return;
        }

        const loaded = await invoke<VaultLoadDto>("load_vault", { pin });
        if (pinEpoch !== pinEpochRef.current) return;

        const validated = await invoke<TargetMetadataDto[]>("validate_targets", {
          targets: loaded.targets,
        });
        if (pinEpoch !== pinEpochRef.current) return;
        if (validated.length !== loaded.targets.length) {
          throw new Error("Vault validation returned an incomplete target set");
        }

        replaceLoadedFiles(validated);
        const revision = ++revisionRef.current;
        vaultPinRef.current = pin;
        vaultLoadedRef.current = true;
        writerLockedRef.current = false;
        loadingRef.current = false;
        setVaultPinState(pin);
        setVaultLoaded(true);

        if (loaded.source_schema === "v1") {
          setVaultState("dirty");
          queueSnapshot(
            createSnapshot(pin, pinEpoch, revision, filesRef.current),
            true
          );
          await flushRevision(pinEpoch, revision);
          if (pinEpoch !== pinEpochRef.current) return;
        } else {
          persistedRevisionRef.current = revision;
        }
        setVaultState("clean");
      } catch (reason) {
        if (pinEpoch !== pinEpochRef.current) return;
        const error = toError(reason);
        writerLockedRef.current = true;
        loadingRef.current = false;
        vaultLoadedRef.current = false;
        vaultPinRef.current = null;
        setVaultLoaded(false);
        setVaultPinState(null);
        setVaultState("error");
        addLogEntry("error", `Failed to restore session: ${error.message}`);
        throw error;
      }
    },
    [addLogEntry, createSnapshot, flushRevision, queueSnapshot, replaceLoadedFiles]
  );

  const flushVault = useCallback(
    () => flushRevision(pinEpochRef.current, revisionRef.current),
    [flushRevision]
  );

  const saveVault = useCallback(
    async (pin: string) => {
      if (
        pin !== vaultPinRef.current ||
        writerLockedRef.current ||
        !vaultLoadedRef.current
      ) {
        return false;
      }

      const revision = ++revisionRef.current;
      queueSnapshot(
        createSnapshot(pin, pinEpochRef.current, revision),
        true
      );
      try {
        await flushRevision(pinEpochRef.current, revision);
        return true;
      } catch (reason) {
        addLogEntry("error", `Failed to save session: ${toError(reason).message}`);
        return false;
      }
    },
    [addLogEntry, createSnapshot, flushRevision, queueSnapshot]
  );

  useEffect(() => {
    filesRef.current = files;
    if (suppressFileEffectRef.current) {
      suppressFileEffectRef.current = false;
      return;
    }
    if (
      loadingRef.current ||
      writerLockedRef.current ||
      !vaultLoadedRef.current ||
      !vaultPinRef.current ||
      isShredding
    ) {
      return;
    }

    const revision = ++revisionRef.current;
    queueSnapshot(
      createSnapshot(vaultPinRef.current, pinEpochRef.current, revision, files),
      false
    );
  }, [createSnapshot, files, isShredding, queueSnapshot]);

  useEffect(() => {
    const hasFiles = files.length > 0;
    invoke("sync_tray_state", { hasFiles, isShredding }).catch((err) => {
      console.debug("[tray] sync_tray_state failed:", err);
    });
  }, [files, isShredding]);

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
        vaultState,
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
        changeVaultPin,
        loadVault,
        flushVault,
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
