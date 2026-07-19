// src/App.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { NavigationProvider } from "@/contexts/NavigationContext";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { SettingsProvider } from "@/contexts/SettingsContext";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { AppShell } from "@/components/layout/AppShell";
import { OperationLog } from "@/components/layout/OperationLog";
import { PinVerify } from "@/components/settings/PinVerify";
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { SettingsSection } from "@/sections/SettingsSection";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

function AppGate() {
  const [pinNeeded, setPinNeeded] = useState<boolean | null>(null);
  const [gatePassed, setGatePassed] = useState(false);
  const [configError, setConfigError] = useState(false);
  const { loadVault, addLogEntry } = useShred();

  useEffect(() => {
    invoke<boolean>("is_pin_enabled")
      .then((enabled) => {
        setPinNeeded(enabled);
        if (!enabled) setGatePassed(true);
      })
      .catch(() => {
        setPinNeeded(true);  // assume PIN required on error
        // DO NOT set gatePassed — keep showing gate
      });

    // Defensive: verify configuration consistency
    if (pinNeeded) {
      invoke<boolean>("has_pin").then((has) => {
        if (!has) {
          // Inconsistent state — enabled flag set but no hash. Show error.
          setConfigError(true);
          setPinNeeded(false);  // don't show normal gate — show error screen
        }
      }).catch(() => {
        // If has_pin IPC fails, treat as config error too
        setConfigError(true);
        setPinNeeded(false);
      });
    }
  }, [pinNeeded]);

  const handleGateVerified = async (pin: string) => {
    try {
      await loadVault(pin);
      setGatePassed(true);
    } catch {
      addLogEntry("error", "Failed to unlock vault");
    }
  };

  if (pinNeeded === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Loading…
        </div>
      </div>
    );
  }

  // Gate NOT passed: only show the PIN dialog, nothing else.
  // The dialog cannot be dismissed — user MUST authenticate.
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

  if (!gatePassed) {
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