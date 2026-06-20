# KnockKnock — Agent Rules

**Project:** Emergency file shredder for Windows, macOS, Linux
**Stack:** Tauri 2.x (Rust backend) + React (TypeScript frontend)
**Goal:** Error-proof, fast, reliable file shredding with browser profile cleanup

---

## 1. CORE PRINCIPLES

### Reliability Is Non-Negotiable

This app destroys data permanently. A bug doesn't crash — it **shreds the wrong files** or **silently fails to shred**. Both are catastrophic.

- **NEVER** ship code that could shred unintended paths
- **NEVER** suppress errors in the shredding pipeline
- **NEVER** assume a shredding operation succeeded — verify it
- **ALWAYS** fail loud, never silent
- **ALWAYS** confirm before destructive operations

### Engineering Philosophy

- **KISS:** Minimum code that solves the problem
- **YAGNI:** No features beyond what was asked
- **SOLID:** Clear responsibilities, single source of truth
- **Error-proof:** Every error path must be handled and surfaced

---

## 2. TECH STACK RULES

### Tauri 2.x + Rust Backend

- **Rust edition 2021**, stable toolchain only
- **Tauri 2.x** with `tauri-plugin-shell`, `tauri-plugin-updater`, `tauri-plugin-dialog`
- All file shredding logic lives in Rust — never in the frontend
- Frontend calls Rust via Tauri `invoke()` commands only
- **NEVER** use `unsafe` Rust unless absolutely necessary and documented why

### React Frontend (TypeScript)

- **TypeScript strict mode** — no `as any`, `@ts-ignore`, `@ts-expect-error`
- **Functional components** only — no class components
- **React Router** for navigation (if multi-page needed)
- **Tailwind CSS** for styling — utility-first, no custom CSS unless unavoidable
- **shadcn/ui** for components when applicable

### Package Manager

- **pnpm ONLY** — never npm or yarn
- If `package-lock.json` or `yarn.lock` appears, delete immediately
- Install: `npm install -g pnpm`

### Build Commands

```bash
pnpm dev          # Development server (Tauri + React hot reload)
pnpm build        # Production build
pnpm lint         # ESLint + clippy
pnpm test         # Rust tests + Vitest
```

---

## 3. FILE SHREDDING RULES (CRITICAL)

### Shredding Pipeline

Every shred operation MUST follow this exact sequence:

1. **Validate** — Confirm path exists, is not a system file, is not a network drive
2. **Detect media** — SSD vs HDD (different strategies)
3. **Overwrite** — Single-pass random data (NIST 800-88 Clear)
4. **Verify** — Read-back sample blocks (start/middle/end)
5. **Rename** — Random filename (obliterate directory entry)
6. **Truncate** — Set file size to 0
7. **Delete** — Remove file entry
8. **Report** — Return success/failure with details

**NEVER skip steps. NEVER reorder steps. NEVER combine steps.**

### What You Must NEVER Shred

- System directories (`/System`, `C:\Windows`, `/usr`)
- The app's own binary/config files
- Paths outside user's home directory (unless explicitly confirmed)
- Network drives / mounted shares
- Symlinks pointing to protected locations

### Locked Files

- If file is in use: report error with process name holding the lock
- On Windows: offer `MoveFileEx(DELAY_UNTIL_REBOOT)` as fallback
- On macOS/Linux: offer to terminate holding process (user confirmation required)
- **NEVER** silently skip locked files — always report

### SSD vs HDD

- Detect drive type before shredding
- HDD: Full single-pass overwrite
- SSD: Single-pass + TRIM, warn user about wear-leveling limitations
- Document limitation: multi-pass shredding is ineffective on SSDs

---

## 4. BROWSER DETECTION RULES

### Detection Approach

- Check known paths per OS (see `src/config/browsers.ts`)
- **NEVER** hardcode browser paths inline — centralize in config
- Detect running processes before attempting to shred browser data
- If browser is running: **MUST** warn user and require confirmation

### Browser Data Shredding

- Target: profiles, cache, cookies, history databases, password stores
- **NEVER** touch Safari Keychain (system-level, dangerous)
- **NEVER** shred browser data while browser is running without explicit user consent
- Report exactly which browsers were found and what was shredded

---

## 5. AGENT BEHAVIOR RULES

### User Authority

- **User makes ALL decisions.** Agent proposes, user decides.
- **Ask before acting** on ambiguous requests
- **Never assume** — state assumptions and ask for confirmation
- If a request risks data loss, **STOP and ask**

### Output Efficiency

- Extreme brevity. No fluff, no praise, no cheerleading.
- Never output entire files unless commanded
- Use surgical edits (diffs/snippets) for code modifications
- Present options concisely, recommend one, wait for decision

### Precision Coding Protocol

