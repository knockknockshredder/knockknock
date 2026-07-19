// src/components/settings/PinSetup.tsx
//
// PIN setup dialog. Used both to set the initial PIN and to change an
// existing PIN (the backend replaces the stored hash on every setup).
// Digits-only enforcement lives at three layers:
//   1. Native keyboard (`inputMode="numeric"`, `pattern="[0-9]*"`)
//   2. onChange strip (`value.replace(/\D/g, "")`)
//   3. Backend `validate_pin_format` (authoritative)

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
import { MIN_PIN_LEN, MAX_PIN_LEN } from "@/lib/pin-constants";

interface PinSetupProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Called after the backend confirms `setup_pin` / `change_pin`. The
   *  argument is the newly-active PIN so callers can update any cached
   *  copies (e.g. the vault auto-save key) without re-prompting. */
  onPinSet: (newPin: string) => void;
  /** When true, an "Old PIN" field is shown and `change_pin` is called
   *  instead of `setup_pin` so the vault gets re-encrypted. */
  requireOldPin?: boolean;
}

export function PinSetup({ open, onOpenChange, onPinSet, requireOldPin = false }: PinSetupProps) {
  const [oldPin, setOldPin] = useState("");
  const [pin, setPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // Reset state whenever the dialog closes so a stale error or partial
  // input never leaks into a fresh setup attempt.
  useEffect(() => {
    if (!open) {
      setOldPin("");
      setPin("");
      setConfirmPin("");
      setError(null);
      setSubmitting(false);
    }
  }, [open]);

  const digitsOnly = (value: string) => value.replace(/\D/g, "");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (requireOldPin && oldPin.length < MIN_PIN_LEN) {
      setError(`Old PIN must be at least ${MIN_PIN_LEN} digits`);
      return;
    }
    if (pin.length < MIN_PIN_LEN || pin.length > MAX_PIN_LEN) {
      setError(`PIN must be between ${MIN_PIN_LEN} and ${MAX_PIN_LEN} digits`);
      return;
    }
    if (pin !== confirmPin) {
      setError("PINs do not match");
      return;
    }

    setSubmitting(true);
    try {
      if (requireOldPin) {
        await invoke("change_pin", { old_pin: oldPin, new_pin: pin });
      } else {
        await invoke("setup_pin", { pin_value: pin });
      }
      onPinSet(pin);
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const canSubmit =
    pin.length >= MIN_PIN_LEN &&
    pin.length <= MAX_PIN_LEN &&
    confirmPin.length >= MIN_PIN_LEN &&
    (!requireOldPin || oldPin.length >= MIN_PIN_LEN) &&
    !submitting;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lock size={16} className="text-accent" />
            {requireOldPin ? "Change PIN" : "Set PIN"}
          </DialogTitle>
          <DialogDescription>
            {requireOldPin
              ? "Enter your current PIN, then choose a new one."
              : `Choose a ${MIN_PIN_LEN} to ${MAX_PIN_LEN} digit PIN. You will be
            asked for it to open the app and before each shred operation.`}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          {requireOldPin && (
            <label className="flex flex-col gap-1">
              <span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
                Old PIN
              </span>
              <input
                type="password"
                inputMode="numeric"
                pattern="[0-9]*"
                autoComplete="off"
                value={oldPin}
                onChange={(e) => setOldPin(digitsOnly(e.target.value))}
                maxLength={MAX_PIN_LEN}
                disabled={submitting}
                autoFocus
                className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
              />
            </label>
          )}
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
              disabled={submitting}
              autoFocus={!requireOldPin}
              className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
              Confirm PIN
            </span>
            <input
              type="password"
              inputMode="numeric"
              pattern="[0-9]*"
              autoComplete="off"
              value={confirmPin}
              onChange={(e) => setConfirmPin(digitsOnly(e.target.value))}
              maxLength={MAX_PIN_LEN}
              disabled={submitting}
              className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
            />
          </label>

          {error && (
            <p className="font-mono text-xs text-red-500 flex items-start gap-1.5">
              <WarningCircle size={14} className="flex-shrink-0 mt-0.5" />
              <span>{error}</span>
            </p>
          )}

          <p className="font-mono text-xs text-muted-foreground">
            Store your PIN safely. If you forget it, the app state must be
            wiped, which destroys all saved vaults and configurations.
            KnockKnock cannot recover a lost PIN.
          </p>

          <DialogFooter>
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              disabled={submitting}
              className="px-4 py-2 font-mono text-xs uppercase tracking-wider border border-border text-foreground transition-colors hover:bg-elevated disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!canSubmit}
              className="px-4 py-2 font-mono text-xs uppercase tracking-wider bg-accent text-background transition-colors hover:bg-accent/90 disabled:opacity-50"
            >
              {submitting ? "Saving..." : "Save PIN"}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}