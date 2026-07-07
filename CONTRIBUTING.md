# Contributing to KnockKnock

Thanks for your interest in contributing. This document covers how to get started.

## Prerequisites

- [Node.js](https://nodejs.org/) v18+
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- Platform-specific build tools:
  - **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++ workload)
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`)
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-4-dev`

## Setup

```bash
git clone https://github.com/knockknockshredder/knockknock.git
cd knockknock
pnpm install
pnpm tauri dev
```

## Development Workflow

1. Fork the repo
2. Create a branch: `git checkout -b feature/your-feature`
3. Make your changes
4. Run checks: `pnpm tauri build` (ensures both frontend and Rust compile)
5. Commit with a clear message
6. Push and open a pull request

## Code Guidelines

### Rust

- Rust edition 2021, stable toolchain only
- No `unsafe` unless absolutely necessary and documented why
- All errors must be handled and surfaced — never suppress errors
- Use `thiserror` for error types, `serde` for serialization
- Every `#[tauri::command]` must be in the `generate_handler![]` macro

### TypeScript

- Strict mode — no `as any`, `@ts-ignore`, `@ts-expect-error`
- Functional components only
- Tailwind CSS for styling — utility-first, no custom CSS unless unavoidable

### General

- Keep changes focused — one feature or fix per PR
- Don't refactor unrelated code
- Match existing code style even if you'd do it differently
- Read files before modifying them

## Testing

```bash
# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend tests (if configured)
pnpm test
```

For shredding code, always test with temporary files in a temp directory — never real data.

## Commit Messages

- Present tense: "Add file validation" not "Added file validation"
- Under 72 characters for the subject
- Body: bulleted list for multi-file changes

## Reporting Bugs

Open an issue with:
- Steps to reproduce
- Expected behavior
- Actual behavior
- OS and version
- KnockKnock version

## Security Vulnerabilities

Do NOT open a public issue. See [SECURITY.md](SECURITY.md) for reporting instructions.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