- Modify ONLY the specific lines required for the task
- Do not refactor or reorganize unless explicitly commanded
- Don't "improve" adjacent code, comments, or formatting
- Match existing style, even if you'd do it differently
- When changes create orphans, remove ONLY what YOUR changes made unused
- Every changed line must trace directly to the user's request

### Read Before Write

- **ALWAYS** read a file before modifying it
- Review `Cargo.toml`, `package.json`, `tauri.conf.json` before every modification
- Review complex `.tsx` and `.rs` files before editing

### Complexity Guard

- If a request makes code considerably more complex, **STOP and warn the user**
- State the complexity increase and ask if they want to proceed

### Stability Guard

- If a bug repeats 3 times: **STOP and summarize**
- Review relevant files before every modification attempt
- Report what you found and propose a different approach

### No Silent Failing

- **IMMEDIATELY report** any error (build, lint, test, git, runtime, platform API)
- Never suppress, ignore, or continue past failures
- Surface all errors explicitly with context

### Verification-First Mindset

- Never assume implementation works because it compiles
- Provide a "Verification Plan" for every feature
- Run tests after every change when applicable
- For shredding code: **MUST** test with temporary files, never real data

---

## 6. GIT WORKFLOW

### Auto Commit After Each Turn

After every implementation cycle:

1. `git add .`
2. `git commit -m "Subject" -m "- Bullet list of changes"`
3. `git push` (when remote is configured)

### Commit Message Format

- Present tense: "Add file validation" not "Added file validation"
- Under 72 chars subject
- Body: bulleted list for multi-file changes

### Git Summarize

At end of response:

```
**Git Summarize**
- **Commit ID:** [git rev-parse --short HEAD]
- **Commit Subject:** [subject]
- **Commit Description:** [bulleted changes]
- **Pending:** [git status -s or "Clean"]
```

### Git Error Reporting

**IMMEDIATELY report** any git commit/push/pull errors:

```
Git Error: [description]
Command: [exact command]
Error: [error message]
Suggestion: [fix or ask user]
```

---

## 7. WINDOWS POWERSHELL RULES

**PowerShell does NOT support `&&`.**

| Wrong | Correct |
|-------|---------|
| `cmd1 && cmd2` | `cmd1; if ($?) { cmd2 }` |
| `cd dir && cmd` | `cmd` with `workdir` parameter |

**Always use `workdir` parameter instead of `cd` inside commands.**

---

## 8. DYNAMIC RULE MAINTENANCE

When a significant architectural change or permanent constraint is implemented:

1. Propose a new rule for AGENTS.md
2. Check for conflicts with existing rules
3. **ONLY add if user explicitly approves**

---

## 9. PROJECT STRUCTURE

```
KnockKnock/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs      # Tauri entry
│   │   ├── shredder/    # File shredding engine
│   │   ├── browser/     # Browser detection
│   │   └── commands/    # Tauri IPC commands
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                 # React frontend
│   ├── components/      # UI components
│   ├── hooks/           # React hooks
│   ├── utils/           # Frontend utilities
│   ├── types/           # TypeScript types
│   └── App.tsx          # Main app + routes
├── AGENTS.md            # This file
├── package.json
└── README.md
```

---

## 10. SKILL EVALUATION

Before ANY task, evaluate available skills. If a skill matches even remotely, load it.

| Task | Skills to check |
|------|----------------|
| UI/styling | `frontend-design`, `tailwind-css-patterns`, `shadcn` |
| Rust code | `nodejs-best-practices` (patterns), `typescript-advanced-types` |
| Testing | `vitest`, `webapp-testing` |
| Git/workflow | `verification-before-completion`, `requesting-code-review` |
| Debugging | `diagnose`, `systematic-debugging` |
| Planning | `writing-plans`, `brainstorming` |

---

## 11. TAURI V2 GOTCHAS (DESKTOP ONLY)

These rules prevent silent failures specific to Tauri 2.x. Sourced from official Tauri v2 docs and verified failure modes. Mobile-specific rules intentionally omitted — KnockKnock is desktop-only for now.

### Setup & Build

- **`tauri migrate` is a starter, not a complete fix.** It handles manifest config but NOT
  JS API renames, capability file creation, or permission string migration. Run it,
  then manually verify frontend imports and capability files.
  *(Tauri v2 migration: from-tauri-1)*

- **lib.rs owns all logic.** `src-tauri/src/main.rs` must be a thin passthrough:
  `fn main() { app_lib::run() }`. `Cargo.toml` requires
  `[lib] name = "app_lib" crate-type = ["staticlib", "cdylib", "rlib"]`.
  Recommended even for desktop-only — enables future mobile support and cleaner testing.

### Commands

