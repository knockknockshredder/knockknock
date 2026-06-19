# KnockKnock UI Design Spec

**Date:** 2026-06-19
**Status:** Approved
**Style:** Security Console (shadcn Lyra preset + Magic UI Terminal)

---

## 1. Overview

KnockKnock is an emergency file shredder desktop app built with Tauri 2.x (Rust backend) and React (TypeScript frontend). The UI must communicate precision, seriousness, and trust — this is a tool that permanently destroys data. The design avoids cliché "hacker" aesthetics (no scanlines, no CRT effects, no random glitch) in favor of a clean, dark "security console" look: boxy panels, monospace typography, a live terminal log, and intentional color restraint.

---

## 2. Design Principles

- **Fail loud, never silent.** Every destructive action requires confirmation. Every error is surfaced visibly in the UI and the terminal log.
- **No AI slop.** No generic gradients, glassmorphism, excessive rounded corners, or decorative noise. Every pixel serves a function.
- **Terminal feel, GUI convenience.** The operation log uses Magic UI Terminal with animated typing. The rest of the app is a standard desktop GUI with drag-drop, checkboxes, and dialogs.
- **Dark by default, dark only.** No light mode. The app is always dark to reduce eye strain during focused security tasks.

---

## 3. Color Tokens

```css
:root {
  --background:         #09090b;
  --surface:            #0c0c0e;
  --elevated:           #111113;
  --border:             #1f1f22;
  --text:               #e4e4e7;
  --muted:              #71717a;
  --accent:             #22d3ee;   /* cyan — active nav, terminal prompt, links */
  --destructive:        #f59e0b;   /* amber — idle SHRED button */
  --destructive-hover:  #ef4444;   /* red — hover over SHRED */
  --success:            #22c55e;   /* verification passed, deleted */
  --warning:            #f59e0b;   /* locked files, browser running */
}
```

**Usage rules:**
- `background`: app root, empty space
- `surface`: cards, panels, sidebar
- `elevated`: hover states, active nav items, modal backdrop
- `border`: 1px hairlines only. No shadows.
- `accent`: never used for primary actions. Used for information, active states, and terminal prompts.
- `destructive`: the only warm color. Amber at rest → red on hover. Two-stage danger signal.

---

## 4. Typography

| Role | Font | Size | Weight |
|------|------|------|--------|
| UI labels, nav, buttons | JetBrains Mono | 13-14px | 400-500 |
| Body text | Geist Sans (or Inter) | 14px | 400 |
| Headings (H1, H2) | Geist Sans | 20-24px | 600 |
| Terminal log | JetBrains Mono | 12px | 400 |
| File metadata (size, path) | JetBrains Mono | 12px | 400 |

**Line height:** 1.5 for body, 1.4 for UI labels, 1.6 for terminal.

---

## 5. Spacing & Shape

- **Border radius:** `4px` maximum for buttons and inputs. `0px` for panels, cards, sidebar, terminal. Lyra's boxy DNA.
- **Panel padding:** 20px-24px
- **Element gap:** 8px-12px
- **Sidebar:** 64px (collapsed, icon-only) or 200px (expanded, icon + label). Toggle via button.
- **Section max-width:** None; panels fill available space.

---

## 6. Layout

### 6.1 Window Configuration

Update `tauri.conf.json`:
```json
{
  "width": 1200,
  "height": 800,
  "minWidth": 900,
  "minHeight": 600
}
```

### 6.2 Shell Structure

```
┌─────────────────────────────────────────────────────┐
│  KNOCKKNOCK                              [_][□][×] │  ← TitleBar (custom)
├──────────┬──────────────────────────────────────────┤
│ Sidebar  │ MainPanel                                │
│ (64px or │                                          │
│  200px)  │  [content based on active section]       │
│          │                                          │
├──────────┼──────────────────────────────────────────┤
│          │ OperationLog (collapsible, ~180px)       │
└──────────┴──────────────────────────────────────────┘
```

- **TitleBar:** Custom Tauri title bar. App name left, window controls right. No menu bar in the web layer.
- **Sidebar:** Vertical nav. Items: Shred, Browser, Settings. Active item has `elevated` background + `accent` left border (2px).
- **MainPanel:** Scrollable content area. Changes based on active section.
- **OperationLog:** Collapsible bottom panel. Always visible in Shred section, collapsible elsewhere. Uses Magic UI Terminal component.

### 6.3 Sections

1. **Shred** — File drop zone, file list, shred controls, algorithm selector, confirmation dialog
2. **Browser** — Detected browsers list, profile checkboxes, cleanup confirmation
3. **Settings** — Algorithm defaults, confirmation toggles, verification level, about

