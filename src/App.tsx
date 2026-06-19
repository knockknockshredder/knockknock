// src/App.tsx
import { NavigationProvider } from "@/contexts/NavigationContext";
import { ShredProvider } from "@/contexts/ShredContext";
import { SettingsProvider } from "@/contexts/SettingsContext";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { AppShell } from "@/components/layout/AppShell";
import { OperationLog } from "@/components/layout/OperationLog";
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { BrowserSection } from "@/sections/BrowserSection";
import { SettingsSection } from "@/sections/SettingsSection";

function AppContent() {
  const { activeSection } = useNavigation();

  return (
    <AppShell bottom={<OperationLog />}>
      {activeSection === "shred" && <ShredSection />}
      {activeSection === "browser" && <BrowserSection />}
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