- **Every `#[tauri::command]` must be in `generate_handler![...]`.** Missing entries
  produce no compile error — `invoke()` returns "command not found" at runtime silently.
  Only the last `invoke_handler` call wins.
  *(Tauri v2 docs: calling-rust)*

- **Async commands cannot use borrowed types.** `async fn bad(name: &str)` fails with
  cryptic "`__tauri_message__` does not live long enough". Use owned types: `String`,
  `Vec<u8>`. `State<'_, T>` works, but the inner type must be `Send + Sync`.
  *(GitHub issue #6733)*

- **Custom error types crossing IPC must `impl Serialize`.** `Result<T, MyError>` without
  `Serialize` silently serializes as `{}`. Use `thiserror` + manual `impl Serialize` that
  forwards to `Display`. Apply to every error type returned from any command.

- **`State<T>` requires interior mutability.** Plain `State<'_, AppState>` where
  `AppState { counter: u32 }` panics on access. Wrap with `Mutex<AppState>` and pass
  `Mutex::new(...)` to `.manage()`. For async commands, use `tokio::sync::Mutex`.

- **`State<T>` type must match `.manage()` exactly.** `State<'_, Mutex<AppState>>` panics
  if `.manage()` was passed `AppState` directly. Type mismatch = runtime panic, not
  compile error. Double-check type parameter on every `.manage()` / `State<...>` pair.

### IPC

- **`invoke` vs `emit` vs `Channel` — choose correctly.**
  - `invoke` = request/response, awaits result
  - `emit`/`listen` = broadcast pub-sub, no acknowledgment
  - `Channel<T>` = ordered high-frequency streams (progress updates)

- **Plugin imports moved from `@tauri-apps/api/*` to `@tauri-apps/plugin-*`.**
  - `invoke` → `@tauri-apps/api/core`
  - Dialog, fs, http, shell → `@tauri-apps/plugin-<name>`
  Old paths don't exist in v2 — `pnpm install` the plugin package separately.
  *(Tauri v2 migration: JavaScript API changes)*

### Capabilities & Permissions

- **Capabilities are deny-by-default.** Installing a plugin (`.plugin(...)` + `pnpm install`)
  is NOT enough. Without the matching permission string in
  `src-tauri/capabilities/default.json`, the operation silently fails — no panic, just a
  rejected Promise with "not allowed".
  *(Tauri v2 docs: security/capabilities)*

- **Plugin `.init()` call is mandatory.** Adding a plugin to `Cargo.toml` does not register it.
  All three steps required: Cargo.toml + `.plugin(tauri_plugin_X::init())` in builder +
  permission string in capability file. Missing any one = silent failure.

- **`shell:allow-execute` requires explicit scope.** No scope = all commands denied.
  Define: `allow: [{ "name": "git", "cmd": "git", "args": true }]`. Wildcards for all
  arguments rejected for security. **Do not grant this permission to KnockKnock — it
  has no shell-execute use case.**

- **`fs:default` is intentionally restricted.** Grants read access only to `$APPDATA`,
  `$APPCONFIG`, etc. For KnockKnock's file shredding across user-selected paths, scope
  explicitly to `$HOME/**` (read/write/remove) — do NOT use `fs:allow-read-all` /
  `fs:allow-write-all` (excessively broad for a file shredder).

### Platform-Specific

- **Windows production uses `http://tauri.localhost` (not `https://`).** This resets
  IndexedDB/LocalStorage/cookies on upgrade unless `app.windows.useHttpsScheme: true` is
  set in `tauri.conf.json`. Affects any frontend persistence layer.
  *(Tauri v2 migration: new-origin-url-on-windows)*

---

## 12. HINDSIGHT MEMORY (OBLIGATORY)

This project uses Hindsight long-term memory. Bank ID: **`KnockKnock`**.

### Retention Policy

- **MANDATORY** — call `hindsight_retain` at these checkpoints:
  - Every 5 implementation steps
  - End of every session
  - After any architectural decision
  - After any installation / dependency change
  - After any rule update to this file

- **Tools available:**
  - `hindsight_retain` — store new information
  - `hindsight_recall` — search past context
  - `hindsight_reflect` — synthesize answer from memory

- **Related skill:** `hindsight-docs` — load for Hindsight API reference when needed.

### What to Retain

- Project context (stack, constraints, target users)
- Architectural decisions with rationale
- Skipped alternatives with reasons (prevents re-research)
- Bug root causes and fixes
- Cross-session state (in-progress work)
- User preferences and standing rules

### What NOT to Retain

- Credentials, API keys, secrets
- Personal file paths or PII
- Transient session state
- Full file contents (use references instead)

---

## SECURITY NOTES

- This tool is for **legitimate privacy/security purposes only**
- Include disclaimer in README and UI
- Never implement features that could be used for malicious purposes
- User is responsible for how they use the tool
