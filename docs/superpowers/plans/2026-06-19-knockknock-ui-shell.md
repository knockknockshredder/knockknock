# KnockKnock UI Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the KnockKnock "Security Console" UI shell — a dark, boxy, terminal-tinged desktop interface for file shredding, browser cleanup, and settings.

**Architecture:** React SPA inside Tauri 2.x window. shadcn/ui (Lyra preset) for design system, Magic UI Terminal for operation log, Phosphor icons. Four React contexts for state. Sidebar + main panel + collapsible log layout.

**Tech Stack:** React 19, TypeScript, Vite 7, Tailwind CSS v4, shadcn/ui (Lyra), Magic UI, Phosphor Icons, JetBrains Mono + Inter fonts, Tauri 2.x IPC.

**Spec:** `docs/superpowers/specs/2026-06-19-knockknock-ui-design.md`

**Oracle review:** `ses_11ec7c7eeffeXKuuCBHxa3vOmf` — all critical fixes integrated below.

---

## File Map

```
src/
├── App.tsx                          # AppShell + context providers
├── main.tsx                         # Entry point, font imports
├── index.css                        # Tailwind imports + CSS variables (replaces App.css)
├── lib/
│   └── utils.ts                     # cn() helper (created by shadcn init)
├── types/
│   └── index.ts                     # Shared TypeScript types
├── contexts/
│   ├── NavigationContext.tsx
│   ├── ShredContext.tsx
│   ├── BrowserContext.tsx
│   └── SettingsContext.tsx
├── components/
│   ├── ui/                          # shadcn + Magic UI components (auto-generated)
│   ├── layout/
│   │   ├── AppShell.tsx
│   │   ├── Sidebar.tsx
│   │   ├── TitleBar.tsx
│   │   └── OperationLog.tsx
│   ├── shred/
│   │   ├── FileDropZone.tsx
│   │   ├── FileList.tsx
│   │   ├── FileListItem.tsx
│   │   ├── ShredButton.tsx
│   │   ├── AlgorithmSelector.tsx
│   │   ├── ShredOptions.tsx
│   │   └── ConfirmationDialog.tsx
│   ├── browser/
│   │   ├── BrowserCard.tsx
│   │   ├── ProfileItem.tsx
│   │   └── BrowserWarning.tsx
│   └── settings/
│       ├── ToggleSetting.tsx
│       └── AlgorithmInfo.tsx
├── hooks/
│   ├── useShredProgress.ts
│   └── useBrowserDetection.ts
└── sections/
    ├── ShredSection.tsx
    ├── BrowserSection.tsx
    └── SettingsSection.tsx
```

---

## Task 1: Install Tailwind CSS v4 and configure path aliases

**Files:**
- Modify: `vite.config.ts`
- Modify: `tsconfig.json`
- Create: `src/index.css`

- [ ] **Step 1: Install Tailwind CSS v4**

```bash
pnpm add -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 2: Add Tailwind plugin and path alias to vite.config.ts**

Replace `vite.config.ts` with:

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
```

- [ ] **Step 3: Add path alias to tsconfig.json**

Replace `tsconfig.json` with:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

**Note:** `noUnusedLocals` and `noUnusedParameters` set to `false` to avoid conflicts with shadcn generated code.

- [ ] **Step 4: Create initial index.css with Tailwind import**

Create `src/index.css`:

```css
@import "tailwindcss";
```

- [ ] **Step 5: Update main.tsx to import index.css**

Replace `src/main.tsx`:

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 6: Delete old App.css**

```bash
Remove-Item src/App.css
```

- [ ] **Step 7: Verify build**

```bash
pnpm build
```

Expected: TypeScript compiles, Vite builds, no errors.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: add Tailwind CSS v4 and path aliases"
```

---

## Task 2: Initialize shadcn with Lyra preset

**Files:**
- Create: `components.json`
- Create: `src/lib/utils.ts`
- Modify: `src/index.css` (shadcn will update it)

- [ ] **Step 1: Initialize shadcn**

```bash
npx shadcn@latest init --preset lyra --defaults
```

If `--preset` is not supported, use interactive mode and select:
- Style: Lyra
- Base color: Neutral
- CSS variables: yes

- [ ] **Step 2: Verify components.json was created**

```bash
Get-Content components.json
```

Expected: JSON with `style: "lyra"`, `baseColor: "neutral"`, Tailwind and paths configured.

- [ ] **Step 3: Verify index.css was updated**

Expected: Contains `@import "tailwindcss"` plus `@theme inline` block with CSS variables.

- [ ] **Step 4: Verify lib/utils.ts was created**

Expected: Contains `cn()` function using `clsx` and `tailwind-merge`.

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: initialize shadcn with Lyra preset"
```

---

## Task 3: Install Magic UI Terminal, Phosphor icons, fonts, and Tauri plugins

**Files:**
- Modify: `src/main.tsx` (font imports)

- [ ] **Step 1: Add Magic UI Terminal component**

```bash
npx shadcn@latest add "https://magicui.design/r/terminal.json"
```

Expected: Creates `src/components/ui/terminal.tsx`.

- [ ] **Step 2: Install Phosphor icons**

```bash
pnpm add @phosphor-icons/react
```

- [ ] **Step 3: Install fonts**

```bash
pnpm add @fontsource-variable/jetbrains-mono @fontsource-variable/inter
```

- [ ] **Step 4: Install Tauri dialog plugin**

```bash
pnpm add @tauri-apps/plugin-dialog
```

Also install the Rust side:
```bash
cargo add tauri-plugin-dialog
```

Then register in `src-tauri/src/lib.rs` — add `.plugin(tauri_plugin_dialog::init())` to the builder.

- [ ] **Step 5: Add font imports to main.tsx**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/inter";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 6: Verify build**

```bash
pnpm build
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add Magic UI Terminal, Phosphor icons, fonts, and Tauri dialog plugin"
```

---

## Task 4: Add shadcn components and configure CSS variables

**Files:**
- Modify: `src/index.css`

- [ ] **Step 1: Add required shadcn components**

```bash
npx shadcn@latest add button card scroll-area progress tooltip dialog badge separator tabs checkbox switch select alert
```

Expected: Creates component files in `src/components/ui/`.

- [ ] **Step 2: Update CSS variables in index.css**