---

## 7. Component Specifications

### 7.1 Layout Components

#### `AppShell`
- Root layout component.
- Props: `children` (MainPanel content)
- Holds `Sidebar`, `MainPanel`, and `OperationLog`.
- Manages sidebar expanded/collapsed state.

#### `Sidebar`
- Vertical flex container, full height.
- Nav items rendered as buttons.
- Each item: Phosphor icon (24px) + optional label.
- Active state: `background: var(--elevated)`, `border-left: 2px solid var(--accent)`.
- Hover state: `background: var(--elevated)`.
- Collapse toggle at bottom.

#### `TitleBar`
- Custom Tauri title bar using `data-tauri-drag-region`.
- Left: app icon (16px) + "KNOCKKNOCK" in JetBrains Mono, 13px, uppercase, letter-spacing 0.1em.
- Right: minimize, maximize, close buttons (simple icons, no OS chrome).

#### `OperationLog`
- Wraps Magic UI `Terminal` component.
- Receives log entries via `ShredContext`.
- Entries have levels: `info` (white), `success` (green), `warning` (amber), `error` (red), `command` (cyan).
- New entries auto-scroll to bottom.
- Collapsible via chevron button.

### 7.2 Shred Section Components

#### `FileDropZone`
- Large dashed border area (`border: 2px dashed var(--border)`).
- Text: "Drop files here or click to browse" in `muted`.
- On drag-over: border changes to `accent`, background to `elevated`.
- On drop: validates files (via backend), adds to `ShredContext`.
- On click: opens Tauri file dialog (`open` from `@tauri-apps/plugin-dialog` — **NOT YET INSTALLED**, must be added).

#### `FileList`
- Scrollable list (`ScrollArea` from shadcn).
- Empty state: centered "No files selected" in `muted`.
- Each item is a `FileListItem`.

#### `FileListItem`
- Horizontal flex row.
- Left: file type icon (Phosphor, based on extension).
- Middle: filename (truncated with ellipsis), file size in `muted` below.
- Right: status icon (pending = `—`, processing = spinner, done = `✓`, error = `✗`), remove button (`X`).
- Background: `surface`. Hover: `elevated`.
- Border-bottom: `1px solid var(--border)`.

#### `ShredButton`
- Large button, full width of its container (max 400px centered).
- Default: `background: transparent`, `border: 2px solid var(--destructive)`, `color: var(--destructive)`.
- Hover: `background: var(--destructive-hover)`, `color: var(--background)`, `border-color: var(--destructive-hover)`.
- Disabled: `opacity: 0.4`, no hover effect.
- Text: "SHRED FILES" in JetBrains Mono, 14px, uppercase.
- Subtext below (when files present): "{N} files, {size} total — this action is irreversible" in `muted`, 12px.

#### `AlgorithmSelector`
- Select dropdown (shadcn `Select`).
- Options populated from backend `get_algorithms()` command.
- Default: first algorithm (NIST Clear).
- Each option shows name + short description.

#### `ConfirmationDialog`
- shadcn `Dialog` / `AlertDialog`.
- Triggered on ShredButton click.
- Title: "Confirm Destruction"
- Body: "You are about to permanently destroy {N} file(s). This action cannot be undone."
- Footer: "Cancel" (secondary) + "DESTROY" (destructive, red).

### 7.3 Browser Section Components

#### `BrowserDetector`
- Runs on Browser section mount.
- Shows scanning state: spinner + "Scanning for browsers..." in terminal log.
- **NOTE:** Backend browser detection commands do not yet exist. The UI must be built with placeholder data and a TODO hook for real backend integration.

#### `BrowserCard`
- Card panel for each detected browser.
- Header: browser icon (Phosphor), browser name, detected profile count.
- Body: list of `ProfileItem`.
- Footer: "Select All" / "Deselect All" links.

#### `ProfileItem`
- Horizontal row: checkbox, profile name, path (truncated), estimated size.
- Checkbox: shadcn `Checkbox`.

#### `BrowserWarning`
- Full-width banner at top of Browser section.
- Background: `warning` at 10% opacity, border: `1px solid var(--warning)`.
- Icon: warning triangle (Phosphor).
- Text: "{Browser} is currently running. Shredding browser data while the browser is open may cause errors."
- Action: "Close browser and continue" (if we can detect process) or just "I understand, continue" checkbox.

### 7.4 Settings Section Components

#### `ToggleSetting`
- Horizontal row: label left, switch right.
- Label: setting name + description below in `muted`.
- Switch: shadcn `Switch`.

