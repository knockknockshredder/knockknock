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
  BrowserRunningState,
  BrowserRunningStatus,
  ProgressEvent,
  RootResultDto,
  ShredStatus,
} from "@/types";

function statusToString(status: ShredStatus): string {
  return status.type;
}

function hasOverwritePass(currentPass: number, totalPasses: number): boolean {
  return (
    Number.isInteger(currentPass) &&
    Number.isInteger(totalPasses) &&
    currentPass > 0 &&
    totalPasses > 0 &&
    currentPass <= totalPasses
  );
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
  const stopRequestedRef = useRef(false);
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
  const blockedSelectedBrowsers = browsers
    .filter((b) => b.runningState !== "closed" && b.profiles.some((p) => p.selected))
    .map((b) => ({ name: b.name, state: b.runningState }));

  /**
   * Open the confirmation dialog. The running-browser warning shown in the
   * dialog comes from the cached BrowserContext state (kept live by the
   * lightweight watcher); a general installed-browser scan runs only at app
   * initialization or on explicit user refresh (LeftSidebar), never per
   * dialog open. The fresh backend running-state check before execution is
   * the final decision — cached state here is never trusted for safety.
   */
  const openConfirmationDialog = useCallback(() => {
    setDialogOpen(true);
  }, []);

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
    // The DELETE action on the dialog confirms the destructive deletion. It
    // is NOT consent to delete browser data while the browser is running —
    // the fresh running-state check below still blocks in that case.
    void executeShred();
  };

  const executeShred = async () => {
    if (isExecutingRef.current) return;
    if (pendingFiles.length === 0 && selectedProfileCount === 0) return;

    isExecutingRef.current = true;
    stopRequestedRef.current = false;

    const selectedProfiles = browsers.flatMap((b) =>
      b.profiles
        .filter((p) => p.selected)
        .map((p) => ({
          browser_name: b.name,
          browser_id: b.id,
          profile_path: p.path,
          data_types: ["cache", "cookies", "history", "passwords"] as const,
        }))
    );

    // Running-state precondition: browser cleanup only proceeds while every
    // selected browser is currently closed, verified FRESH right now. The
    // dialog's cached state is never the final decision. This runs before
    // anything destructive starts, so a running browser also blocks
    // file/folder roots selected in the same operation. Fail-closed: if the
    // running state cannot be confirmed, nothing starts.
    if (selectedProfiles.length > 0) {
      const requests = Array.from(
        selectedProfiles.reduce((byBrowser, profile) => {
          const paths = byBrowser.get(profile.browser_id) ?? [];
          paths.push(profile.profile_path);
          byBrowser.set(profile.browser_id, paths);
          return byBrowser;
        }, new Map<string, string[]>())
      ).map(([browserId, profilePaths]) => ({ browserId, profilePaths }));

      let blockedBrowser: { name: string; state: BrowserRunningStatus } | null = null;
      let checkFailed = false;
      try {
        const states = await invoke<BrowserRunningState[]>(
          "check_browser_running_states",
          { requests }
        );
        const stateById = new Map(states.map((state) => [state.browserId, state.state]));
        const blockedProfile = selectedProfiles.find(
          (profile) => stateById.get(profile.browser_id) !== "closed"
        );
        if (blockedProfile) {
          blockedBrowser = {
            name: blockedProfile.browser_name,
            state: stateById.get(blockedProfile.browser_id) ?? "unknown",
          };
        }
      } catch {
        checkFailed = true;
      }

      if (checkFailed) {
        addLogEntry(
          "error",
          "Could not confirm that the selected browser is closed. Browser data was not deleted."
        );
        isExecutingRef.current = false;
        return;
      }
      if (blockedBrowser) {
        addLogEntry(
          "warning",
          blockedBrowser.state === "running"
            ? `Browser data deletion blocked: ${blockedBrowser.name} is currently running. Close it before deleting browser data.`
            : "Could not confirm that the selected browser is closed. Browser data was not deleted."
        );
        isExecutingRef.current = false;
        return;
      }
    }

    // Create the shared cancellation session before any asynchronous
    // preparation. This lets Stop target the same session while vault
    // persistence is in flight.
    try {
      await invoke<void>("begin_shred_operation");
    } catch (err) {
      addLogEntry("error", `Refusing to shred: could not begin operation: ${String(err)}`);
      setIsShredding(false);
      isExecutingRef.current = false;
      return;
    }

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

    const outcome = {
      removed: 0,
      failed: 0,
      cancelled: 0,
      skipped: 0,
      browserProfilesCompleted: 0,
      browserProfilesFailed: 0,
      browserProfilesCancelled: 0,
      browserProfilesSkipped: 0,
      hasWriteCheckFailure: false,
      stopped: false,
    };

    const shouldStartNextPhase = async () => {
      const backendCancelled = await invoke<boolean>("is_shred_operation_cancelled");
      return !backendCancelled && !stopRequestedRef.current;
    };

    const recordRootOutcome = (report: BatchRootResult) => {
      outcome.removed += report.roots.filter(
        (root) => root.status === "destroyed" && root.root_removed
      ).length;
      outcome.failed += report.roots.filter((root) => root.status === "failed").length;
      outcome.cancelled += report.roots.filter((root) => root.status === "cancelled").length;
      outcome.skipped += report.roots.filter((root) => root.status === "skipped").length;
      outcome.hasWriteCheckFailure ||= report.roots.some(isWriteCheckFailure);
    };

    const recordBrowserProfileOutcome = (report: BatchRootResult) => {
      const hasFailedRoot = report.roots.some((root) => root.status === "failed");
      const hasCancelledRoot = report.roots.some(
        (root) => root.status === "cancelled"
      );
      const hasSkippedRoot = report.roots.some((root) => root.status === "skipped");

      if (hasCancelledRoot) {
        outcome.browserProfilesCancelled += 1;
      } else if (hasFailedRoot) {
        outcome.browserProfilesFailed += 1;
      } else if (hasSkippedRoot) {
        outcome.browserProfilesSkipped += 1;
      } else {
        outcome.browserProfilesCompleted += 1;
      }
      outcome.hasWriteCheckFailure ||= report.roots.some(isWriteCheckFailure);
    };

    let unlisten: (() => void) | null = null;

    try {
      // Reset completed count before listening.
      completedCountRef.current = 0;

      // Listen for progress events.
      unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
        const {
          file_path,
          status,
          current_pass,
          total_passes,
          bytes_written,
          file_size,
        } = event.payload;
        const statusStr = statusToString(status);
        // Pass numbers are only meaningful while an overwrite pass is in
        // progress; completion events can carry valid-looking pass values
        // (e.g. Automatic 1/1, Legacy 3/3) and must not render a suffix.
        const passSuffix =
          status.type === "Shredding" &&
          hasOverwritePass(current_pass, total_passes)
            ? ` (pass ${current_pass}/${total_passes})`
            : "";
        const message =
          status.type === "Error"
            ? `[${file_path}] error: ${status.message}`
            : status.type === "Warning"
              ? `[${file_path}] warning: ${status.message}`
              : `[${file_path}] ${statusStr}${passSuffix}`;
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
      let canStartNextPhase = !stopRequestedRef.current;

      if (request.roots.length > 0 && canStartNextPhase) {
        const report: BatchRootResult = await invoke<BatchRootResult>("execute_roots", {
          request,
          method: deletionMethod,
          writeCheck,
          logObfuscation,
        });

        recordRootOutcome(report);
        // Apply typed per-root results: destroyed roots with root_removed are
        // removed from the list, everything else is retained with error details.
        await applyRootResults(report.roots);

        // The backend token is authoritative between destructive phases. Check
        // after persistence has applied the root results, immediately before
        // browser cleanup can begin, even when Stop is already known locally.
        canStartNextPhase = await shouldStartNextPhase();
      }

      for (const profile of selectedProfiles) {
        if (!canStartNextPhase) break;

        try {
          addLogEntry(
            "info",
            `Deleting selected local data from ${profile.browser_name} profile: ${profile.profile_path}`
          );
          const browserReport: BatchRootResult = await invoke<BatchRootResult>(
            "shred_browser_data",
            {
              request: {
                browser_id: profile.browser_id,
                browser_name: profile.browser_name,
                profile_path: profile.profile_path,
                data_types: profile.data_types,
                method: deletionMethod,
                write_check: writeCheck,
              },
            }
          );
          recordBrowserProfileOutcome(browserReport);
        } catch (err) {
          outcome.browserProfilesFailed += 1;
          addLogEntry(
            "error",
            `Failed to clean ${profile.browser_name} profile: ${err}`
          );
        }

        // Always query the shared backend session after a browser phase before
        // considering another profile, including after a local Stop request.
        canStartNextPhase = await shouldStartNextPhase();
      }

      if (!canStartNextPhase || stopRequestedRef.current) {
        outcome.stopped = true;
      }

      const rootSummary = `${outcome.removed} removed, ${outcome.failed} failed, ${outcome.cancelled} cancelled, ${outcome.skipped} skipped`;
      const browserSummary =
        selectedProfiles.length > 0
          ? `; ${outcome.browserProfilesCompleted} browser profiles completed, ${outcome.browserProfilesFailed} failed, ${outcome.browserProfilesCancelled} cancelled, ${outcome.browserProfilesSkipped} skipped`
          : "";
      const writeCheckSummary = outcome.hasWriteCheckFailure
        ? "; requested write check did not pass"
        : "";
      const summary = `${rootSummary}${browserSummary}${writeCheckSummary}`;
      const hasFailures =
        outcome.failed > 0 || outcome.browserProfilesFailed > 0 || outcome.skipped > 0 || outcome.browserProfilesSkipped > 0;
      const wasStopped =
        outcome.stopped || outcome.cancelled > 0 || outcome.browserProfilesCancelled > 0;
      const cleanSuccess = !hasFailures && !wasStopped && !outcome.hasWriteCheckFailure;

      if (wasStopped) {
        addLogEntry("warning", `Stopped: ${summary}`);
        invoke("send_notification", {
          title: "Deletion Stopped",
          body: summary,
        }).catch(() => {});
      } else if (hasFailures) {
        addLogEntry("error", `Completed with issues: ${summary}`);
        invoke("send_notification", {
          title: "Deletion Completed with Issues",
          body: summary,
        }).catch(() => {});
      } else if (outcome.hasWriteCheckFailure) {
        addLogEntry("warning", `Completed with warnings: ${summary}`);
        invoke("send_notification", {
          title: "Deletion Completed with Warnings",
          body: summary,
        }).catch(() => {});
      } else {
        addLogEntry("success", `Complete: ${summary}`);
        invoke("send_notification", {
          title: "Deletion Complete",
          body: summary,
        }).catch(() => {});
      }

      if (autoClearLog && cleanSuccess) {
        clearLog();
      }
    } catch (err) {
      addLogEntry(
        "error",
        `Deletion terminated unexpectedly. Technical detail: ${String(err)}`
      );
      invoke("send_notification", {
        title: "Deletion Failed",
        body: `Operation terminated unexpectedly: ${String(err).slice(0, 200)}`,
      }).catch(() => {});
    } finally {
      unlisten?.();
      unlistenRef.current = null;
      setProgress(null);
      setIsShredding(false);
      isExecutingRef.current = false;
    }
  };

  const requestStop = async () => {
    stopRequestedRef.current = true;
    try {
      await invoke<void>("cancel_shred");
      addLogEntry("warning", "Stop requested. Already processed targets will not be restored.");
    } catch (err) {
      addLogEntry("error", `Stop request failed: ${err}`);
    }
  };

  const handleCancel = async () => {
    if (pinNeeded) {
      setCancelPinOpen(true);
      return; // Shredding continues if PIN not entered
    }
    await requestStop();
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
        blockedSelectedBrowsers={blockedSelectedBrowsers}
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
          void requestStop();
        }}
        purpose="cancel"
      />
    </div>
  );
}