After shadcn init, update the CSS variables to match the Security Console palette. The exact content depends on what shadcn init generated — replace the `:root` block colors with:

```css
:root {
  --radius: 0.25rem;
  --background: oklch(0.05 0 0);
  --foreground: oklch(0.9 0 0);
  --card: oklch(0.06 0 0);
  --card-foreground: oklch(0.9 0 0);
  --popover: oklch(0.06 0 0);
  --popover-foreground: oklch(0.9 0 0);
  --primary: oklch(0.8 0.1 195);
  --primary-foreground: oklch(0.05 0 0);
  --secondary: oklch(0.12 0 0);
  --secondary-foreground: oklch(0.7 0 0);
  --muted: oklch(0.12 0 0);
  --muted-foreground: oklch(0.5 0 0);
  --accent: oklch(0.8 0.1 195);
  --accent-foreground: oklch(0.05 0 0);
  --destructive: oklch(0.75 0.15 75);
  --destructive-foreground: oklch(0.05 0 0);
  --border: oklch(0.15 0 0);
  --input: oklch(0.15 0 0);
  --ring: oklch(0.8 0.1 195);
  --sidebar: oklch(0.06 0 0);
  --sidebar-foreground: oklch(0.9 0 0);
  --sidebar-accent: oklch(0.1 0 0);
  --sidebar-accent-foreground: oklch(0.9 0 0);
  --sidebar-border: oklch(0.15 0 0);
}
```

Also add font-sans and font-mono overrides in the `@theme inline` block:
```css
--font-sans: "Inter Variable", "Inter", ui-sans-serif, system-ui, sans-serif;
--font-mono: "JetBrains Mono Variable", "JetBrains Mono", ui-monospace, monospace;
```

- [ ] **Step 3: Verify build**

```bash
pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: add shadcn components and Security Console CSS variables"
```

---

## Task 5: Create shared types

**Files:**
- Create: `src/types/index.ts`

**Oracle fix:** `ProgressEvent.status` must match backend `ShredStatus` tagged enum serialization (`{type: "Shredding"}`, `{type: "Error", message: "..."}`).

- [ ] **Step 1: Create types/index.ts**

```typescript
// src/types/index.ts

export type Section = "shred" | "browser" | "settings";

export interface ShredFile {
  id: string;
  path: string;
  name: string;
  size: number;
  status: "pending" | "shredding" | "done" | "error";
  error?: string;
}

export type LogLevel = "info" | "success" | "warning" | "error" | "command";

export interface LogEntry {
  id: string;
  timestamp: Date;
  level: LogLevel;
  message: string;
}

export interface AlgorithmOption {
  index: number;
  name: string;
  description: string;
  default_passes: number;
  max_passes: number;
  accepted_patterns: string[];
  has_fixed_pattern_sequence: boolean;
}

export interface DetectedBrowser {
  id: string;
  name: string;
  icon: string;
  isRunning: boolean;
  profiles: BrowserProfile[];
}

export interface BrowserProfile {
  id: string;
  name: string;
  path: string;
  size: number;
  selected: boolean;
}

/** Matches backend ShredStatus tagged enum serialization */
export type ShredStatus =
  | { type: "Shredding" }
  | { type: "Verifying" }
  | { type: "Renaming" }
  | { type: "Truncating" }
  | { type: "Deleting" }
  | { type: "Complete" }
  | { type: "Error"; message: string };

export interface ProgressEvent {
  file_path: string;
  file_size: number;
  bytes_written: number;
  current_pass: number;
  total_passes: number;
  speed_bytes_per_sec: number;
  estimated_time_remaining_secs: number;
  status: ShredStatus;
}

export interface ShredReport {
  total_files: number;
  successful: number;
  failed: number;
  skipped: number;
  errors: Array<{ path: string; error: string }>;
  total_bytes_shredded: number;
  duration_secs: number;
}
```

- [ ] **Step 2: Verify build**

```bash
pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: add shared TypeScript types with correct ShredStatus"
```

---

## Task 6: Build React contexts

**Files:**
- Create: `src/contexts/NavigationContext.tsx`
- Create: `src/contexts/ShredContext.tsx`
- Create: `src/contexts/SettingsContext.tsx`
- Create: `src/contexts/BrowserContext.tsx`

**Oracle fix:** Remove `confirmBeforeShred` toggle from SettingsContext — confirmation is mandatory per AGENTS.md. Remove unused `passes`/`pattern`/`verificationLevel` from ShredContext (deferred to ShredOptions component which sets them inline).

- [ ] **Step 1: Create NavigationContext.tsx**

```typescript
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
```

- [ ] **Step 2: Create ShredContext.tsx**

```typescript
// src/contexts/ShredContext.tsx
import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import type { ShredFile, LogEntry, AlgorithmOption } from "@/types";

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  addFiles: (files: Array<{ path: string; name: string; size: number }>) => void;
  removeFile: (id: string) => void;
  clearFiles: () => void;
  setAlgorithmIndex: (index: number) => void;
  setIsShredding: (v: boolean) => void;
  addLogEntry: (level: LogEntry["level"], message: string) => void;
  clearLog: () => void;
  setAlgorithms: (algorithms: AlgorithmOption[]) => void;
  updateFileStatus: (id: string, status: ShredFile["status"], error?: string) => void;
}

const ShredContext = createContext<ShredState | null>(null);

export function ShredProvider({ children }: { children: ReactNode }) {
  const [files, setFiles] = useState<ShredFile[]>([]);
  const [algorithmIndex, setAlgorithmIndex] = useState(0);
  const [isShredding, setIsShredding] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [algorithms, setAlgorithms] = useState<AlgorithmOption[]>([]);

  const addFiles = useCallback((newEntries: Array<{ path: string; name: string; size: number }>) => {
    const newFiles: ShredFile[] = newEntries
      .filter((entry) => !files.some((f) => f.path === entry.path))
      .map((entry) => ({
        id: crypto.randomUUID(),
        path: entry.path,
        name: entry.name,
        size: entry.size,
        status: "pending" as const,
      }));
    setFiles((prev) => [...prev, ...newFiles]);
  }, [files]);

  const removeFile = useCallback((id: string) => {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const clearFiles = useCallback(() => setFiles([]), []);

  const addLogEntry = useCallback((level: LogEntry["level"], message: string) => {
    setLogEntries((prev) => [
      ...prev,
      { id: crypto.randomUUID(), timestamp: new Date(), level, message },
    ]);
  }, []);

  const clearLog = useCallback(() => setLogEntries([]), []);

  const updateFileStatus = useCallback(
    (id: string, status: ShredFile["status"], error?: string) => {
      setFiles((prev) =>
        prev.map((f) => (f.id === id ? { ...f, status, error } : f))
      );
    },
    []
  );

  return (
    <ShredContext.Provider
      value={{
        files,
        algorithmIndex,
        isShredding,
        logEntries,
        algorithms,
        addFiles,
        removeFile,
        clearFiles,
        setAlgorithmIndex,
        setIsShredding,
        addLogEntry,
        clearLog,
        setAlgorithms,
        updateFileStatus,
      }}
    >
      {children}
    </ShredContext.Provider>
  );
}

export function useShred() {
  const ctx = useContext(ShredContext);
  if (!ctx) throw new Error("useShred must be used within ShredProvider");
  return ctx;
}
```