#### `AlgorithmInfo`
- Expandable card for each algorithm.
- Shows: name, description, security level badge (e.g., "NIST 800-88 Clear"), default passes, max passes.
- **NOTE:** This is informational only. Actual algorithm selection happens in the Shred section.

---

## 8. State Management

Four React contexts, all using simple `useState` + `useReducer`. No external state library needed for this scope.

### 8.1 `NavigationContext`
```typescript
type Section = 'shred' | 'browser' | 'settings';
interface NavigationState {
  activeSection: Section;
  sidebarExpanded: boolean;
}
```

### 8.2 `ShredContext`
```typescript
interface ShredFile {
  id: string;           // crypto.randomUUID()
  path: string;
  name: string;
  size: number;
  status: 'pending' | 'shredding' | 'done' | 'error';
  error?: string;
}

interface ShredState {
  files: ShredFile[];
  algorithmIndex: number;
  passes: number;
  pattern: 'random' | 'zeros' | 'ones';
  verificationLevel: 'none' | 'sample' | 'full';
  isShredding: boolean;
  logEntries: LogEntry[];
}

type LogEntry = {
  id: string;
  timestamp: Date;
  level: 'info' | 'success' | 'warning' | 'error' | 'command';
  message: string;
};
```

### 8.3 `BrowserContext`
```typescript
interface DetectedBrowser {
  id: string;
  name: string;
  icon: string;         // phosphor icon name
  isRunning: boolean;
  profiles: BrowserProfile[];
}

interface BrowserProfile {
  id: string;
  name: string;
  path: string;
  size: number;
  selected: boolean;
}

interface BrowserState {
  browsers: DetectedBrowser[];
  isScanning: boolean;
  selectedCount: number;
}
```

### 8.4 `SettingsContext`
```typescript
interface SettingsState {
  confirmBeforeShred: boolean;
  confirmBeforeBrowserCleanup: boolean;
  autoClearLog: boolean;
  defaultAlgorithmIndex: number;
  defaultVerificationLevel: 'none' | 'sample' | 'full';
}
```
**Persistence:** Saved to disk via Tauri `localStorage` equivalent or a simple JSON file. **NOT YET IMPLEMENTED** in backend — frontend must gracefully handle absence.

---

## 9. Backend Integration

### 9.1 Existing Commands

| Command | Args | Returns | Used By |
|---------|------|---------|---------|
| `shred_files` | `paths: string[]`, `algorithm_index: number`, `passes: number`, `pattern: PatternType`, `verification_level: VerificationLevel` | `ShredReport` | Shred section |
| `get_algorithms` | none | `AlgorithmInfo[]` | Settings, AlgorithmSelector |

### 9.2 Required Commands (Not Yet Implemented)

| Command | Purpose | Section |
|---------|---------|---------|
| `detect_browsers` | Scan OS for installed browsers and profiles | Browser |
| `is_browser_running` | Check if browser process is active | Browser |
| `shred_browser_profiles` | Delete selected browser profiles | Browser |
| `get_settings` | Load persisted settings | Settings |
| `save_settings` | Persist settings | Settings |
| `open_file_dialog` | Open native file picker | Shred |
| `validate_paths` | Check if paths are safe to shred | Shred |

### 9.3 Progress Events

The backend uses `TauriProgressReporter` to emit progress events. The frontend must listen via Tauri `listen('shred-progress', handler)`.

`ProgressEvent` structure:
```typescript
interface ProgressEvent {
  file_path: string;
  file_size: number;
  bytes_written: number;
  current_pass: number;
  total_passes: number;
  speed_bytes_per_sec: number;
  estimated_time_remaining_secs: number;
  status: 'shredding' | 'verifying' | 'renaming' | 'truncating' | 'deleting' | 'complete' | 'error';
}
```

Each progress event is converted to a `LogEntry` and appended to `OperationLog`.

---

## 10. Animation & Motion

| Element | Animation | Details |
|---------|-----------|---------|
| Terminal log | `TypingAnimation` (Magic UI) | New log entries type in character by character. Duration: 30ms/char. |
| Terminal spans | `AnimatedSpan` (Magic UI) | Success/error lines fade in with slight Y translate. |
| File list items | Exit animation | On shred complete, item fades out over 300ms and collapses height. |
| Shred button | Hover transition | 150ms ease on background/border/color. |
| Sidebar | Collapse | 200ms ease on width. Content fades. |
| Dialog | Open/close | 150ms fade + scale(0.98 → 1). |
| Page transitions | None | Instant switch. This is a tool, not a marketing site. |

