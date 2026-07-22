// src/App.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { NavigationProvider } from "@/contexts/NavigationContext";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { SettingsProvider } from "@/contexts/SettingsContext";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { AppShell } from "@/components/layout/AppShell";
import { OperationLog } from "@/components/layout/OperationLog";
import { PinSetup } from "@/components/settings/PinSetup";
import { PinVerify } from "@/components/settings/PinVerify";
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { SettingsSection } from "@/sections/SettingsSection";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

function AppGate() {
  const [hasPin, setHasPin] = useState<boolean | null>(null);
  const [pinEnabled, setPinEnabled] = useState<boolean | null>(null);
  const [gatePassed, setGatePassed] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [configError, setConfigError] = useState(false);
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
                // PIN was configured but user disabled it — skip gate
                setGatePassed(true);
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
      setShowOnboarding(false);
      setGatePassed(true);
    } catch {
      addLogEntry("error", "Failed to initialize vault");
    }
  };

  if (hasPin === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Loading…
        </div>
      </div>
    );
  }

  if (configError) {
    return (
      <div className="flex h-screen items-center justify-center bg-background p-6">
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
      <PinSetup
        open
        onOpenChange={() => {}}
        requireOldPin={false}
        onPinSet={handleOnboardingPinSet}
      />
    );
  }

  // Gate: PIN exists and the gate is enabled
  if (hasPin && pinEnabled && !gatePassed) {
    return (
      <PinVerify
        open
        onOpenChange={() => {}}
        onVerified={handleGateVerified}
        purpose="app_open"
      />
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