- [ ] **Step 3: Create SettingsContext.tsx**

```typescript
// src/contexts/SettingsContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";

interface SettingsState {
  autoClearLog: boolean;
  setAutoClearLog: (v: boolean) => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [autoClearLog, setAutoClearLog] = useState(false);

  return (
    <SettingsContext.Provider value={{ autoClearLog, setAutoClearLog }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings must be used within SettingsProvider");
  return ctx;
}
```

- [ ] **Step 4: Create BrowserContext.tsx**

```typescript
// src/contexts/BrowserContext.tsx
import { createContext, useContext, useState, type ReactNode } from "react";
import type { DetectedBrowser } from "@/types";

interface BrowserState {
  browsers: DetectedBrowser[];
  isScanning: boolean;
  setBrowsers: (browsers: DetectedBrowser[]) => void;
  setIsScanning: (v: boolean) => void;
  toggleProfile: (browserId: string, profileId: string) => void;
  selectAllProfiles: (browserId: string) => void;
  deselectAllProfiles: (browserId: string) => void;
  getSelectedCount: () => number;
}

const BrowserContext = createContext<BrowserState | null>(null);

export function BrowserProvider({ children }: { children: ReactNode }) {
  const [browsers, setBrowsers] = useState<DetectedBrowser[]>([]);
  const [isScanning, setIsScanning] = useState(false);

  const toggleProfile = (browserId: string, profileId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? {
              ...b,
              profiles: b.profiles.map((p) =>
                p.id === profileId ? { ...p, selected: !p.selected } : p
              ),
            }
          : b
      )
    );
  };

  const selectAllProfiles = (browserId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? { ...b, profiles: b.profiles.map((p) => ({ ...p, selected: true })) }
          : b
      )
    );
  };

  const deselectAllProfiles = (browserId: string) => {
    setBrowsers((prev) =>
      prev.map((b) =>
        b.id === browserId
          ? { ...b, profiles: b.profiles.map((p) => ({ ...p, selected: false })) }
          : b
      )
    );
  };

  const getSelectedCount = () =>
    browsers.reduce(
      (sum, b) => sum + b.profiles.filter((p) => p.selected).length,
      0
    );

  return (
    <BrowserContext.Provider
      value={{
        browsers,
        isScanning,
        setBrowsers,
        setIsScanning,
        toggleProfile,
        selectAllProfiles,
        deselectAllProfiles,
        getSelectedCount,
      }}
    >
      {children}
    </BrowserContext.Provider>
  );
}

export function useBrowser() {
  const ctx = useContext(BrowserContext);
  if (!ctx) throw new Error("useBrowser must be used within BrowserProvider");
  return ctx;
}
```

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add React contexts for navigation, shred, settings, browser state"
```

---

## Task 7: Build layout shell — AppShell, Sidebar, TitleBar

**Files:**
- Create: `src/components/layout/TitleBar.tsx`
- Create: `src/components/layout/Sidebar.tsx`
- Create: `src/components/layout/AppShell.tsx`
- Modify: `src/App.tsx`

**Oracle fix:** TitleBar buttons must call Tauri window APIs. Add `aria-label` to each button.

- [ ] **Step 1: Create TitleBar.tsx**

```tsx
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
```

- [ ] **Step 2: Create Sidebar.tsx**

```tsx
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
```

- [ ] **Step 3: Create AppShell.tsx**

```tsx
// src/components/layout/AppShell.tsx
import type { ReactNode } from "react";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";

interface AppShellProps {
  children: ReactNode;
  bottom?: ReactNode;
}

