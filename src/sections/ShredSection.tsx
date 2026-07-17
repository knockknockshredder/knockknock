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
import { PinVerify } from "@/components/settings/PinVerify";
import type {
  ShredReport,
  ProgressEvent,
  ShredStatus,
  AlgorithmOption,
} from "@/types";

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
    progress,
    setProgress,
    saveVault,
  } = useShred();

  const { getSelectedCount, browsers } = useBrowser();
  const { defaultAlgorithmIndex, logObfuscation } = useSettings();

  const [dialogOpen, setDialogOpen] = useState(false);
  const [passes, setPasses] = useState(1);
  const [pattern, setPattern] = useState<"random" | "zeros" | "ones">("random");
  const [verificationLevel, setVerificationLevel] = useState<"none" | "sample" | "full">("sample");
  const unlistenRef = useRef<(() => void) | null>(null);

  // PIN verification gates
  const [pinNeeded, setPinNeeded] = useState(false);
  const [shredPinOpen, setShredPinOpen] = useState(false);
  const [cancelPinOpen, setCancelPinOpen] = useState(false);
  const [deferredShred, setDeferredShred] = useState<(() => void) | null>(null);

  // Check if PIN is enabled on mount
  useEffect(() => {
    invoke<boolean>("is_pin_enabled").then(setPinNeeded).catch(() => setPinNeeded(false));
  }, []);

  // Auto-save vault when files change after PIN is known
  const [pinForVault, setPinForVault] = useState("");
  useEffect(() => {
    if (pinForVault && files.length > 0) {
      const timer = setTimeout(() => {
        saveVault(pinForVault).catch(console.error);
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [files, pinForVault, saveVault]);

  const pendingFiles = files.filter((f) => f.status === "pending");
  const selectedProfileCount = getSelectedCount();
  const currentAlgorithm = algorithms[algorithmIndex];
  const runningBrowsers = browsers.filter((b) => b.isRunning).map((b) => b.name);

  // Load algorithms on mount and sync default from settings
  useEffect(() => {
    invoke<AlgorithmOption[]>("get_algorithms")
      .then((algorithms) => {
        setAlgorithms(algorithms);
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
    if (pinNeeded) {
      setDeferredShred(() => () => setDialogOpen(true));
      setShredPinOpen(true);
    } else {
      setDialogOpen(true);
    }
  };

  const executeShred = async () => {
    if (pendingFiles.length === 0 && selectedProfileCount === 0) return;

    setIsShredding(true);
    addLogEntry(
      "command",
      `shredding ${pendingFiles.length} file(s) and ${selectedProfileCount} browser profile(s)...`
    );

    // Listen for progress events
    const unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
      const { file_path, status, current_pass, total_passes } = event.payload;
      const statusStr = statusToString(status);
      const message =
        status.type === "Error"
          ? `[${file_path}] error: ${status.message}`
          : `[${file_path}] ${statusStr} (pass ${current_pass}/${total_passes})`;
      addLogEntry(status.type === "Error" ? "error" : "info", message);

      // Update progress state
      setProgress({
        current: pendingFiles.filter((f) => f.status === "done").length,
        total: pendingFiles.length,
        percent: Math.round((current_pass / total_passes) * 100),
        currentFile: file_path,
      });
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
        logObfuscation,
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

      // Shred browser profiles if any
      if (selectedProfileCount > 0) {
        const selectedProfiles = browsers.flatMap((b) =>
          b.profiles
            .filter((p) => p.selected)
            .map((p) => ({
              browser_name: b.name,
              profile_path: p.path,
              data_types: ["cache", "cookies", "history", "passwords"] as const,
            }))
        );

        for (const profile of selectedProfiles) {
          try {
            addLogEntry(
              "info",
              `Shredding ${profile.browser_name} profile: ${profile.profile_path}`
            );
            const browserReport: ShredReport = await invoke("shred_browser_data", {
              request: {
                browser_name: profile.browser_name,
                profile_path: profile.profile_path,
                data_types: profile.data_types,
                algorithm_index: algorithmIndex,
                passes: passes,
                pattern: pattern,
                verification_level: verificationLevel,
                explicit_consent: true,
              },
            });
            addLogEntry(
              "success",
              `${profile.browser_name}: ${browserReport.successful} files destroyed, ${browserReport.failed} failed`
            );
          } catch (err) {
            addLogEntry(
              "error",
              `Failed to shred ${profile.browser_name} profile: ${err}`
            );
          }
        }
      }
    } catch (err) {
      addLogEntry("error", `Shred failed: ${err}`);
      // Mark all pending as error
      for (const file of pendingFiles) {
        updateFileStatus(file.id, "error", String(err));
      }
    } finally {
      unlisten();
      unlistenRef.current = null;
      setProgress(null);
      setIsShredding(false);
    }
  };

  const handleCancel = async () => {
    if (pinNeeded) {
      setCancelPinOpen(true);
      return; // Shredding continues if PIN not entered
    }
    try {
      await invoke("cancel_shred");
      addLogEntry("warning", "Cancellation requested...");
    } catch (err) {
      addLogEntry("error", `Cancel failed: ${err}`);
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
          onCancel={handleCancel}
          progress={progress}
        />
      </div>
      <ConfirmationDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        fileCount={pendingFiles.length}
        profileCount={selectedProfileCount}
        runningBrowsers={runningBrowsers}
        onConfirm={executeShred}
      />
      <PinVerify
        open={shredPinOpen}
        onOpenChange={setShredPinOpen}
        onVerified={(pin) => {
          setPinForVault(pin);
          setShredPinOpen(false);
          deferredShred?.();
        }}
        purpose="shred"
      />
      <PinVerify
        open={cancelPinOpen}
        onOpenChange={setCancelPinOpen}
        onVerified={(_pin) => {
          setCancelPinOpen(false);
          invoke("cancel_shred").catch(() => {});
        }}
        purpose="cancel"
      />
    </div>
  );
}
