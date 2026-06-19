// src/components/layout/Sidebar.tsx
import {
  Trash,
  Globe,
  GearSix,
  SidebarSimple,
} from "@phosphor-icons/react";
import { useNavigation } from "@/contexts/NavigationContext";
import { cn } from "@/lib/utils";
import type { Section } from "@/types";

const navItems: { section: Section; icon: typeof Trash; label: string }[] = [
  { section: "shred", icon: Trash, label: "Shred" },
  { section: "browser", icon: Globe, label: "Browser" },
  { section: "settings", icon: GearSix, label: "Settings" },
];

export function Sidebar() {
  const { activeSection, sidebarExpanded, setActiveSection, toggleSidebar } =
    useNavigation();

  return (
    <aside
      className={cn(
        "flex flex-col border-r border-border bg-surface transition-all duration-200",
        sidebarExpanded ? "w-[200px]" : "w-[64px]"
      )}
    >
      <nav className="flex-1 py-2">
        <ul className="flex flex-col gap-1 px-2">
          {navItems.map(({ section, icon: Icon, label }) => (
            <li key={section}>
              <button
                onClick={() => setActiveSection(section)}
                className={cn(
                  "flex w-full items-center gap-3 rounded px-3 py-2 text-sm transition-colors",
                  activeSection === section
                    ? "border-l-2 border-accent bg-elevated text-foreground"
                    : "border-l-2 border-transparent text-muted-foreground hover:bg-elevated hover:text-foreground"
                )}
              >
                <Icon size={20} weight="duotone" />
                {sidebarExpanded && <span>{label}</span>}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="border-t border-border p-2">
        <button
          onClick={toggleSidebar}
          aria-label="Toggle sidebar"
          className="flex w-full items-center justify-center rounded p-2 text-muted-foreground hover:bg-elevated hover:text-foreground"
        >
          <SidebarSimple size={20} />
        </button>
      </div>
    </aside>
  );
}
