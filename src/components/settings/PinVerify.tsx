// src/components/settings/PinVerify.tsx
//
// PIN verification dialog. Prompts the user for their PIN before
// privileged operations (app open, shred, cancel mid-shred). Mirrors the
// digits-only enforcement used by PinSetup, and surfaces the backend
// lockout state with a live countdown so users see when they can retry.
//
// The `purpose` prop is for future use (e.g. different copy depending on
// whether we're guarding a shred vs an app-open) and currently only
// influences the dialog description.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Lock, WarningCircle } from "@phosphor-icons/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { MIN_PIN_LEN, MAX_PIN_LEN } from "@/lib/pin-constants";

export type PinVerifyPurpose =
  | "app_open"
  | "shred"
  | "cancel"
  | "disable_pin"
  | "set_pin_enabled";

interface PinVerifyProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onVerified: (pin: string) => void | Promise<void>;
  onReset?: () => void | Promise<void>;
  purpose: PinVerifyPurpose;
}

const LOCKOUT_ERROR_FALLBACK_SECONDS = 86_400;
const RESET_CONFIRMATION = "RESET";

const PURPOSE_COPY: Record<PinVerifyPurpose, { title: string; description: string }> = {
  app_open: {
    title: "Enter PIN",
    description: "Enter your PIN to unlock KnockKnock.",
  },
  shred: {
    title: "Authorize deletion",
    description: "Enter your PIN to confirm the deletion operation.",
  },
  cancel: {
    title: "Authorize stop",
    description:
      "Enter your PIN to request stopping the operation. Already processed targets will not be restored.",
  },
  disable_pin: {
    title: "Authorize disable",
    description: "Enter your PIN to disable PIN protection.",
  },
  set_pin_enabled: {
    title: "Authorize enable",
    description: "Enter your PIN to enable PIN protection.",
  },
};

