// src/sections/ShredSection.tsx
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ShredButton } from "@/components/shred/ShredButton";
import { DeletionMethodSelector } from "@/components/shred/DeletionMethodSelector";
import { WriteCheckSelector } from "@/components/shred/WriteCheckSelector";
import { ConfirmationDialog } from "@/components/shred/ConfirmationDialog";
import { useShred } from "@/contexts/ShredContext";
import { useBrowser } from "@/contexts/BrowserContext";
import { useSettings } from "@/contexts/SettingsContext";
import { PinVerify } from "@/components/settings/PinVerify";
import type {
  BatchRootResult,
  ProgressEvent,
  ShredReport,
  ShredStatus,
} from "@/types";

function statusToString(status: ShredStatus): string {
  return status.type.toLowerCase();
}

export function ShredSection() {
  const {
    files,
    deletionMethod,
    writeCheck,
    isShredding,
    setIsShredding,
    addLogEntry,
    clearLog,
    updateFileStatus,
    progress,
    setProgress,
    vaultPin,
    setVaultPin,
    flushVault,
    buildExecuteRootsRequest,
    applyRootResults,
  } = useShred();

  const { getSelectedCount, browsers } = useBrowser();
  const { logObfuscation, autoClearLog } = useSettings();

  const [dialogOpen, setDialogOpen] = useState(false);
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
  const pendingFileCount = pendingFiles.filter(
    (f) => f.kind !== "directory"
  ).length;
  const pendingFolderCount = pendingFiles.filter(
    (f) => f.kind === "directory"
  ).length;
  const selectedProfileCount = getSelectedCount();
  const runningBrowsers = browsers.filter((b) => b.isRunning).map((b) => b.name);

  // Transitional IPC shim (removed when the v2 contract lands): map the
  // policy state to the legacy execute_roots argument surface. The engine
  // derives its pass plan from the policy, so `passes`/`pattern` are
  // placeholders that are validated for bounds and otherwise ignored.
  const algorithmIndex = deletionMethod === "legacy_three_pass" ? 1 : 0;
  const verificationLevel =
    writeCheck === "off" ? "none" : writeCheck === "full" ? "full" : "sample";

  // Handle tray menu "Quick Shred" — triggers the same PIN→confirmation
  // flow as clicking the Shred button, using the existing executeShred
  // pipeline so there is no second code path for the destructive work.
  useEffect(() => {
    const unlistenPromise = listen("quick-shred-request", () => {
      // Nothing to shred — notify instead of opening an empty dialog.
      // The window was already shown and focused by the tray action.
      if (pendingFiles.length === 0 && selectedProfileCount === 0) {
        invoke("send_notification", {
          title: "KnockKnock",
          body: "No targets selected",
        }).catch(() => {});
        return;
      }

      if (pinNeeded) {
        setDeferredShred(() => () => setDialogOpen(true));
        setShredPinOpen(true);
      } else {
        setDialogOpen(true);
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [pinNeeded, pendingFiles.length, selectedProfileCount]);

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
    // true, so this explicit flush is the final checkpoint. If the flush
    // fails we MUST abort — proceeding would shred the files without a
    // recoverable session backup.
    if (vaultPin) {
      try {
        await flushVault();
      } catch (err) {
        addLogEntry("error", `Refusing to shred: vault save failed: ${String(err)}`);
        setIsShredding(false);
        isExecutingRef.current = false;
        return;
      }
    }
    const request = buildExecuteRootsRequest();
    addLogEntry(
      "command",
      `Processing ${pendingFileCount} file(s), ${pendingFolderCount} folder(s), and ${selectedProfileCount} browser profile(s)...`
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
      const report: BatchRootResult = await invoke<BatchRootResult>("execute_roots", {
        request,
        algorithmIndex,
        passes: 1,
        pattern: "random",
        verificationLevel,
        logObfuscation,
      });

      // Apply typed per-root results: destroyed roots with root_removed are
      // removed from the list, everything else is retained with error details.
      await applyRootResults(report.roots);

      const removed = report.roots.filter(
        (root) => root.status === "destroyed" && root.root_removed
      ).length;
      const failed = report.roots.filter((root) => root.status === "failed").length;
      const cancelled = report.roots.filter((root) => root.status === "cancelled").length;
      const skipped = report.roots.filter((root) => root.status === "skipped").length;

      addLogEntry(
        "success",
        `Complete: ${removed} removed, ${failed} failed, ${cancelled} cancelled, ${skipped} skipped`
      );

      if (autoClearLog && failed === 0) {
        clearLog();
      }

      // Send system notification for the main shred result.
      invoke("send_notification", {
        title: "Deletion Complete",
        body: `${removed} removed, ${failed} failed, ${cancelled} cancelled, ${skipped} skipped`,
      }).catch(() => {});

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
              `Deleting selected local data from ${profile.browser_name} profile: ${profile.profile_path}`
            );
            const browserReport: ShredReport = await invoke<ShredReport>("shred_browser_data", {
              request: {
                browser_name: profile.browser_name,
                profile_path: profile.profile_path,
                data_types: profile.data_types,
                algorithm_index: algorithmIndex,
                passes: 1,
                pattern: "random",
                verification_level: verificationLevel,
                explicit_consent: true,
              },
            });
            addLogEntry(
              "success",
              `${profile.browser_name}: ${browserReport.successful} files processed, ${browserReport.failed} failed`
            );
          } catch (err) {
            addLogEntry(
              "error",
              `Failed to clean ${profile.browser_name} profile: ${err}`
            );
            invoke("send_notification", {
              title: "Browser Cleanup Failed",
              body: `${profile.browser_name}: ${err}`,
            }).catch(() => {});
          }
        }
      }
    } catch (err) {
      addLogEntry("error", `Deletion failed: ${err}`);
      // Mark all pending as error
      for (const file of pendingFiles) {
        updateFileStatus(file.id, "error", String(err));
      }
      invoke("send_notification", {
        title: "Deletion Failed",
        body: `${String(err).slice(0, 200)}`,
      }).catch(() => {});
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
      await invoke<void>("cancel_shred");
      addLogEntry("warning", "Stop requested. Already processed targets will not be restored.");
    } catch (err) {
      addLogEntry("error", `Stop request failed: ${err}`);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-mono text-xl font-semibold tracking-tight">
  Local Data Deletion
</h1>
      <div className="flex flex-col gap-4 w-full max-w-lg mx-auto">
        <DeletionMethodSelector />
        <WriteCheckSelector />
        <ShredButton
          fileCount={pendingFileCount}
          folderCount={pendingFolderCount}
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
        fileCount={pendingFileCount}
        folderCount={pendingFolderCount}
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
          invoke<void>("cancel_shred").catch((err) =>
            addLogEntry("error", `Failed to stop operation: ${err}`)
          );
        }}
        purpose="cancel"
      />
    </div>
  );
}
