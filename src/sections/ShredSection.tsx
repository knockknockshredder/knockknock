// src/sections/ShredSection.tsx
import { useState, useEffect, useRef, useCallback } from "react";
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
  RootResultDto,
  ShredStatus,
} from "@/types";

function statusToString(status: ShredStatus): string {
  return status.type.toLowerCase();
}

/**
 * Pass-local progress percent (M5): completed passes plus the fraction of
 * the current pass, divided by the total pass count. Guarded so
 * `total_passes < 1` yields 0 and the result is clamped to 0..=100; a
 * zero-length file (file_size 0) contributes no fraction.
 */
export function computeProgressPercent(
  currentPass: number,
  totalPasses: number,
  bytesInPass: number,
  fileSize: number
): number {
  if (totalPasses < 1) return 0;
  const completedPasses = Math.max(0, currentPass - 1);
  const passFraction = fileSize > 0 ? Math.min(1, bytesInPass / fileSize) : 0;
  const percent = ((completedPasses + passFraction) / totalPasses) * 100;
  return Math.min(100, Math.max(0, percent));
}

/**
 * A root whose final write check did not pass: either the aggregate outcome
 * is `failed`, or a child error was reported at the `verify` stage. Such
 * roots are not a clean success — the frontend renders a warning instead of
 * a plain "Success" (M3).
 */
function isWriteCheckFailure(root: RootResultDto): boolean {
  return (
    root.write_check === "failed" ||
    root.errors.some((error) => error.stage === "verify")
  );
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

  const { getSelectedCount, browsers, rescanBrowsers } = useBrowser();
  const { logObfuscation, autoClearLog } = useSettings();

  const [dialogOpen, setDialogOpen] = useState(false);
  const unlistenRef = useRef<(() => void) | null>(null);
  const isExecutingRef = useRef(false); // guards against StrictMode double-fire
  const completedCountRef = useRef(0);
  // Honest consent flag (M10, ora-2 amendment 5): set ONLY by the DELETE
  // action on the confirmation dialog. Never derived from a stale browser
  // scan, never unconditional. The backend lock check remains authoritative.
  const dialogConfirmedRef = useRef(false);

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

  /**
   * Open the confirmation dialog with a fresh consent flag and refresh the
   * running-browser scan so the warning list is current while it is open.
   * The consent flag itself remains `dialogConfirmedRef` — set only by the
   * DELETE action (M10).
   */
  const openConfirmationDialog = useCallback(() => {
    dialogConfirmedRef.current = false;
    setDialogOpen(true);
    void rescanBrowsers();
  }, [rescanBrowsers]);

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
        setDeferredShred(() => openConfirmationDialog);
        setShredPinOpen(true);
      } else {
        openConfirmationDialog();
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [pinNeeded, pendingFiles.length, selectedProfileCount, openConfirmationDialog]);

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
      setDeferredShred(() => openConfirmationDialog);
      setShredPinOpen(true);
    } else {
      openConfirmationDialog();
    }
  };

  const handleConfirm = () => {
    // The DELETE action on the dialog IS the explicit user action (M10).
    dialogConfirmedRef.current = true;
    void executeShred();
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
      const {
        file_path,
        status,
        current_pass,
        total_passes,
        bytes_written,
        file_size,
      } = event.payload;
      const statusStr = statusToString(status);
      const message =
        status.type === "Error"
          ? `[${file_path}] error: ${status.message}`
          : status.type === "Warning"
            ? `[${file_path}] warning: ${status.message}`
            : `[${file_path}] ${statusStr} (pass ${current_pass}/${total_passes})`;
      const level =
        status.type === "Error"
          ? "error"
          : status.type === "Warning"
            ? "warning"
            : "info";
      addLogEntry(level, message);

      if (status.type === "Complete") {
        completedCountRef.current += 1;
      }

      // Pass-local progress percent (M5): completed passes plus the fraction
      // of the current pass over the total pass count, guarded and clamped.
      setProgress({
        current: completedCountRef.current,
        total: pendingFiles.length,
        percent: computeProgressPercent(
          current_pass,
          total_passes,
          bytes_written,
          file_size
        ),
        currentFile: file_path,
      });
    });
    unlistenRef.current = unlisten;

    try {
      const report: BatchRootResult = await invoke<BatchRootResult>("execute_roots", {
        request,
        method: deletionMethod,
        writeCheck,
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

      // A root whose write check did not pass is not a clean success — warn
      // instead of rendering a plain "Success" (M3).
      if (report.roots.some(isWriteCheckFailure)) {
        addLogEntry(
          "warning",
          "Deletion completed, but the requested write check did not pass."
        );
      }

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
            const browserReport: BatchRootResult = await invoke<BatchRootResult>(
              "shred_browser_data",
              {
                request: {
                  browser_name: profile.browser_name,
                  profile_path: profile.profile_path,
                  data_types: profile.data_types,
                  method: deletionMethod,
                  write_check: writeCheck,
                  explicit_consent: dialogConfirmedRef.current,
                },
              }
            );
            const removed = browserReport.roots.filter(
              (root) => root.status === "destroyed" && root.root_removed
            ).length;
            const failed = browserReport.roots.filter(
              (root) => root.status === "failed"
            ).length;
            const cancelled = browserReport.roots.filter(
              (root) => root.status === "cancelled"
            ).length;
            addLogEntry(
              "success",
              `${profile.browser_name}: ${removed} removed, ${failed} failed, ${cancelled} cancelled`
            );
            if (browserReport.roots.some(isWriteCheckFailure)) {
              addLogEntry(
                "warning",
                "Deletion completed, but the requested write check did not pass."
              );
            }
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
        onConfirm={handleConfirm}
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
