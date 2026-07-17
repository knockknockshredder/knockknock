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

const MIN_PIN_LEN = 6;
const MAX_PIN_LEN = 32;

export type PinVerifyPurpose = "app_open" | "shred" | "cancel" | "disable_pin";

interface PinVerifyProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onVerified: (pin: string) => void;
  purpose: PinVerifyPurpose;
}

const PURPOSE_COPY: Record<PinVerifyPurpose, { title: string; description: string }> = {
  app_open: {
    title: "Enter PIN",
    description: "Enter your PIN to unlock KnockKnock.",
  },
  shred: {
    title: "Authorize shred",
    description: "Enter your PIN to confirm the shred operation.",
  },
  cancel: {
    title: "Authorize cancel",
    description: "Enter your PIN to cancel the shred in progress.",
  },
  disable_pin: {
    title: "Authorize disable",
    description: "Enter your PIN to disable PIN protection.",
  },
};

export function PinVerify({ open, onOpenChange, onVerified, purpose }: PinVerifyProps) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [lockoutSeconds, setLockoutSeconds] = useState(0);

  // Reset on close.
  useEffect(() => {
    if (!open) {
      setPin("");
      setError(null);
      setSubmitting(false);
      setLockoutSeconds(0);
    }
  }, [open]);

  // Poll the backend for lockout state whenever the dialog opens. If
  // locked, start a local countdown that ticks down once per second so
  // the user sees the wait time update in real time.
  useEffect(() => {
    if (!open) return;

    let cancelled = false;

    const refresh = async () => {
      try {
        const remaining = await invoke<number>("get_lockout_remaining");
        if (!cancelled) setLockoutSeconds(remaining);
      } catch {
        if (!cancelled) setLockoutSeconds(0);
      }
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
        onVerified(pin);
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
      try {
        const remaining = await invoke<number>("get_lockout_remaining");
        setLockoutSeconds(remaining);
      } catch {
        // ignore — polling errors aren't actionable here
      }
      setPin("");
    } finally {
      setSubmitting(false);
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
              {submitting ? "Verifying..." : "Unlock"}
            </button>
          </DialogFooter>
        </form>

        <div className="border-t border-border pt-3">
          <p className="font-mono text-xs text-muted-foreground">
            Forgot your PIN? KnockKnock cannot recover it. The app must be
            reset, which clears all saved vaults and configurations. Use
            the "Reset app" option in Settings to start over.
          </p>
        </div>
      </DialogContent>
    </Dialog>
  );
}