// src/contexts/NavigationContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";
import type { Section } from "@/types";

interface NavigationState {
  activeSection: Section;
  sidebarExpanded: boolean;
  setActiveSection: (section: Section) => void;
  toggleSidebar: () => void;
}

const NavigationContext = createContext<NavigationState | null>(null);

export function NavigationProvider({ children }: { children: ReactNode }) {
  const [activeSection, setActiveSection] = useState<Section>("shred");
  const [sidebarExpanded, setSidebarExpanded] = useState(true);

  const toggleSidebar = () => setSidebarExpanded((prev) => !prev);

  return (
    <NavigationContext.Provider
      value={{ activeSection, sidebarExpanded, setActiveSection, toggleSidebar }}
    >
      {children}
    </NavigationContext.Provider>
  );
}

export function useNavigation() {
  const ctx = useContext(NavigationContext);
  if (!ctx) throw new Error("useNavigation must be used within NavigationProvider");
  return ctx;
}
