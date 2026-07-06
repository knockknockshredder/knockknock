// src/App.tsx
import { NavigationProvider } from "@/contexts/NavigationContext";
import { ShredProvider } from "@/contexts/ShredContext";
import { SettingsProvider } from "@/contexts/SettingsContext";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { AppShell } from "@/components/layout/AppShell";
import { OperationLog } from "@/components/layout/OperationLog";
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { SettingsSection } from "@/sections/SettingsSection";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

function AppContent() {
  const { activeSection } = useNavigation();
  useBrowserDetection(); // Runs once at app level

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
            <AppContent />
          </BrowserProvider>
        </SettingsProvider>
      </ShredProvider>
    </NavigationProvider>
  );
}

export default App;