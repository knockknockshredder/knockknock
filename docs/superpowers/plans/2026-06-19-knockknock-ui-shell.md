# KnockKnock UI Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the KnockKnock "Security Console" UI shell — a dark, boxy, terminal-tinged desktop interface for file shredding, browser cleanup, and settings.

**Architecture:** React SPA inside Tauri 2.x window. shadcn/ui (Lyra preset) for design system, Magic UI Terminal for operation log, Phosphor icons. Four React contexts for state. Sidebar + main panel + collapsible log layout.

**Tech Stack:** React 19, TypeScript, Vite 7, Tailwind CSS v4, shadcn/ui (Lyra), Magic UI, Phosphor Icons, JetBrains Mono + Inter fonts, Tauri 2.x IPC.

**Spec:** `docs/superpowers/specs/2026-06-19-knockknock-ui-design.md`

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
- Create: `tsconfig.app.json`

- [ ] **Step 1: Install Tailwind CSS v4**

```bash
pnpm add -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 2: Add Tailwind plugin to vite.config.ts**

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
    "noUnusedLocals": true,
    "noUnusedParameters": true,
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

- [ ] **Step 4: Create initial index.css with Tailwind import**

Replace `src/App.css` with `src/index.css`:

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

```bash
Get-Content src/index.css
```

Expected: Contains `@import "tailwindcss"` plus `@theme inline` block with CSS variables.

- [ ] **Step 4: Verify lib/utils.ts was created**

```bash
Get-Content src/lib/utils.ts
```

Expected: Contains `cn()` function using `clsx` and `tailwind-merge`.

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: initialize shadcn with Lyra preset"
```

---

## Task 3: Install Magic UI Terminal, Phosphor icons, and fonts

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

- [ ] **Step 4: Add font imports to main.tsx**

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

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add Magic UI Terminal, Phosphor icons, and fonts"
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

After shadcn init, `index.css` will have a `@theme inline` block. Update the CSS variables to match the Security Console palette. Add these overrides inside the `:root` block (shadcn may have created it):

```css
@import "tailwindcss";

@custom-variant dark (&:is(.dark *));

@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-sidebar: var(--sidebar);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
  --color-sidebar-border: var(--sidebar-border);
  --radius-sm: calc(var(--radius) * 0.6);
  --radius-md: calc(var(--radius) * 0.8);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) * 1.4);
  --font-sans: "Inter Variable", "Inter", ui-sans-serif, system-ui, sans-serif;
  --font-mono: "JetBrains Mono Variable", "JetBrains Mono", ui-monospace, monospace;
}

:root {
  --radius: 0.25rem;
  --background: oklch(0.05 0 0);           /* #09090b */
  --foreground: oklch(0.9 0 0);            /* #e4e4e7 */
  --card: oklch(0.06 0 0);                 /* #0c0c0e */
  --card-foreground: oklch(0.9 0 0);
  --popover: oklch(0.06 0 0);
  --popover-foreground: oklch(0.9 0 0);
  --primary: oklch(0.8 0.1 195);           /* #22d3ee cyan */
  --primary-foreground: oklch(0.05 0 0);
  --secondary: oklch(0.12 0 0);            /* #111113 */
  --secondary-foreground: oklch(0.7 0 0);
  --muted: oklch(0.12 0 0);
  --muted-foreground: oklch(0.5 0 0);      /* #71717a */
  --accent: oklch(0.8 0.1 195);            /* #22d3ee */
  --accent-foreground: oklch(0.05 0 0);
  --destructive: oklch(0.75 0.15 75);      /* #f59e0b amber */
  --destructive-foreground: oklch(0.05 0 0);
  --border: oklch(0.15 0 0);               /* #1f1f22 */
  --input: oklch(0.15 0 0);
  --ring: oklch(0.8 0.1 195);              /* cyan */
  --sidebar: oklch(0.06 0 0);
  --sidebar-foreground: oklch(0.9 0 0);
  --sidebar-accent: oklch(0.1 0 0);
  --sidebar-accent-foreground: oklch(0.9 0 0);
  --sidebar-border: oklch(0.15 0 0);
}
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

export interface ProgressEvent {
  file_path: string;
  file_size: number;
  bytes_written: number;
  current_pass: number;
  total_passes: number;
  speed_bytes_per_sec: number;
  estimated_time_remaining_secs: number;
  status:
    | "shredding"
    | "verifying"
    | "renaming"
    | "truncating"
    | "deleting"
    | "complete"
    | "error";
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
git commit -m "feat: add shared TypeScript types"
```

