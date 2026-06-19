// src/components/layout/TitleBar.tsx
import { X, Square, Minus } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div
      data-tauri-drag-region
      className="flex h-10 items-center justify-between border-b border-border bg-surface px-4 select-none"
    >
      <div className="flex items-center gap-2">
        <span className="font-mono text-xs font-semibold uppercase tracking-widest text-foreground">
          KnockKnock
        </span>
      </div>
      <div className="flex items-center gap-1">
        <button
          aria-label="Minimize window"
          onClick={() => appWindow.minimize()}
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10"
        >
          <Minus size={14} className="text-muted-foreground" />
        </button>
        <button
          aria-label="Toggle maximize"
          onClick={() => appWindow.toggleMaximize()}
          className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10"
        >
          <Square size={12} className="text-muted-foreground" />
        </button>
        <button
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
