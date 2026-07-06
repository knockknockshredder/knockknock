// src/components/layout/TitleBar.tsx
import { X, Square, Minus, GearSix, House } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNavigation } from "@/contexts/NavigationContext";

export function TitleBar() {
  const appWindow = getCurrentWindow();
  const { activeSection, setActiveSection } = useNavigation();

  return (
    <div
      data-tauri-drag-region
      className="flex h-10 items-center justify-between border-b border-border bg-surface px-4 select-none"
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => setActiveSection("home")}
          className="cursor-pointer font-mono text-xs font-semibold tracking-widest text-foreground transition-colors hover:text-accent"
        >
          KnockKnock
        </button>
      </div>
      <div className="flex items-center gap-1">
        <button
          type="button"
          aria-label={
            activeSection === "home" ? "Open settings" : "Go to home"
          }
          onClick={() =>
            setActiveSection(activeSection === "home" ? "settings" : "home")
          }
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10"
        >
          {activeSection === "home" ? (
            <GearSix size={14} className="text-muted-foreground" />
          ) : (
            <House size={14} className="text-muted-foreground" />
          )}
        </button>
        <button
          type="button"
          aria-label="Minimize window"
          onClick={() => appWindow.minimize()}
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10"
        >
          <Minus size={14} className="text-muted-foreground" />
        </button>
        <button
          type="button"
          aria-label="Toggle maximize"
          onClick={() => appWindow.toggleMaximize()}
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10"
        >
          <Square size={12} className="text-muted-foreground" />
        </button>
        <button
          type="button"
          aria-label="Close window"
          onClick={() => appWindow.close()}
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-destructive/20"
        >
          <X size={14} className="text-muted-foreground" />
        </button>
      </div>
    </div>
  );
}
