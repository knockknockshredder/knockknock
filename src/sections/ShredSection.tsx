// src/sections/ShredSection.tsx
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ShredButton } from "@/components/shred/ShredButton";
import { AlgorithmSelector } from "@/components/shred/AlgorithmSelector";
import { ShredOptions } from "@/components/shred/ShredOptions";
import { ConfirmationDialog } from "@/components/shred/ConfirmationDialog";
import { useShred } from "@/contexts/ShredContext";
import { useBrowser } from "@/contexts/BrowserContext";
import { useSettings } from "@/contexts/SettingsContext";
import type { ShredReport, ProgressEvent, ShredStatus } from "@/types";

function statusToString(status: ShredStatus): string {
  return status.type.toLowerCase();
}

export function ShredSection() {
  const {
    files,
    algorithmIndex,
    setAlgorithmIndex,
    isShredding,
    setIsShredding,
    addLogEntry,
    updateFileStatus,
    setAlgorithms,
    algorithms,
  } = useShred();

  const { getSelectedCount } = useBrowser();
  const { defaultAlgorithmIndex } = useSettings();

  const [dialogOpen, setDialogOpen] = useState(false);
  const [passes, setPasses] = useState(1);
  const [pattern, setPattern] = useState<"random" | "zeros" | "ones">("random");
  const [verificationLevel, setVerificationLevel] = useState<"none" | "sample" | "full">("sample");
  const unlistenRef = useRef<(() => void) | null>(null);

  const pendingFiles = files.filter((f) => f.status === "pending");
  const selectedProfileCount = getSelectedCount();
  const currentAlgorithm = algorithms[algorithmIndex];

  // Load algorithms on mount and sync default from settings
  useEffect(() => {
    invoke<ShredReport[]>("get_algorithms")
      .then((algorithms) => {
        setAlgorithms(algorithms as any);
        // Apply default algorithm from settings
        if (defaultAlgorithmIndex > 0 && defaultAlgorithmIndex < algorithms.length) {
          setAlgorithmIndex(defaultAlgorithmIndex);
        }
      })
      .catch((err) => addLogEntry("error", `Failed to load algorithms: ${err}`));
  }, [setAlgorithms, addLogEntry, defaultAlgorithmIndex, setAlgorithmIndex]);

  // Cleanup progress listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  const handleShredClick = () => {
    setDialogOpen(true);
  };

  const executeShred = async () => {
    if (pendingFiles.length === 0) return;

    setIsShredding(true);
    addLogEntry("command", `shredding ${pendingFiles.length} file(s)...`);

    // Listen for progress events
    const unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
      const { file_path, status, current_pass, total_passes } = event.payload;
      const statusStr = statusToString(status);
      const message =
        status.type === "Error"
          ? `[${file_path}] error: ${status.message}`
          : `[${file_path}] ${statusStr} (pass ${current_pass}/${total_passes})`;
      addLogEntry(status.type === "Error" ? "error" : "info", message);
    });
    unlistenRef.current = unlisten;

    try {
      const paths = pendingFiles.map((f) => f.path);
      const report: ShredReport = await invoke("shred_files", {
        paths,
        algorithmIndex,
        passes,
        pattern,
        verificationLevel,
      });

      // Map report errors to per-file status
      const failedPaths = new Set(report.errors.map((e) => e.path));
      for (const file of pendingFiles) {
        if (failedPaths.has(file.path)) {
          const errorEntry = report.errors.find((e) => e.path === file.path);
          updateFileStatus(file.id, "error", errorEntry?.error ?? "Unknown error");
        } else {
          updateFileStatus(file.id, "done");
        }
      }

      addLogEntry(
        "success",
        `Complete: ${report.successful} destroyed, ${report.failed} failed, ${report.skipped} skipped (${report.duration_secs.toFixed(1)}s)`
      );
    } catch (err) {
      addLogEntry("error", `Shred failed: ${err}`);
      // Mark all pending as error
      for (const file of pendingFiles) {
        updateFileStatus(file.id, "error", String(err));
      }
    } finally {
      unlisten();
      unlistenRef.current = null;
      setIsShredding(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Shred Files</h1>
      <div className="flex flex-col gap-4 w-full max-w-lg">
        <AlgorithmSelector />
        {currentAlgorithm && (
          <ShredOptions
            passes={passes}
            onPassesChange={setPasses}
            pattern={pattern}
            onPatternChange={setPattern}
            verificationLevel={verificationLevel}
            onVerificationLevelChange={setVerificationLevel}
            maxPasses={currentAlgorithm.max_passes}
            currentAlgorithm={currentAlgorithm}
          />
        )}
        <ShredButton
          fileCount={pendingFiles.length}
          profileCount={selectedProfileCount}
          isShredding={isShredding}
          onClick={handleShredClick}
        />
      </div>
      <ConfirmationDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        fileCount={pendingFiles.length}
        profileCount={selectedProfileCount}
        onConfirm={executeShred}
      />
    </div>
  );
}
