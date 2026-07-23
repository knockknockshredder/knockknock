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
    isShredding,
    setIsShredding,
    addLogEntry,
    clearLog,
    updateFileStatus,
    setAlgorithms,
    algorithms,
    progress,
    setProgress,
    vaultPin,
    setVaultPin,
    saveVault,
  } = useShred();

  const { getSelectedCount, browsers } = useBrowser();
  const { logObfuscation, autoClearLog } = useSettings();

  const [dialogOpen, setDialogOpen] = useState(false);
  const [passes, setPasses] = useState(1);
  const [pattern, setPattern] = useState<"random" | "zeros" | "ones">("random");
  const [verificationLevel, setVerificationLevel] = useState<"none" | "sample" | "full">("sample");
  const [shredTargets, setShredTargets] = useState(false);
  const unlistenRef = useRef<(() => void) | null>(null);
  const isExecutingRef = useRef(false); // guards against StrictMode double-fire
  const completedCountRef = useRef(0);

  // PIN verification gates
  const [pinNeeded, setPinNeeded] = useState(false);
  const [shredPinOpen, setShredPinOpen] = useState(false);
  const [cancelPinOpen, setCancelPinOpen] = useState(false);
  const [deferredShred, setDeferredShred] = useState<(() => void) | null>(null);

  // Check if PIN is enabled on mount
  useEffect(() => {
    invoke<boolean>("is_pin_enabled")
      .then(setPinNeeded)
      .catch(() => setPinNeeded(true));  // fail closed — assume PIN required
  }, []);

  const pendingFiles = files.filter((f) => f.status === "pending");
  const selectedProfileCount = getSelectedCount();
  const currentAlgorithm = algorithms[algorithmIndex];
  const runningBrowsers = browsers.filter((b) => b.isRunning).map((b) => b.name);
  // Count of shortcut/symlink entries (pending or otherwise). Drives the
  // visibility of the "Also shred linked targets" opt-in checkbox.
  const shortcutCount = files.filter((f) => f.is_shortcut).length;

  // Load algorithms on mount and sync default from settings
  useEffect(() => {
    invoke<AlgorithmOption[]>("get_algorithms")
      .then((algorithms) => {
        setAlgorithms(algorithms);
      })
      .catch((err) => addLogEntry("error", `Failed to load algorithms: ${err}`));
  }, [setAlgorithms, addLogEntry]);

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
    if (isExecutingRef.current) return;
    if (pendingFiles.length === 0 && selectedProfileCount === 0) return;

    isExecutingRef.current = true;
    setIsShredding(true);
    // Persist the pending shred list one last time before the destructive
    // operation. The auto-save effect is suppressed while isShredding is
    // true, so this explicit call is the final checkpoint. If the save
    // fails we MUST abort — proceeding would shred the files without a
    // recoverable session backup.
    if (vaultPin) {
      const ok = await saveVault(vaultPin);
      if (!ok) {
        addLogEntry("error", "Refusing to shred: vault save failed");
        setIsShredding(false);
        isExecutingRef.current = false;
        return;
      }
    }
    addLogEntry(
      "command",
      `shredding ${pendingFiles.length} file(s) and ${selectedProfileCount} browser profile(s)...`
    );

    // Reset completed count before listening
    completedCountRef.current = 0;

    // Listen for progress events
    const unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
      const { file_path, status, current_pass, total_passes } = event.payload;
      const statusStr = statusToString(status);
      const message =
        status.type === "Error"
          ? `[${file_path}] error: ${status.message}`
          : `[${file_path}] ${statusStr} (pass ${current_pass}/${total_passes})`;
      addLogEntry(status.type === "Error" ? "error" : "info", message);

      if (status.type === "Complete") {
        completedCountRef.current += 1;
      }

      // Update progress state
      setProgress({
        current: completedCountRef.current,
        total: pendingFiles.length,
        percent: Math.round((current_pass / total_passes) * 100),
        currentFile: file_path,
      });
    });
    unlistenRef.current = unlisten;

    try {
      const paths = pendingFiles.map((f) => f.path);
      const report: ShredReport = await invoke<ShredReport>("shred_files", {
        paths,
        algorithmIndex,
        passes,
        pattern,
        verificationLevel,
        logObfuscation,
        shredTargets,
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

      if (autoClearLog && report.failed === 0) {
        clearLog();
      }

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
            const browserReport: ShredReport = await invoke<ShredReport>("shred_browser_data", {
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
      isExecutingRef.current = false;
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
      <h1 className="font-mono text-xl font-semibold tracking-tight">Someone's KnockKnock'ing...</h1>
      <div className="flex flex-col gap-4 w-full max-w-lg mx-auto">
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
        {shortcutCount > 0 && (
          <label className="flex items-center gap-2 text-sm text-amber-400 mt-2">
            <input
              type="checkbox"
              checked={shredTargets}
              onChange={(e) => setShredTargets(e.target.checked)}
            />
            Also shred linked targets ({shortcutCount})
          </label>
        )}
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
          setVaultPin(pin);
          setShredPinOpen(false);
          deferredShred?.();
        }}
        purpose="shred"
      />
      <PinVerify
        open={cancelPinOpen}
        onOpenChange={setCancelPinOpen}
        onVerified={(pin) => {
          setVaultPin(pin);
          setCancelPinOpen(false);
          invoke("cancel_shred").catch((err) =>
            addLogEntry("error", `Failed to cancel shred: ${err}`)
          );
        }}
        purpose="cancel"
      />
    </div>
  );
}