export function PinVerify({ open, onOpenChange, onVerified, onReset, purpose }: PinVerifyProps) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [unlocking, setUnlocking] = useState(false);
  const [lockoutSeconds, setLockoutSeconds] = useState(0);
  const [resetDialogOpen, setResetDialogOpen] = useState(false);
  const [resetPhrase, setResetPhrase] = useState("");
  const [resetError, setResetError] = useState<string | null>(null);
  const [resetSubmitting, setResetSubmitting] = useState(false);

  // Reset on close.
  useEffect(() => {
    if (!open) {
      setPin("");
      setError(null);
      setSubmitting(false);
      setUnlocking(false);
      setLockoutSeconds(0);
      setResetDialogOpen(false);
      setResetPhrase("");
      setResetError(null);
      setResetSubmitting(false);
    }
  }, [open]);

  // Poll the backend for lockout state whenever the dialog opens. If
  // locked, start a local countdown that ticks down once per second so
  // the user sees the wait time update in real time.
  useEffect(() => {
    if (!open) return;

    let cancelled = false;

    const refresh = async () => {
      const remaining = await invoke<number>("get_lockout_remaining").catch(
        () => LOCKOUT_ERROR_FALLBACK_SECONDS,
      );
      if (!cancelled) setLockoutSeconds(remaining);
    };

    void refresh();

    const interval = setInterval(refresh, 1000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [open]);

  const digitsOnly = (value: string) => value.replace(/\D/g, "");

  const isLocked = lockoutSeconds > 0;

  const handleResetDialogOpenChange = (nextOpen: boolean) => {
    setResetDialogOpen(nextOpen);
    if (!nextOpen) {
      setResetPhrase("");
      setResetError(null);
      setResetSubmitting(false);
    }
  };

  const handleReset = async () => {
    if (!onReset || resetPhrase !== RESET_CONFIRMATION) return;

    setResetError(null);
    setResetSubmitting(true);
    try {
      await onReset();
      handleResetDialogOpenChange(false);
    } catch (err) {
      setResetError(String(err));
    } finally {
      setResetSubmitting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isLocked) return;
    setError(null);

    if (pin.length < MIN_PIN_LEN || pin.length > MAX_PIN_LEN) {
      setError(`PIN must be between ${MIN_PIN_LEN} and ${MAX_PIN_LEN} digits`);
      return;
    }

    setSubmitting(true);
    try {
      const ok = await invoke<boolean>("verify_pin", { pinValue: pin });
      if (ok) {
        // PIN verified — now give the caller time to do its work
        // (e.g. decrypting the vault) before resetting the UI.
        setUnlocking(true);
        await onVerified(pin);
        onOpenChange(false);
      } else {
        setError("Incorrect PIN");
        setPin("");
      }
    } catch (err) {
      // Backend returns Err with the lockout message; surface it and
      // also re-poll the remaining time so the countdown updates.
      const msg = String(err);
      setError(msg);
      const remaining = await invoke<number>("get_lockout_remaining").catch(
        () => LOCKOUT_ERROR_FALLBACK_SECONDS,
      );
      setLockoutSeconds(remaining);
      setPin("");
    } finally {
      setSubmitting(false);
      setUnlocking(false);
    }
  };

  const copy = PURPOSE_COPY[purpose];
  const isGate = purpose === "app_open";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={!isGate}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lock size={16} className="text-accent" />
            {copy.title}
          </DialogTitle>
          <DialogDescription>{copy.description}</DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          {isLocked ? (
            <div className="border border-red-500/40 bg-red-500/10 p-3 flex items-start gap-2">
              <WarningCircle size={14} className="text-red-500 flex-shrink-0 mt-0.5" />
              <div className="font-mono text-xs text-red-500">
                <p>Too many incorrect attempts.</p>
                <p className="mt-1">
                  Try again in {lockoutSeconds} second{lockoutSeconds === 1 ? "" : "s"}.
                </p>
              </div>
            </div>
          ) : (
            <label className="flex flex-col gap-1">
              <span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
                PIN
              </span>
              <input
                type="password"
                inputMode="numeric"
                pattern="[0-9]*"
                autoComplete="off"
                value={pin}
                onChange={(e) => setPin(digitsOnly(e.target.value))}
                maxLength={MAX_PIN_LEN}
                disabled={submitting || isLocked}
                autoFocus
                className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
              />
            </label>
          )}

          {error && !isLocked && (
            <p className="font-mono text-xs text-red-500 flex items-start gap-1.5">
              <WarningCircle size={14} className="flex-shrink-0 mt-0.5" />
              <span>{error}</span>
            </p>
          )}

          <DialogFooter>
            {!isGate && (
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                disabled={submitting}
                className="px-4 py-2 font-mono text-xs uppercase tracking-wider border border-border text-foreground transition-colors hover:bg-elevated disabled:opacity-50"
              >
                Cancel
              </button>
            )}
            <button
              type="submit"
              disabled={submitting || isLocked || pin.length < MIN_PIN_LEN}
              className="px-4 py-2 font-mono text-xs uppercase tracking-wider bg-accent text-background transition-colors hover:bg-accent/90 disabled:opacity-50"
            >
              {unlocking ? "Unlocking..." : submitting ? "Verifying..." : "Unlock"}
            </button>
          </DialogFooter>
        </form>

        {isGate && onReset ? (
          <div className="border-t border-border pt-3">
            <button
              type="button"
              onClick={() => handleResetDialogOpenChange(true)}
              disabled={submitting || unlocking}
              className="font-mono text-xs text-muted-foreground underline underline-offset-3 transition-colors hover:text-foreground"
            >
              Forgot PIN?
            </button>
          </div>
        ) : !isGate ? (
          <div className="border-t border-border pt-3">
            <p className="font-mono text-xs text-muted-foreground">
              KnockKnock cannot recover a forgotten PIN.
            </p>
          </div>
        ) : null}
      </DialogContent>

      <AlertDialog
        open={resetDialogOpen}
        onOpenChange={handleResetDialogOpenChange}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reset app protection?</AlertDialogTitle>
            <AlertDialogDescription>
              This removes your PIN and deletes KnockKnock&apos;s saved target
              list. It will not delete files from your computer or reset other app settings.
            </AlertDialogDescription>
          </AlertDialogHeader>

          <label className="flex flex-col gap-1">
            <span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
              Type RESET to confirm
            </span>
            <input
              aria-label="Reset confirmation"
              autoComplete="off"
              value={resetPhrase}
              onChange={(e) => setResetPhrase(e.target.value)}
              disabled={resetSubmitting}
              className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
            />
          </label>

          {resetError && (
            <p className="font-mono text-xs text-red-500 flex items-start gap-1.5">
              <WarningCircle size={14} className="flex-shrink-0 mt-0.5" />
              <span>{resetError}</span>
            </p>
          )}

          <AlertDialogFooter>
            <AlertDialogCancel disabled={resetSubmitting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              type="button"
              onClick={() => void handleReset()}
              disabled={resetSubmitting || resetPhrase !== RESET_CONFIRMATION}
              className="bg-red-600 text-white hover:bg-red-700"
            >
              {resetSubmitting ? "Resetting..." : "Reset app protection"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Dialog>
  );
}
