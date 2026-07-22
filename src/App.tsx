// src/App.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Lock, WarningCircle } from "@phosphor-icons/react";
import { NavigationProvider } from "@/contexts/NavigationContext";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { SettingsProvider } from "@/contexts/SettingsContext";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { AppShell } from "@/components/layout/AppShell";
import { OperationLog } from "@/components/layout/OperationLog";
import { PinSetup } from "@/components/settings/PinSetup";
import { PinVerify } from "@/components/settings/PinVerify";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { SettingsSection } from "@/sections/SettingsSection";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";
import { MIN_PIN_LEN, MAX_PIN_LEN } from "@/lib/pin-constants";

function AppGate() {
  const [hasPin, setHasPin] = useState<boolean | null>(null);
  const [pinEnabled, setPinEnabled] = useState<boolean | null>(null);
  const [gatePassed, setGatePassed] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [configError, setConfigError] = useState(false);
  const [showVaultRestore, setShowVaultRestore] = useState(false);
  const [restorePin, setRestorePin] = useState("");
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoreSubmitting, setRestoreSubmitting] = useState(false);
  const { loadVault, addLogEntry } = useShred();

  useEffect(() => {
    // First: check whether a PIN hash exists at all
    invoke<boolean>("has_pin")
      .then((has) => {
        setHasPin(has);
        if (!has) {
          // Fresh install — no PIN configured. Show onboarding.
          setShowOnboarding(true);
        } else {
          // PIN exists — check if the gate is enabled
          invoke<boolean>("is_pin_enabled")
            .then((enabled) => {
              setPinEnabled(enabled);
              if (!enabled) {
                // PIN configured but gate disabled — check if a vault
                // exists on disk.  If it does, offer a soft restore
                // prompt so the user can recover their previous session.
                invoke<boolean>("vault_exists")
                  .then((exists) => {
                    if (exists) {
                      setShowVaultRestore(true);
                    } else {
                      setGatePassed(true);
                    }
                  })
                  .catch(() => {
                    // vault_exists check failed — proceed without vault
                    setGatePassed(true);
                  });
              }
            })
            .catch(() => {
              setConfigError(true);
            });
        }
      })
      .catch(() => {
        setConfigError(true);
      });
  }, []);

  const handleGateVerified = async (pin: string) => {
    try {
      await loadVault(pin);
      setGatePassed(true);
    } catch {
      addLogEntry("error", "Failed to unlock vault");
    }
  };

  const handleOnboardingPinSet = async (newPin: string) => {
    try {
      await loadVault(newPin);
    } catch {
      addLogEntry("error", "Failed to initialize vault");
      return;
    }
    // Enable PIN protection so the gate is shown on future restarts.
    // Without this, the vault is never loaded on subsequent opens,
    // and auto-save silently never fires.  If enabling fails, the
    // vault is still loaded for this session — the user is blocked
    // from entering the app only if loadVault itself failed.
    try {
      await invoke("set_pin_enabled", { currentPin: newPin, enabled: true });
    } catch {
      addLogEntry("error", "Failed to enable PIN protection");
    }
    setShowOnboarding(false);
    setGatePassed(true);
  };

  const handleRestoreSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setRestoreError(null);

    if (restorePin.length < MIN_PIN_LEN || restorePin.length > MAX_PIN_LEN) {
      setRestoreError(
        `PIN must be between ${MIN_PIN_LEN} and ${MAX_PIN_LEN} digits`
      );
      return;
    }

    setRestoreSubmitting(true);
    try {
      // Verify the PIN BEFORE calling loadVault — loadVault swallows
      // errors internally (it is designed to never throw, marking the
      // vault as loaded even on wrong-PIN).  We verify separately so a
      // bad PIN surfaces the error to the user instead of silently
      // proceeding without restoring their files.
      const ok = await invoke<boolean>("verify_pin", {
        pin_value: restorePin,
      });
      if (!ok) {
        setRestoreError("Incorrect PIN");
        setRestorePin("");
        return;
      }
      await loadVault(restorePin);
      setShowVaultRestore(false);
      setGatePassed(true);
    } catch (err) {
      // verify_pin throws on lockout or format error — surface the
      // backend message directly so the user sees accurate feedback.
      setRestoreError(String(err));
      setRestorePin("");
    } finally {
      setRestoreSubmitting(false);
    }
  };

  const handleRestoreSkip = () => {
    setShowVaultRestore(false);
    setGatePassed(true);
  };

  if (hasPin === null) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background">
        <div className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Loading…
        </div>
      </div>
    );
  }

  if (configError) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background p-6">
        <div className="max-w-md text-center">
          <h2 className="font-sans text-xl font-semibold text-destructive">
            Configuration Error
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            The PIN configuration is inconsistent. Please reinstall the app or
            contact support.
          </p>
        </div>
      </div>
    );
  }

  // Onboarding: no PIN exists — user must set one before using the app
  if (showOnboarding) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background">
        <PinSetup
          open
          onOpenChange={() => {}}
          requireOldPin={false}
          onPinSet={handleOnboardingPinSet}
        />
      </div>
    );
  }

  // Gate: PIN exists and the gate is enabled
  if (hasPin && pinEnabled && !gatePassed) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background">
        <PinVerify
          open
          onOpenChange={() => {}}
          onVerified={handleGateVerified}
          purpose="app_open"
        />
      </div>
    );
  }

  // Vault restore: PIN is configured but the gate is disabled, and a
  // vault file exists on disk.  Offer the user an optional PIN prompt to
  // recover their shred list; they may also skip and start fresh.
  if (showVaultRestore) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background">
        <Dialog open onOpenChange={() => {}}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Lock size={16} className="text-accent" />
              Restore Previous Session
            </DialogTitle>
            <DialogDescription>
              Your previous shred list is saved on disk. Enter your PIN to
              restore it, or skip to start fresh.
            </DialogDescription>
          </DialogHeader>

          <form onSubmit={handleRestoreSubmit} className="flex flex-col gap-3">
            <label className="flex flex-col gap-1">
              <span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
                PIN
              </span>
              <input
                type="password"
                inputMode="numeric"
                pattern="[0-9]*"
                autoComplete="off"
                value={restorePin}
                onChange={(e) =>
                  setRestorePin(e.target.value.replace(/\D/g, ""))
                }
                maxLength={MAX_PIN_LEN}
                disabled={restoreSubmitting}
                autoFocus
                className="font-mono px-3 py-2 bg-surface border border-border focus:border-accent focus:outline-none disabled:opacity-50"
              />
            </label>

            {restoreError && (
              <p className="font-mono text-xs text-red-500 flex items-start gap-1.5">
                <WarningCircle size={14} className="flex-shrink-0 mt-0.5" />
                <span>{restoreError}</span>
              </p>
            )}

            <DialogFooter>
              <button
                type="button"
                onClick={handleRestoreSkip}
                disabled={restoreSubmitting}
                className="px-4 py-2 font-mono text-xs uppercase tracking-wider border border-border text-foreground transition-colors hover:bg-elevated disabled:opacity-50"
              >
                Skip
              </button>
              <button
                type="submit"
                disabled={
                  restoreSubmitting ||
                  restorePin.length < MIN_PIN_LEN
                }
                className="px-4 py-2 font-mono text-xs uppercase tracking-wider bg-accent text-background transition-colors hover:bg-accent/90 disabled:opacity-50"
              >
                {restoreSubmitting ? "Restoring..." : "Restore"}
              </button>
            </DialogFooter>
          </form>

          <div className="border-t border-border pt-3">
            <p className="font-mono text-xs text-muted-foreground">
              Forgot your PIN? Skip to start a fresh session. The saved files
              will remain on disk until overwritten.
            </p>
          </div>
        </DialogContent>
      </Dialog>
      </div>
    );
  }

  return <AppContent />;
}

function AppContent() {
  const { activeSection } = useNavigation();
  useBrowserDetection();

  return (
    <AppShell bottom={<OperationLog />}>
      {activeSection === "home" && <ShredSection />}
      {activeSection === "settings" && <SettingsSection />}
    </AppShell>
  );
}

function App() {
  return (
    <NavigationProvider>
      <ShredProvider>
        <SettingsProvider>
          <BrowserProvider>
            <AppGate />
          </BrowserProvider>
        </SettingsProvider>
      </ShredProvider>
    </NavigationProvider>
  );
}

export default App;