**No other animations.** No page transitions, no skeleton loaders (use spinners), no parallax, no particle effects.

---

## 11. Accessibility

- All interactive elements must have visible focus rings (`2px solid var(--accent)`).
- Color is never the sole indicator of state. Icons + text always.
- Dialogs trap focus and close on Escape.
- Sidebar nav is a `<nav>` with `<ul>` / `<li>` structure.
- Terminal log is a `<pre>` / `<code>` region with `aria-live="polite"`.
- Shred button has `aria-describedby` pointing to the subtext warning.

---

## 12. Dependencies

### 12.1 New Packages to Install

```bash
# Tailwind CSS v4
pnpm add -D tailwindcss @tailwindcss/vite

# shadcn/ui base
npx shadcn@latest init --preset lyra

# shadcn components
npx shadcn@latest add button card scroll-area progress tooltip dialog badge separator tabs checkbox switch select alert

# Magic UI
npx shadcn@latest add https://magicui.design/r/terminal.json

# Icons
pnpm add @phosphor-icons/react

# Fonts
pnpm add @fontsource-variable/jetbrains-mono @fontsource-variable/inter

# Tauri plugins (not yet installed)
# pnpm add @tauri-apps/plugin-dialog
```

### 12.2 Existing Packages (No Changes)

- `react`, `react-dom` (v19)
- `@tauri-apps/api`, `@tauri-apps/plugin-opener`
- `typescript`, `vite`, `@vitejs/plugin-react`

---

## 13. File Structure

```
src/
├── App.tsx                          # AppShell + providers
├── main.tsx                         # Entry point, font imports
├── App.css                          # Global styles, CSS variables
├── components/
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
│   │   ├── BrowserDetector.tsx
│   │   ├── BrowserCard.tsx
│   │   ├── ProfileItem.tsx
│   │   └── BrowserWarning.tsx
│   └── settings/
│       ├── ToggleSetting.tsx
│       └── AlgorithmInfo.tsx
├── contexts/
│   ├── NavigationContext.tsx
│   ├── ShredContext.tsx
│   ├── BrowserContext.tsx
│   └── SettingsContext.tsx
├── hooks/
│   ├── useShredProgress.ts          # Tauri event listener
│   └── useBrowserDetection.ts       # Placeholder for backend
├── types/
│   └── index.ts                     # Shared TypeScript types
├── lib/
│   └── utils.ts                     # cn() and helpers
└── sections/
    ├── ShredSection.tsx
    ├── BrowserSection.tsx
    └── SettingsSection.tsx
```

---

## 14. Open Questions / TODOs

1. **Tauri plugin-dialog** is not installed. File drop zone click-to-browse requires it.
2. **Browser detection backend** does not exist. Browser section UI must be built with mock data.
3. **Settings persistence backend** does not exist. Settings will be in-memory only initially.
4. **Custom title bar** requires Tauri window decorations to be disabled in `tauri.conf.json`.
5. **Drag-and-drop** file handling: Tauri has `dragDropEnabled` in window config. Need to verify if we use Tauri's native drag-drop or HTML5 `dragover`/`drop`.

---

## 15. Implementation Order

1. Install Tailwind + shadcn (Lyra) + Magic UI Terminal + Phosphor icons + fonts
2. Set up CSS variables and global styles
3. Build layout shell (AppShell, Sidebar, TitleBar, OperationLog)
4. Build Shred section (FileDropZone, FileList, ShredButton, AlgorithmSelector, ConfirmationDialog)
5. Wire up existing backend commands (`get_algorithms`, `shred_files`)
6. Build Settings section (placeholder, reads `get_algorithms`)
7. Build Browser section (with mock data, ready for backend)
8. Add animations and polish
9. Accessibility pass
10. Oracle review for gaps

---

## 16. Risk Assessment

| Risk | Mitigation |
|------|------------|
| shadcn init fails on Vite non-Next.js project | shadcn v2 supports Vite. Use `--base-color neutral` and manual preset config if `--preset lyra` fails. |
| Magic UI Terminal conflicts with Tailwind v4 | Magic UI supports Tailwind v4. If issues arise, pin to compatible version or fork component. |
| Tauri custom title bar breaks window controls | Test on Windows, macOS, Linux. Fallback to native decorations if unstable. |
| File drag-drop not working in Tauri window | Use Tauri `tauri://drag-drop` event via `@tauri-apps/api/window`. Have HTML5 fallback. |
| Designer copy is weak | Orchestrator will review and fix all user-facing copy after design work. |

---

*Spec written 2026-06-19. Approved by user. Next step: implementation plan.*