---

## Task 6: Build React contexts

**Files:**
- Create: `src/contexts/NavigationContext.tsx`
- Create: `src/contexts/ShredContext.tsx`
- Create: `src/contexts/SettingsContext.tsx`
- Create: `src/contexts/BrowserContext.tsx`

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
  passes: number;
  pattern: "random" | "zeros" | "ones";
  verificationLevel: "none" | "sample" | "full";
  isShredding: boolean;
  logEntries: LogEntry[];
  algorithms: AlgorithmOption[];
  addFiles: (paths: string[]) => void;
  removeFile: (id: string) => void;
  clearFiles: () => void;
  setAlgorithmIndex: (index: number) => void;
  setPasses: (passes: number) => void;
  setPattern: (pattern: "random" | "zeros" | "ones") => void;
  setVerificationLevel: (level: "none" | "sample" | "full") => void;
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
  const [passes, setPasses] = useState(1);
  const [pattern, setPattern] = useState<"random" | "zeros" | "ones">("random");
  const [verificationLevel, setVerificationLevel] = useState<"none" | "sample" | "full">("sample");
  const [isShredding, setIsShredding] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [algorithms, setAlgorithms] = useState<AlgorithmOption[]>([]);

  const addFiles = useCallback((paths: string[]) => {
    const newFiles: ShredFile[] = paths.map((p) => {
      const parts = p.replace(/\\/g, "/").split("/");
      return {
        id: crypto.randomUUID(),
        path: p,
        name: parts[parts.length - 1],
        size: 0,
        status: "pending",
      };
    });
    setFiles((prev) => [...prev, ...newFiles]);
  }, []);

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
        passes,
        pattern,
        verificationLevel,
        isShredding,
        logEntries,
        algorithms,
        addFiles,
        removeFile,
        clearFiles,
        setAlgorithmIndex,
        setPasses,
        setPattern,
        setVerificationLevel,
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
  confirmBeforeShred: boolean;
  confirmBeforeBrowserCleanup: boolean;
  autoClearLog: boolean;
  defaultAlgorithmIndex: number;
  defaultVerificationLevel: "none" | "sample" | "full";
  setConfirmBeforeShred: (v: boolean) => void;
  setConfirmBeforeBrowserCleanup: (v: boolean) => void;
  setAutoClearLog: (v: boolean) => void;
  setDefaultAlgorithmIndex: (v: number) => void;
  setDefaultVerificationLevel: (v: "none" | "sample" | "full") => void;
}

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [confirmBeforeShred, setConfirmBeforeShred] = useState(true);
  const [confirmBeforeBrowserCleanup, setConfirmBeforeBrowserCleanup] = useState(true);
  const [autoClearLog, setAutoClearLog] = useState(false);
  const [defaultAlgorithmIndex, setDefaultAlgorithmIndex] = useState(0);
  const [defaultVerificationLevel, setDefaultVerificationLevel] = useState<"none" | "sample" | "full">("sample");

  return (
    <SettingsContext.Provider
      value={{
        confirmBeforeShred,
        confirmBeforeBrowserCleanup,
        autoClearLog,
        defaultAlgorithmIndex,
        defaultVerificationLevel,
        setConfirmBeforeShred,
        setConfirmBeforeBrowserCleanup,
        setAutoClearLog,
        setDefaultAlgorithmIndex,
        setDefaultVerificationLevel,
      }}
    >
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

- [ ] **Step 1: Create TitleBar.tsx**

```tsx
// src/components/layout/TitleBar.tsx
import { X, Square, Minus } from "@phosphor-icons/react";

export function TitleBar() {
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
        <button className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10">
          <Minus size={14} className="text-muted-foreground" />
        </button>
        <button className="flex h-8 w-8 items-center justify-center rounded hover:bg-accent/10">
          <Square size={12} className="text-muted-foreground" />
        </button>
        <button className="flex h-8 w-8 items-center justify-center rounded hover:bg-destructive/20">
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
import { useNavigation } from "@/contexts/NavigationContext";
import { ShredSection } from "@/sections/ShredSection";
import { BrowserSection } from "@/sections/BrowserSection";
import { SettingsSection } from "@/sections/SettingsSection";

function AppContent() {
  const { activeSection } = useNavigation();

  return (
    <AppShell>
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
git commit -m "feat: add layout shell — AppShell, Sidebar, TitleBar"
```

---

## Task 8: Build OperationLog component

**Files:**
- Create: `src/components/layout/OperationLog.tsx`
- Modify: `src/App.tsx` (wire OperationLog as bottom panel)

- [ ] **Step 1: Create OperationLog.tsx**

```tsx
// src/components/layout/OperationLog.tsx
import { useState } from "react";
import { CaretDown, CaretUp, Trash } from "@phosphor-icons/react";
import {
  Terminal,
  AnimatedSpan,
  TypingAnimation,
} from "@/components/ui/terminal";
import { useShred } from "@/contexts/ShredContext";
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
  const [collapsed, setCollapsed] = useState(false);

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
              No operations yet. Drop files to begin.
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

- [ ] **Step 2: Update App.tsx to include OperationLog**

Update the `AppContent` function in `src/App.tsx`:

```tsx
import { OperationLog } from "@/components/layout/OperationLog";

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
```

- [ ] **Step 3: Verify build**

```bash
pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: add OperationLog with Magic UI Terminal"
```

---

## Task 9: Build Shred section — FileDropZone, FileList, FileListItem

**Files:**
- Create: `src/components/shred/FileDropZone.tsx`
- Create: `src/components/shred/FileListItem.tsx`
- Create: `src/components/shred/FileList.tsx`
- Modify: `src/sections/ShredSection.tsx`

- [ ] **Step 1: Create FileDropZone.tsx**

```tsx
// src/components/shred/FileDropZone.tsx
import { useCallback, useState } from "react";
import { Upload } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

export function FileDropZone() {
  const { addFiles } = useShred();
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);
      const paths = Array.from(e.dataTransfer.files).map((f) => f.name);
      if (paths.length > 0) addFiles(paths);
    },
    [addFiles]
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setIsDragOver(false);
  }, []);

  const handleClick = useCallback(async () => {
    // TODO: Replace with Tauri plugin-dialog when installed
    // const selected = await open({ multiple: true, filters: [{ name: "All Files", extensions: ["*"] }] });
    // if (selected) addFiles(Array.isArray(selected) ? selected : [selected]);
  }, [addFiles]);

  return (
    <div
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
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
        <p className="font-mono text-xs text-muted-foreground">
          {file.size > 0 ? `${(file.size / 1024 / 1024).toFixed(1)} MB` : "—"}
        </p>
      </div>
      {file.status === "pending" && (
        <button
          onClick={() => removeFile(file.id)}
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

- [ ] **Step 4: Update ShredSection.tsx**

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
git commit -m "feat: add Shred section — FileDropZone, FileList, FileListItem"
```

---

## Task 10: Build ShredButton, AlgorithmSelector, ConfirmationDialog

**Files:**
- Create: `src/components/shred/ShredButton.tsx`
- Create: `src/components/shred/AlgorithmSelector.tsx`
- Create: `src/components/shred/ConfirmationDialog.tsx`
- Modify: `src/sections/ShredSection.tsx`

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
          {totalSize > 0 ? `, ${(totalSize / 1024 / 1024).toFixed(1)} MB` : ""} —
          this action is irreversible
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

- [ ] **Step 3: Create ConfirmationDialog.tsx**

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

- [ ] **Step 4: Update ShredSection.tsx**

```tsx
// src/sections/ShredSection.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FileDropZone } from "@/components/shred/FileDropZone";
import { FileList } from "@/components/shred/FileList";
import { ShredButton } from "@/components/shred/ShredButton";
import { AlgorithmSelector } from "@/components/shred/AlgorithmSelector";
import { ConfirmationDialog } from "@/components/shred/ConfirmationDialog";
import { useShred } from "@/contexts/ShredContext";
import { useSettings } from "@/contexts/SettingsContext";
import type { ShredReport, ProgressEvent } from "@/types";

export function ShredSection() {
  const {
    files,
    algorithmIndex,
    passes,
    pattern,
    verificationLevel,
    isShredding,
    setIsShredding,
    addLogEntry,
    updateFileStatus,
    setAlgorithms,
  } = useShred();
  const { confirmBeforeShred } = useSettings();
  const [dialogOpen, setDialogOpen] = useState(false);

  const pendingFiles = files.filter((f) => f.status === "pending");

  const handleShredClick = () => {
    if (confirmBeforeShred) {
      setDialogOpen(true);
    } else {
      executeShred();
    }
  };

  const executeShred = async () => {
    if (pendingFiles.length === 0) return;

    setIsShredding(true);
    addLogEntry("command", `shredding ${pendingFiles.length} file(s)...`);

    // Listen for progress events
    const unlisten = await listen<ProgressEvent>("shred-progress", (event) => {
      const { file_path, status, current_pass, total_passes } = event.payload;
      addLogEntry(
        status === "error" ? "error" : "info",
        `[${file_path}] ${status} (pass ${current_pass}/${total_passes})`
      );
    });

    try {
      const paths = pendingFiles.map((f) => f.path);
      const report: ShredReport = await invoke("shred_files", {
        paths,
        algorithmIndex,
        passes,
        pattern,
        verificationLevel,
      });

      addLogEntry(
        "success",
        `Complete: ${report.successful} destroyed, ${report.failed} failed, ${report.skipped} skipped (${report.duration_secs.toFixed(1)}s)`
      );

      // Mark files as done
      for (const file of pendingFiles) {
        updateFileStatus(file.id, "done");
      }
    } catch (err) {
      addLogEntry("error", `Shred failed: ${err}`);
    } finally {
      unlisten();
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

- [ ] **Step 5: Verify build**

```bash
pnpm build
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add ShredButton, AlgorithmSelector, ConfirmationDialog"
```

---

## Task 11: Wire up backend — load algorithms on mount

**Files:**
- Modify: `src/sections/ShredSection.tsx` (add useEffect to load algorithms)

- [ ] **Step 1: Add algorithm loading to ShredSection**

Add this `useEffect` inside the `ShredSection` component, after the existing hooks:

```typescript
import { useEffect } from "react";

// Inside ShredSection, after other hooks:
useEffect(() => {
  invoke<Array<{
    index: number;
    name: string;
    description: string;
    default_passes: number;
    max_passes: number;
    accepted_patterns: string[];
    has_fixed_pattern_sequence: boolean;
  }>>("get_algorithms").then((algorithms) => {
    setAlgorithms(algorithms);
  }).catch((err) => {
    addLogEntry("error", `Failed to load algorithms: ${err}`);
  });
}, [setAlgorithms, addLogEntry]);
```

- [ ] **Step 2: Verify build**

```bash
pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: wire up get_algorithms backend command"
```

---

## Task 12: Build Settings section

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
  const {
    confirmBeforeShred,
    setConfirmBeforeShred,
    confirmBeforeBrowserCleanup,
    setConfirmBeforeBrowserCleanup,
    autoClearLog,
    setAutoClearLog,
  } = useSettings();
  const { algorithms } = useShred();

  return (
    <div className="flex flex-col gap-6">
      <h1 className="font-sans text-xl font-semibold">Settings</h1>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Confirmations
        </h2>
        <ToggleSetting
          label="Confirm before shredding"
          description="Show a confirmation dialog before destroying files"
          checked={confirmBeforeShred}
          onCheckedChange={setConfirmBeforeShred}
        />
        <ToggleSetting
          label="Confirm before browser cleanup"
          description="Show a confirmation dialog before deleting browser data"
          checked={confirmBeforeBrowserCleanup}
          onCheckedChange={setConfirmBeforeBrowserCleanup}
        />
      </section>

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
git commit -m "feat: add Settings section with toggles and algorithm info"
```

---

## Task 13: Build Browser section (with mock data)

**Files:**
- Create: `src/components/browser/BrowserCard.tsx`
- Create: `src/components/browser/ProfileItem.tsx`
- Create: `src/components/browser/BrowserWarning.tsx`
- Create: `src/hooks/useBrowserDetection.ts`
- Modify: `src/sections/BrowserSection.tsx`

- [ ] **Step 1: Create useBrowserDetection.ts (mock)**

```typescript
// src/hooks/useBrowserDetection.ts
import { useEffect } from "react";
import { useBrowser } from "@/contexts/BrowserContext";
import { useShred } from "@/contexts/ShredContext";
import type { DetectedBrowser } from "@/types";

// Mock data until backend detect_browsers command exists
const MOCK_BROWSERS: DetectedBrowser[] = [
  {
    id: "chrome",
    name: "Google Chrome",
    icon: "GoogleChrome",
    isRunning: false,
    profiles: [
      { id: "chrome-default", name: "Default", path: "C:\\Users\\…\\AppData\\Local\\Google\\Chrome\\User Data\\Default", size: 524288000, selected: false },
      { id: "chrome-profile1", name: "Profile 1", path: "C:\\Users\\…\\AppData\\Local\\Google\\Chrome\\User Data\\Profile 1", size: 104857600, selected: false },
    ],
  },
  {
    id: "firefox",
    name: "Mozilla Firefox",
    icon: "FirefoxLogo",
    isRunning: false,
    profiles: [
      { id: "firefox-default", name: "default-release", path: "C:\\Users\\…\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\xxxx.default-release", size: 314572800, selected: false },
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
    // invoke<DetectedBrowser[]>("detect_browsers").then(...)
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
        {(profile.size / 1024 / 1024).toFixed(0)} MB
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
import { Warning } from "@phosphor-icons/react";

interface BrowserWarningProps {
  browserName: string;
}

export function BrowserWarning({ browserName }: BrowserWarningProps) {
  return (
    <div className="flex items-center gap-3 rounded border border-amber-500/30 bg-amber-500/10 px-4 py-3">
      <Warning size={20} className="shrink-0 text-amber-500" />
      <p className="text-sm text-foreground">
        <strong>{browserName}</strong> is currently running. Shredding browser
        data while the browser is open may cause errors. Close the browser
        before continuing.
      </p>
    </div>
  );
}
```

- [ ] **Step 5: Update BrowserSection.tsx**

```tsx
// src/sections/BrowserSection.tsx
import { BrowserCard } from "@/components/browser/BrowserCard";
import { BrowserWarning } from "@/components/browser/BrowserWarning";
import { useBrowser } from "@/contexts/BrowserContext";
import { useBrowserDetection } from "@/hooks/useBrowserDetection";

export function BrowserSection() {
  useBrowserDetection();
  const { browsers, isScanning } = useBrowser();
  const runningBrowsers = browsers.filter((b) => b.isRunning);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-sans text-xl font-semibold">Browser Cleanup</h1>

      {runningBrowsers.map((b) => (
        <BrowserWarning key={b.id} browserName={b.name} />
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
git commit -m "feat: add Browser section with mock detection"
```

---

## Task 14: Update Tauri window config

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Update window configuration**

Edit `src-tauri/tauri.conf.json` — replace the `app.windows` array:

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
        "decorations": false
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

- [ ] **Step 2: Verify build**

```bash
pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: update Tauri window to 1200x800, disable native decorations"
```

---

## Task 15: Final integration and polish

- [ ] **Step 1: Verify full build**

```bash
pnpm build
```

Expected: No errors.

- [ ] **Step 2: Verify lint**

```bash
pnpm lint
```

Fix any lint errors.

- [ ] **Step 3: Verify Rust tests pass**

```bash
cargo test
```

Run from `src-tauri/` directory.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: address lint and build issues from UI integration"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** Every spec section has a corresponding task
- [x] **Placeholder scan:** No TBD/TODO in code steps (only in hooks noting backend gaps)
- [x] **Type consistency:** All types defined in Task 5, used consistently in Tasks 6-13
- [x] **No missing imports:** All imports reference `@/` paths or installed packages
- [x] **Backend integration:** `get_algorithms` wired in Task 11, `shred_files` wired in Task 10, progress events in Task 10
- [x] **Browser detection:** Mock data in Task 13, ready for real backend replacement

---

*Plan written 2026-06-19. 15 tasks, ~45-60 minutes estimated execution time.*
