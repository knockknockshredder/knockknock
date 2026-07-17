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
  const [showPinVerify, setShowPinVerify] = useState(false);
  const { loadVault, addLogEntry } = useShred();

  useEffect(() => {
    invoke<boolean>("is_pin_enabled").then((enabled) => {
      setPinNeeded(enabled);
      if (enabled) setShowPinVerify(true);
    }).catch(() => setPinNeeded(false));
  }, []);

  const handleGateVerified = async (pin: string) => {
    try {
      await loadVault(pin);
      setShowPinVerify(false);
    } catch {
      addLogEntry("error", "Failed to unlock vault");
    }
  };

  if (pinNeeded === null) return null;

  return (
    <>
      {pinNeeded && (
        <PinVerify
          open={showPinVerify}
          onOpenChange={setShowPinVerify}
          onVerified={handleGateVerified}
          purpose="app_open"
        />
      )}
      <AppContent />
    </>
  );
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