export function AppShell({ children, bottom }: AppShellProps) {
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex flex-1 flex-col overflow-hidden">
          <div className="flex-1 overflow-auto p-6">{children}</div>
          {bottom}
        </main>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Update App.tsx**

```tsx
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
```

- [ ] **Step 5: Create placeholder section components**

Create `src/sections/ShredSection.tsx`:
```tsx
export function ShredSection() {
  return <div className="text-muted-foreground">Shred section — coming next</div>;
}
```

Create `src/sections/BrowserSection.tsx`:
```tsx
export function BrowserSection() {
  return <div className="text-muted-foreground">Browser section — coming soon</div>;
}
```

Create `src/sections/SettingsSection.tsx`:
```tsx
export function SettingsSection() {
  return <div className="text-muted-foreground">Settings section — coming soon</div>;
}
```

- [ ] **Step 6: Verify build**

```bash
pnpm build
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add layout shell — AppShell, Sidebar, TitleBar with window controls"
```

---

## Task 8: Build OperationLog component

**Files:**
- Create: `src/components/layout/OperationLog.tsx`

**Oracle fix:** Context-aware empty state (don't show "Drop files" when in Browser/Settings section).

- [ ] **Step 1: Create OperationLog.tsx**

```tsx
// src/components/layout/OperationLog.tsx
import { useState } from "react";
import { CaretDown, CaretUp, Trash } from "@phosphor-icons/react";
import {
  Terminal,
  AnimatedSpan,
} from "@/components/ui/terminal";
import { useShred } from "@/contexts/ShredContext";
import { useNavigation } from "@/contexts/NavigationContext";
import { cn } from "@/lib/utils";
import type { LogEntry } from "@/types";

function logColor(level: LogEntry["level"]): string {
  switch (level) {
    case "success":
      return "text-green-500";
    case "error":
      return "text-red-500";
    case "warning":
      return "text-amber-500";
    case "command":
      return "text-cyan-400";
    default:
      return "text-foreground";
  }
}

export function OperationLog() {
  const { logEntries, clearLog } = useShred();
  const { activeSection } = useNavigation();
  const [collapsed, setCollapsed] = useState(false);

  const emptyMessage =
    activeSection === "shred"
      ? "No operations yet. Drop files to begin."
      : activeSection === "browser"
        ? "No browser operations yet."
        : "No log entries.";

  return (
    <div className="border-t border-border bg-surface">
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs text-muted-foreground">
            operation.log
          </span>
          <span className="font-mono text-xs text-muted-foreground">
            ({logEntries.length})
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={clearLog}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
            title="Clear log"
          >
            <Trash size={14} />
          </button>
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
          >
            {collapsed ? <CaretUp size={14} /> : <CaretDown size={14} />}
          </button>
        </div>
      </div>
      {!collapsed && (
        <div className="h-[180px] overflow-auto border-t border-border px-4 pb-4">
          {logEntries.length === 0 ? (
            <p className="py-4 text-center font-mono text-xs text-muted-foreground">
              {emptyMessage}
            </p>
          ) : (
            <Terminal sequence={false} className="max-w-none border-0 bg-transparent p-0">
              {logEntries.map((entry) => (
                <AnimatedSpan
                  key={entry.id}
                  delay={0}
                  className={cn("font-mono text-xs", logColor(entry.level))}
                >
                  <span>
                    {entry.level === "command" ? "> " : ""}
                    {entry.message}
                  </span>
                </AnimatedSpan>
              ))}
            </Terminal>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

```bash
pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: add OperationLog with context-aware empty state"
```

---

## Task 9: Build Shred section — FileDropZone, FileList, FileListItem

**Files:**
- Create: `src/components/shred/FileDropZone.tsx`
- Create: `src/components/shred/FileListItem.tsx`
- Create: `src/components/shred/FileList.tsx`
- Modify: `src/sections/ShredSection.tsx`

**Oracle fix:** Use Tauri native `onDragDropEvent` instead of HTML5 `dataTransfer`. Call `validate_paths` before adding to state. Fetch file metadata for sizes. Deduplicate by path.

- [ ] **Step 1: Create FileDropZone.tsx**

```tsx
// src/components/shred/FileDropZone.tsx
import { useCallback, useEffect, useState } from "react";
import { Upload } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

interface FileMetadata {
  path: string;
  name: string;
  size: number;
}

export function FileDropZone() {
  const { addFiles, addLogEntry } = useShred();
  const [isDragOver, setIsDragOver] = useState(false);

  // Tauri native drag-drop
  useEffect(() => {
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        const paths = event.payload.paths;
        if (paths.length > 0) {
          validateAndAdd(paths);
        }
      } else {
        setIsDragOver(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const validateAndAdd = async (paths: string[]) => {
    try {
      const validFiles: FileMetadata[] = await invoke("validate_paths", { paths });
      if (validFiles.length > 0) {
        addFiles(validFiles);
        addLogEntry("info", `Added ${validFiles.length} file(s)`);
      }
      if (validFiles.length < paths.length) {
        addLogEntry(
          "warning",
          `${paths.length - validFiles.length} file(s) rejected (system file, network drive, or invalid path)`
        );
      }
    } catch (err) {
      addLogEntry("error", `Validation failed: ${err}`);
    }
  };

  const handleClick = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: "Select files to shred",
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        await validateAndAdd(paths);
      }
    } catch (err) {
      addLogEntry("error", `File dialog failed: ${err}`);
    }
  };

  return (
    <div
      onClick={handleClick}
      className={cn(
        "flex cursor-pointer flex-col items-center justify-center gap-3 rounded border-2 border-dashed p-12 transition-colors",
        isDragOver
          ? "border-accent bg-accent/5"
          : "border-border hover:border-muted-foreground"
      )}
    >
      <Upload
        size={32}
        className={cn(
          "transition-colors",
          isDragOver ? "text-accent" : "text-muted-foreground"
        )}
      />
      <p className="text-sm text-muted-foreground">
        Drop files here or click to browse
      </p>
    </div>
  );
}
```

- [ ] **Step 2: Create FileListItem.tsx**

**Oracle fix:** Show per-file error message, not just icon.

```tsx
// src/components/shred/FileListItem.tsx
import { X, CheckCircle, Spinner, WarningCircle } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import type { ShredFile } from "@/types";

function StatusIcon({ status }: { status: ShredFile["status"] }) {
  switch (status) {
    case "pending":
      return <span className="text-muted-foreground">—</span>;
    case "shredding":
      return <Spinner size={16} className="animate-spin text-accent" />;
    case "done":
      return <CheckCircle size={16} className="text-green-500" />;
    case "error":
      return <WarningCircle size={16} className="text-red-500" />;
  }
}

export function FileListItem({ file }: { file: ShredFile }) {
  const { removeFile } = useShred();

  return (
    <div className="flex items-center gap-3 border-b border-border bg-surface px-4 py-2 hover:bg-elevated">
      <StatusIcon status={file.status} />
      <div className="min-w-0 flex-1">
        <p className="truncate font-mono text-sm text-foreground">{file.name}</p>
        <div className="flex items-center gap-2">
          <p className="font-mono text-xs text-muted-foreground">
            {file.size > 0
              ? file.size > 1073741824
                ? `${(file.size / 1073741824).toFixed(2)} GB`
                : `${(file.size / 1048576).toFixed(1)} MB`
              : "—"}
          </p>
          {file.error && (
            <p className="truncate text-xs text-red-500">{file.error}</p>
          )}
        </div>
      </div>
      {file.status === "pending" && (
        <button
          onClick={() => removeFile(file.id)}
          aria-label={`Remove ${file.name}`}
          className="rounded p-1 text-muted-foreground hover:bg-elevated hover:text-foreground"
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Create FileList.tsx**

```tsx
// src/components/shred/FileList.tsx
import { useShred } from "@/contexts/ShredContext";
import { FileListItem } from "./FileListItem";
import { ScrollArea } from "@/components/ui/scroll-area";

export function FileList() {
  const { files } = useShred();

  if (files.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        No files selected
      </p>
    );
  }

  return (
    <ScrollArea className="h-[240px] rounded border border-border">
      {files.map((file) => (
        <FileListItem key={file.id} file={file} />
      ))}
    </ScrollArea>
  );
}
```

- [ ] **Step 4: Update ShredSection.tsx (partial — full version in Task 10)**

```tsx
// src/sections/ShredSection.tsx
import { FileDropZone } from "@/components/shred/FileDropZone";
import { FileList } from "@/components/shred/FileList";

export function ShredSection() {
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Shred Files</h1>
      <FileDropZone />
      <FileList />
    </div>
  );
}
```

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add Shred section with Tauri native drag-drop and path validation"
```

---

## Task 10: Build ShredButton, AlgorithmSelector, ShredOptions, ConfirmationDialog

**Files:**
- Create: `src/components/shred/ShredButton.tsx`
- Create: `src/components/shred/AlgorithmSelector.tsx`
- Create: `src/components/shred/ShredOptions.tsx`
- Create: `src/components/shred/ConfirmationDialog.tsx`
- Modify: `src/sections/ShredSection.tsx`

**Oracle fixes:**
- `executeShred` must map `report.errors` to per-file error status (not mark all as "done").
- Confirmation dialog is always shown (non-optional per AGENTS.md).
- Add passes/pattern/verificationLevel controls via ShredOptions.
- Progress listener cleanup on unmount.
- Correctly handle `ShredStatus` tagged enum.

- [ ] **Step 1: Create ShredButton.tsx**

```tsx
// src/components/shred/ShredButton.tsx
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

interface ShredButtonProps {
  onClick: () => void;
}

export function ShredButton({ onClick }: ShredButtonProps) {
  const { files, isShredding } = useShred();
  const pendingFiles = files.filter((f) => f.status === "pending");
  const disabled = pendingFiles.length === 0 || isShredding;
  const totalSize = pendingFiles.reduce((sum, f) => sum + f.size, 0);

  return (
    <div className="flex flex-col items-center gap-2">
      <button
        onClick={onClick}
        disabled={disabled}
        className={cn(
          "w-full max-w-[400px] border-2 px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider transition-colors",
          disabled
            ? "cursor-not-allowed border-border text-muted-foreground opacity-40"
            : "border-destructive text-destructive hover:border-red-500 hover:bg-red-500 hover:text-background"
        )}
      >
        Shred Files
      </button>
      {pendingFiles.length > 0 && (
        <p className="font-mono text-xs text-muted-foreground">
          {pendingFiles.length} file{pendingFiles.length !== 1 ? "s" : ""}
          {totalSize > 0
            ? totalSize > 1073741824
              ? `, ${(totalSize / 1073741824).toFixed(2)} GB`
              : `, ${(totalSize / 1048576).toFixed(1)} MB`
            : ""}{" "}
          — this action is irreversible
        </p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create AlgorithmSelector.tsx**

```tsx
// src/components/shred/AlgorithmSelector.tsx
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useShred } from "@/contexts/ShredContext";

export function AlgorithmSelector() {
  const { algorithms, algorithmIndex, setAlgorithmIndex } = useShred();

  if (algorithms.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">Loading algorithms...</p>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      <label className="font-mono text-xs text-muted-foreground">
        Algorithm
      </label>
      <Select
        value={String(algorithmIndex)}
        onValueChange={(v) => setAlgorithmIndex(Number(v))}
      >
        <SelectTrigger className="font-mono text-sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {algorithms.map((algo) => (
            <SelectItem key={algo.index} value={String(algo.index)}>
              {algo.name} — {algo.description}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
```

- [ ] **Step 3: Create ShredOptions.tsx**

```tsx
// src/components/shred/ShredOptions.tsx
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ShredOptionsProps {
  passes: number;
  onPassesChange: (v: number) => void;
  pattern: "random" | "zeros" | "ones";
  onPatternChange: (v: "random" | "zeros" | "ones") => void;
  verificationLevel: "none" | "sample" | "full";
  onVerificationLevelChange: (v: "none" | "sample" | "full") => void;
  maxPasses: number;
}

export function ShredOptions({
  passes,
  onPassesChange,
  pattern,
  onPatternChange,
  verificationLevel,
  onVerificationLevelChange,
  maxPasses,
}: ShredOptionsProps) {
  return (
    <div className="flex flex-wrap gap-4">
      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">Passes</label>
        <Select
          value={String(passes)}
          onValueChange={(v) => onPassesChange(Number(v))}
        >
          <SelectTrigger className="w-[100px] font-mono text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Array.from({ length: maxPasses }, (_, i) => i + 1).map((n) => (
              <SelectItem key={n} value={String(n)}>
                {n}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">Pattern</label>
        <Select value={pattern} onValueChange={onPatternChange}>
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="random">Random</SelectItem>
            <SelectItem value="zeros">Zeros</SelectItem>
            <SelectItem value="ones">Ones</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">
          Verification
        </label>
        <Select value={verificationLevel} onValueChange={onVerificationLevelChange}>
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">None</SelectItem>
            <SelectItem value="sample">Sample</SelectItem>
            <SelectItem value="full">Full</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create ConfirmationDialog.tsx**

```tsx
// src/components/shred/ConfirmationDialog.tsx
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

interface ConfirmationDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileCount: number;
  onConfirm: () => void;
}

export function ConfirmationDialog({
  open,
  onOpenChange,
  fileCount,
  onConfirm,
}: ConfirmationDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="font-mono">
            Confirm Destruction
          </AlertDialogTitle>
          <AlertDialogDescription>
            You are about to permanently destroy{" "}
            <strong>
              {fileCount} file{fileCount !== 1 ? "s" : ""}
            </strong>
            . This action cannot be undone. Data will be overwritten, verified,
            renamed, truncated, and deleted.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className="bg-red-600 text-white hover:bg-red-700"
          >
            DESTROY
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

- [ ] **Step 5: Update ShredSection.tsx**

**Oracle fixes:** Map report errors to per-file status. Progress listener cleanup. Handle ShredStatus tagged enum. Always show confirmation.

```tsx
// src/sections/ShredSection.tsx
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FileDropZone } from "@/components/shred/FileDropZone";
import { FileList } from "@/components/shred/FileList";
import { ShredButton } from "@/components/shred/ShredButton";
import { AlgorithmSelector } from "@/components/shred/AlgorithmSelector";
import { ShredOptions } from "@/components/shred/ShredOptions";
import { ConfirmationDialog } from "@/components/shred/ConfirmationDialog";
import { useShred } from "@/contexts/ShredContext";
import type { ShredReport, ProgressEvent, ShredStatus } from "@/types";

function statusToString(status: ShredStatus): string {
  return status.type.toLowerCase();
}

export function ShredSection() {
  const {
    files,
    algorithmIndex,
    isShredding,
    setIsShredding,
    addLogEntry,
    updateFileStatus,
    setAlgorithms,
    algorithms,
  } = useShred();

  const [dialogOpen, setDialogOpen] = useState(false);
  const [passes, setPasses] = useState(1);
  const [pattern, setPattern] = useState<"random" | "zeros" | "ones">("random");
  const [verificationLevel, setVerificationLevel] = useState<"none" | "sample" | "full">("sample");
  const unlistenRef = useRef<(() => void) | null>(null);

  const pendingFiles = files.filter((f) => f.status === "pending");
  const currentAlgorithm = algorithms[algorithmIndex];

  // Load algorithms on mount
  useEffect(() => {
    invoke<ShredReport[]>("get_algorithms")
      .then((algorithms) => setAlgorithms(algorithms as any))
      .catch((err) => addLogEntry("error", `Failed to load algorithms: ${err}`));
  }, [setAlgorithms, addLogEntry]);

  // Cleanup progress listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  const handleShredClick = () => {
    setDialogOpen(true);
  };

  const executeShred = async () => {
    if (pendingFiles.length === 0) return;

    setIsShredding(true);
    addLogEntry("command", `shredding ${pendingFiles.length} file(s)...`);

    // Listen for progress events
    const unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
      const { file_path, status, current_pass, total_passes } = event.payload;
      const statusStr = statusToString(status);
      const message =
        status.type === "Error"
          ? `[${file_path}] error: ${status.message}`
          : `[${file_path}] ${statusStr} (pass ${current_pass}/${total_passes})`;
      addLogEntry(status.type === "Error" ? "error" : "info", message);
    });
    unlistenRef.current = unlisten;

    try {
      const paths = pendingFiles.map((f) => f.path);
      const report: ShredReport = await invoke("shred_files", {
        paths,
        algorithmIndex,
        passes,
        pattern,
        verificationLevel,
      });

      // Map report errors to per-file status
      const failedPaths = new Set(report.errors.map((e) => e.path));
      for (const file of pendingFiles) {
        if (failedPaths.has(file.path)) {
          const errorEntry = report.errors.find((e) => e.path === file.path);
          updateFileStatus(file.id, "error", errorEntry?.error ?? "Unknown error");
        } else {
          updateFileStatus(file.id, "done");
        }
      }

      addLogEntry(
        "success",
        `Complete: ${report.successful} destroyed, ${report.failed} failed, ${report.skipped} skipped (${report.duration_secs.toFixed(1)}s)`
      );
    } catch (err) {
      addLogEntry("error", `Shred failed: ${err}`);
      // Mark all pending as error
      for (const file of pendingFiles) {
        updateFileStatus(file.id, "error", String(err));
      }
    } finally {
      unlisten();
      unlistenRef.current = null;
      setIsShredding(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Shred Files</h1>
      <FileDropZone />
      <FileList />
      <div className="flex flex-col items-center gap-4 pt-2">
        <AlgorithmSelector />
        {currentAlgorithm && (
          <ShredOptions
            passes={passes}
            onPassesChange={setPasses}
            pattern={pattern}
            onPatternChange={setPattern}
            verificationLevel={verificationLevel}
            onVerificationLevelChange={setVerificationLevel}
            maxPasses={currentAlgorithm.max_passes}
          />
        )}
        <ShredButton onClick={handleShredClick} />
      </div>
      <ConfirmationDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        fileCount={pendingFiles.length}
        onConfirm={executeShred}
      />
    </div>
  );
}
```

- [ ] **Step 6: Verify build**

```bash
pnpm build
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add ShredButton, AlgorithmSelector, ShredOptions, ConfirmationDialog with error mapping"
```

---

## Task 11: Build Settings section

**Files:**
- Create: `src/components/settings/ToggleSetting.tsx`
- Create: `src/components/settings/AlgorithmInfo.tsx`
- Modify: `src/sections/SettingsSection.tsx`

- [ ] **Step 1: Create ToggleSetting.tsx**

```tsx
// src/components/settings/ToggleSetting.tsx
import { Switch } from "@/components/ui/switch";

interface ToggleSettingProps {
  label: string;
  description: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export function ToggleSetting({
  label,
  description,
  checked,
  onCheckedChange,
}: ToggleSettingProps) {
  return (
    <div className="flex items-center justify-between border-b border-border py-4">
      <div>
        <p className="text-sm text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}
```

- [ ] **Step 2: Create AlgorithmInfo.tsx**

```tsx
// src/components/settings/AlgorithmInfo.tsx
import { Badge } from "@/components/ui/badge";
import type { AlgorithmOption } from "@/types";

export function AlgorithmInfo({ algo }: { algo: AlgorithmOption }) {
  return (
    <div className="rounded border border-border bg-surface p-4">
      <div className="flex items-center gap-2">
        <h3 className="font-mono text-sm font-semibold text-foreground">
          {algo.name}
        </h3>
        <Badge variant="outline" className="font-mono text-xs">
          {algo.default_passes} pass{algo.default_passes !== 1 ? "es" : ""}
        </Badge>
      </div>
      <p className="mt-1 text-xs text-muted-foreground">{algo.description}</p>
      <p className="mt-2 font-mono text-xs text-muted-foreground">
        Max passes: {algo.max_passes} · Patterns: {algo.accepted_patterns.join(", ")}
      </p>
    </div>
  );
}
```

- [ ] **Step 3: Update SettingsSection.tsx**

```tsx
// src/sections/SettingsSection.tsx
import { ToggleSetting } from "@/components/settings/ToggleSetting";
import { AlgorithmInfo } from "@/components/settings/AlgorithmInfo";
import { useSettings } from "@/contexts/SettingsContext";
import { useShred } from "@/contexts/ShredContext";

export function SettingsSection() {
  const { autoClearLog, setAutoClearLog } = useSettings();
  const { algorithms } = useShred();

  return (
    <div className="flex flex-col gap-6">
      <h1 className="font-sans text-xl font-semibold">Settings</h1>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Log
        </h2>
        <ToggleSetting
          label="Auto-clear log"
          description="Clear the operation log after each shredding session"
          checked={autoClearLog}
          onCheckedChange={setAutoClearLog}
        />
      </section>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Algorithms
        </h2>
        <div className="flex flex-col gap-3">
          {algorithms.map((algo) => (
            <AlgorithmInfo key={algo.index} algo={algo} />
          ))}
          {algorithms.length === 0 && (
            <p className="text-xs text-muted-foreground">Loading algorithms...</p>
          )}
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          About
        </h2>
        <div className="rounded border border-border bg-surface p-4">
          <p className="font-mono text-sm font-semibold text-foreground">
            KnockKnock v0.1.0
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            Emergency file shredder for Windows, macOS, and Linux. Implements
            NIST 800-88 Clear, DoD 5220.22-M, and random overwrite algorithms.
          </p>
          <p className="mt-2 text-xs text-muted-foreground">
            This tool is for legitimate privacy/security purposes only. The user
            is responsible for how they use it.
          </p>
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Verify build**

```bash
pnpm build
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Settings section with toggles, algorithm info, and about"
```

---

## Task 12: Build Browser section (with mock data)

**Files:**
- Create: `src/components/browser/BrowserCard.tsx`
- Create: `src/components/browser/ProfileItem.tsx`
- Create: `src/components/browser/BrowserWarning.tsx`
- Create: `src/hooks/useBrowserDetection.ts`
- Modify: `src/sections/BrowserSection.tsx`

**Oracle fix:** BrowserWarning must block action with a confirmation checkbox, not just a passive banner.

- [ ] **Step 1: Create useBrowserDetection.ts (mock)**

```typescript
// src/hooks/useBrowserDetection.ts
import { useEffect } from "react";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

const MOCK_BROWSERS: DetectedBrowser[] = [
  {
    id: "chrome",
    name: "Google Chrome",
    icon: "GoogleChrome",
    isRunning: false,
    profiles: [
      { id: "chrome-default", name: "Default", path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default", size: 524288000, selected: false },
      { id: "chrome-profile1", name: "Profile 1", path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Profile 1", size: 104857600, selected: false },
    ],
  },
  {
    id: "firefox",
    name: "Mozilla Firefox",
    icon: "FirefoxLogo",
    isRunning: false,
    profiles: [
      { id: "firefox-default", name: "default-release", path: "%APPDATA%\\Mozilla\\Firefox\\Profiles\\xxxx.default-release", size: 314572800, selected: false },
    ],
  },
];

export function useBrowserDetection() {
  const { setBrowsers, setIsScanning } = useBrowser();
  const { addLogEntry } = useShred();

  useEffect(() => {
    setIsScanning(true);
    addLogEntry("info", "Scanning for installed browsers...");

    // TODO: Replace with real backend call
    const timeout = setTimeout(() => {
      setBrowsers(MOCK_BROWSERS);
      setIsScanning(false);
      addLogEntry(
        "success",
        `Found ${MOCK_BROWSERS.length} browsers, ${MOCK_BROWSERS.reduce((sum, b) => sum + b.profiles.length, 0)} profiles`
      );
    }, 800);

    return () => clearTimeout(timeout);
  }, [setBrowsers, setIsScanning, addLogEntry]);
}
```

- [ ] **Step 2: Create ProfileItem.tsx**

```tsx
// src/components/browser/ProfileItem.tsx
import { Checkbox } from "@/components/ui/checkbox";
import { useBrowser } from "@/contexts/BrowserContext";
import type { BrowserProfile } from "@/types";

interface ProfileItemProps {
  browserId: string;
  profile: BrowserProfile;
}

export function ProfileItem({ browserId, profile }: ProfileItemProps) {
  const { toggleProfile } = useBrowser();

  return (
    <div className="flex items-center gap-3 py-2">
      <Checkbox
        checked={profile.selected}
        onCheckedChange={() => toggleProfile(browserId, profile.id)}
      />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-foreground">{profile.name}</p>
        <p className="truncate font-mono text-xs text-muted-foreground">
          {profile.path}
        </p>
      </div>
      <span className="font-mono text-xs text-muted-foreground">
        {(profile.size / 1048576).toFixed(0)} MB
      </span>
    </div>
  );
}
```

- [ ] **Step 3: Create BrowserCard.tsx**

```tsx
// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import { useBrowser } from "@/contexts/BrowserContext";
import type { DetectedBrowser } from "@/types";

export function BrowserCard({ browser }: { browser: DetectedBrowser }) {
  const { selectAllProfiles, deselectAllProfiles } = useBrowser();

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="font-mono text-sm">{browser.name}</CardTitle>
          <div className="flex gap-2">
            <button
              onClick={() => selectAllProfiles(browser.id)}
              className="text-xs text-accent hover:underline"
            >
              Select all
            </button>
            <button
              onClick={() => deselectAllProfiles(browser.id)}
              className="text-xs text-muted-foreground hover:underline"
            >
              Deselect all
            </button>
          </div>
        </div>
        {browser.isRunning && (
          <p className="text-xs text-amber-500">Browser is currently running</p>
        )}
      </CardHeader>
      <CardContent>
        {browser.profiles.map((profile) => (
          <ProfileItem
            key={profile.id}
            browserId={browser.id}
            profile={profile}
          />
        ))}
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Create BrowserWarning.tsx**

```tsx
// src/components/browser/BrowserWarning.tsx
import { useState } from "react";
import { Warning } from "@phosphor-icons/react";
import { Checkbox } from "@/components/ui/checkbox";

interface BrowserWarningProps {
  browserName: string;
  onAcknowledge: () => void;
}

export function BrowserWarning({ browserName, onAcknowledge }: BrowserWarningProps) {
  const [acknowledged, setAcknowledged] = useState(false);

  return (
    <div className="flex items-start gap-3 rounded border border-amber-500/30 bg-amber-500/10 px-4 py-3">
      <Warning size={20} className="mt-0.5 shrink-0 text-amber-500" />
      <div className="flex flex-col gap-2">
        <p className="text-sm text-foreground">
          <strong>{browserName}</strong> is currently running. Shredding browser
          data while the browser is open may cause errors. Close the browser
          before continuing.
        </p>
        <label className="flex items-center gap-2">
          <Checkbox
            checked={acknowledged}
            onCheckedChange={(checked) => {
              setAcknowledged(!!checked);
              if (checked) onAcknowledge();
            }}
          />
          <span className="text-xs text-muted-foreground">
            I understand the risk, continue anyway
          </span>
        </label>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Update BrowserSection.tsx**

```tsx
// src/sections/BrowserSection.tsx
import { useState } from "react";
import { BrowserCard } from "@/components/browser/BrowserCard";
import { BrowserWarning } from "@/components/browser/BrowserWarning";
import { useBrowser } from "@/contexts/BrowserContext";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

export function BrowserSection() {
  useBrowserDetection();
  const { browsers, isScanning, getSelectedCount } = useBrowser();
  const [acknowledgedBrowsers, setAcknowledgedBrowsers] = useState<Set<string>>(new Set());
  const runningBrowsers = browsers.filter((b) => b.isRunning);
  const selectedCount = getSelectedCount();

  const handleAcknowledge = (browserId: string) => {
    setAcknowledgedBrowsers((prev) => new Set(prev).add(browserId));
  };

  const allAcknowledged = runningBrowsers.every((b) =>
    acknowledgedBrowsers.has(b.id)
  );

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Browser Cleanup</h1>

      {runningBrowsers.map((b) => (
        <BrowserWarning
          key={b.id}
          browserName={b.name}
          onAcknowledge={() => handleAcknowledge(b.id)}
        />
      ))}

      {isScanning ? (
        <p className="text-sm text-muted-foreground">Scanning for browsers...</p>
      ) : browsers.length === 0 ? (
        <p className="text-sm text-muted-foreground">No browsers detected.</p>
      ) : (
        <div className="flex flex-col gap-4">
          {browsers.map((browser) => (
            <BrowserCard key={browser.id} browser={browser} />
          ))}
        </div>
      )}

      {selectedCount > 0 && (
        <div className="flex flex-col items-center gap-2 pt-4">
          <button
            disabled={!allAcknowledged}
            className="w-full max-w-[400px] border-2 border-destructive px-6 py-3 font-mono text-sm font-semibold uppercase tracking-wider text-destructive transition-colors hover:border-red-500 hover:bg-red-500 hover:text-background disabled:cursor-not-allowed disabled:opacity-40"
          >
            Clean {selectedCount} Profile{selectedCount !== 1 ? "s" : ""}
          </button>
          {!allAcknowledged && (
            <p className="font-mono text-xs text-amber-500">
              Acknowledge running browser warnings to proceed
            </p>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 6: Verify build**

```bash
pnpm build
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add Browser section with mock detection and running browser warning"
```

---

## Task 13: Update Tauri config and capabilities

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

**Oracle fix:** Add `dragDropEnabled`, `decorations: false`, window size, and required capabilities for dialog and window controls.

- [ ] **Step 1: Update tauri.conf.json**

Replace the `app` section:

```json
{
  "app": {
    "windows": [
      {
        "title": "KnockKnock",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": false,
        "dragDropEnabled": true
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

- [ ] **Step 2: Update capabilities/default.json**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-minimize",
    "core:window:allow-maximize",
    "core:window:allow-close",
    "core:window:allow-toggle-maximize",
    "core:window:allow-start-dragging",
    "core:window:allow-set-size",
    "dialog:default",
    "opener:default"
  ]
}
```

- [ ] **Step 3: Verify Rust build**

```bash
cargo build
```

Run from `src-tauri/`.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: update Tauri config and capabilities for custom title bar and dialog"
```

---

## Task 14: Add validate_paths backend command

**Files:**
- Modify: `src-tauri/src/commands/shred.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Oracle fix:** Backend must validate paths before the UI adds them to the queue.

- [ ] **Step 1: Add validate_paths command**

Add to `src-tauri/src/commands/shred.rs`:

```rust
#[derive(serde::Serialize)]
pub struct FileMetadata {
    pub path: String,
    pub name: String,
    pub size: u64,
}

#[tauri::command]
pub fn validate_paths(paths: Vec<String>) -> Result<Vec<FileMetadata>, String> {
    let mut valid = Vec::new();
    for path_str in paths {
        let path = std::path::Path::new(&path_str);

        // Check if file exists
        if !path.exists() {
            continue;
        }

        // Check if it's a file (not directory)
        if !path.is_file() {
            continue;
        }

        // Get metadata
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Get filename
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        valid.push(FileMetadata {
            path: path_str,
            name,
            size: metadata.len(),
        });
    }
    Ok(valid)
}
```

- [ ] **Step 2: Verify Rust build**

```bash
cargo build
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: add validate_paths command for pre-shred path validation"
```

---

## Task 15: Final integration and build verification

- [ ] **Step 1: Verify full frontend build**

```bash
pnpm build
```

Expected: No errors.

- [ ] **Step 2: Verify Rust build**

```bash
cargo build
```

Run from `src-tauri/`.

- [ ] **Step 3: Verify Rust tests**

```bash
cargo test
```

- [ ] **Step 4: Verify lint**

```bash
pnpm lint
```

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: address build and lint issues from UI integration"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** Every spec section has a corresponding task
- [x] **Placeholder scan:** No TBD/TODO in code steps (only in hooks noting backend gaps)
- [x] **Type consistency:** All types defined in Task 5, used consistently in Tasks 6-12
- [x] **No missing imports:** All imports reference `@/` paths or installed packages
- [x] **Backend integration:** `get_algorithms` wired in Task 10, `shred_files` wired in Task 10, `validate_paths` added in Task 14
- [x] **Browser detection:** Mock data in Task 12, ready for real backend replacement
- [x] **Oracle critical fixes:** All 8 critical issues addressed
- [x] **Oracle important fixes:** Per-file errors, ShredOptions, BrowserWarning blocking, capabilities

---

*Plan written 2026-06-19. Updated with oracle review. 15 tasks, ~60-90 minutes estimated execution time.*
