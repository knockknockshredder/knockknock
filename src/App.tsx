// src/App.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  const [gatePassed, setGatePassed] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [configError, setConfigError] = useState(false);
  const { loadVault, addLogEntry, clearFiles, setVaultPin } = useShred();

  useEffect(() => {
    invoke<boolean>("has_pin")
      .then((has) => {
        setHasPin(has);
        if (!has) {
          setShowOnboarding(true);
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
      addLogEntry("error", "Failed to unlock saved target list");
    }
  };

  const handleGateReset = async () => {
    await invoke<void>("reset_app_without_pin");
    clearFiles();
    setVaultPin(null);
    setGatePassed(false);
    setHasPin(false);
    setShowOnboarding(true);
  };

  const handleOnboardingPinSet = async (newPin: string) => {
    try {
      await loadVault(newPin);
    } catch {
      addLogEntry("error", "Failed to initialize saved target list");
      return;
    }
    setShowOnboarding(false);
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

  if (hasPin && !gatePassed) {
    return (
      <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background">
        <PinVerify
          open
          onOpenChange={() => {}}
          onVerified={handleGateVerified}
          onReset={handleGateReset}
          purpose="app_open"
        />
      </div>
    );
  }

  return <AppContent />;
}

function AppContent() {
  const { activeSection, setActiveSection } = useNavigation();
  useBrowserDetection();

  // Listen for tray menu "Quick Shred" — shows window and navigates to
  // home section. The actual shred flow is triggered by ShredSection's
  // own listener on the same event.
  useEffect(() => {
    const unlistenPromise = listen("quick-shred-request", () => {
      setActiveSection("home");
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [setActiveSection]);

  // Listen for tray menu "Settings" — shows window and navigates to
  // the Settings section.
  useEffect(() => {
    const unlistenPromise = listen("open-settings", () => {
      setActiveSection("settings");
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [setActiveSection]);

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
