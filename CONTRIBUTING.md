# Contributing to Cyron

Thank you for your interest in contributing to Cyron!

Cyron is built with **Tauri v2**, **Rust**, and **React 19 / TypeScript / Vite 7**. It runs exclusively on Windows 10/11 and interfaces directly with native Win32 APIs.

---

## Prerequisites

1. **Windows 10 / 11** (64-bit).
2. **Visual Studio 2022** with the "Desktop development with C++" workload (MSVC toolchain).
3. **Node.js 22 LTS** and `npm`.
4. **Rust toolchain** (`stable-x86_64-pc-windows-msvc`).

> [!IMPORTANT]
> Git for Windows includes a coreutils `link.exe` that can shadow the Visual Studio linker. Always use `node scripts/msvc-env.mjs` to invoke `cargo`, `tauri`, or compilation tools if you are not running from a Visual Studio Developer Command Prompt.

---

## Development Setup

```powershell
# 1. Install frontend dependencies
npm install

# 2. Run frontend typecheck and tests
npm run typecheck
npm test

# 3. Run Rust unit tests
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml

# 4. Launch Tauri dev window
npm run dev:desktop
```

---

## Architecture & Safety Rules

All contributions must follow these strict safety principles:

1. **Read-Only by Default**:
   Scanning, duplicate detection, disk categorization, and inventory reading must remain strictly read-only.
2. **Review-First Destructive Operations**:
   - Files are moved into quarantine batches, never deleted in-place.
   - Destructive operations require server-side, single-use, expiring confirmation plans (`CleanupPlan`, `UninstallPlan`, `LeftoverPlan`).
   - The frontend never supplies executables, arguments, or raw file paths for destructive operations.
3. **Registry Protection**:
   - Deleting a registry key requires a verified recursive `.reg` backup preserving all value types and subkeys.
   - Shared vendor keys (`Microsoft`, `Google`, `Apple`, `Intel`, etc.) and system roots are strictly protected.
4. **Process Manager Safety**:
   - Processes are matched only by executable image path within the application's verified install directory.
   - Matching by process name alone is strictly forbidden.
   - Windows critical system processes (`csrss.exe`, `explorer.exe`, `services.exe`, etc.) can never be terminated.
5. **No Shell Invocations**:
   - Unregistered executables, shell interpreters (`cmd.exe`, `powershell.exe`, `pwsh`, `bash`), and scripts are rejected.
   - User input is never interpolated into command lines.
6. **No Network Code**:
   Cyron does not make network requests or collect telemetry.

---

## Verification Suite

Before submitting a pull request, run the complete verification suite:

```powershell
# Frontend checks
npm run typecheck
npm test
npm run build
node scripts/verify-dist.mjs

# Rust test suites
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture
node scripts/verify-cleanup.mjs

# Desktop build and smoke test
npm run build:desktop
node scripts/verify-desktop-binary.mjs
node scripts/smoke-desktop.mjs
```

---

## Documentation Integrity

Keep documentation accurate and honest:
- Never claim benchmarks, signed binaries, or verification passes that were not actually executed.
- Record any platform limitations or known constraints in `docs/ARCHITECTURE.md` and `HANDOFF.md`.
