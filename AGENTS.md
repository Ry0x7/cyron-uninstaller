# AGENTS.md — Cyron Uninstaller

Canonical architectural invariants, safety rules, and developer commands for AI coding agents and human contributors.

## What this is

Cyron is a Windows desktop application (Tauri v2 + Rust backend, React 19 + TypeScript frontend) combining deep uninstallation (Revo-style), visual disk exploration (WinDirStat/WizTree-style), and review-first developer/AI cache cleaning (BleachBit-style) with transparent risk communication, crash-consistent quarantine, and zero telemetry.

## Stack & Platform

- **Backend:** Tauri v2, Rust (`windows-sys`, `winreg`, `tauri-plugin-dialog`, `sha2`). Windows 10/11 x64 only. Win32 APIs are called unconditionally; there is no cross-platform layer.
- **Frontend:** React 19, TypeScript, Vite 7. No router, no external chart library. Hand-built design tokens (`src/styles/tokens.css`, `src/styles/app.css`) and primitives (`src/components/ui.tsx`).
- **Toolchain:** MSVC is mandatory. Use `node scripts/msvc-env.mjs` for all Cargo and Tauri commands to avoid Git's coreutils `link.exe` shadowing the MSVC linker.

## Architectural Invariants (Non-Negotiable)

1. **IPC Surface:** 34 commands in `src-tauri/src/commands.rs`, registered in `lib.rs`. Payloads use `#[serde(rename_all = "camelCase")]`; `src/lib/types.ts` mirrors the contract. Change both together. Never accept free-form execution commands from the frontend.
2. **Scanner is Strictly Read-Only:** Directory walk (`fs.rs`, `scan.rs`), direct NTFS MFT reader (`mft.rs`), duplicate finder (`duplicates.rs`), and storage categorization (`storage.rs`) never modify the filesystem. Reparse points (junctions, symlinks) are never followed.
3. **Review-First Destructive Operations:** All modifications (cache cleaning, uninstallation, process termination, leftover removal) require server-held, single-use, 10-minute expiring confirmation plans (`CleanupPlan`, `UninstallPlan`, `BatchUninstallPlan`, `LeftoverPlan`).
4. **Quarantine Over Deletion:** Files are moved into `%APPDATA%\dev.cyron.uninstaller\quarantine\<run-id>\` using Win32 renames; files are never deleted in place during cleanup. Every move is preceded by a durable write-ahead recovery journal (`recovery_journal.jsonl`) with `sync_all`. `purge_quarantine` is the only permanent removal and is strictly manifest-scoped.
5. **Atomic No-Overwrite Restores:** Restoring quarantined files uses `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`. Existing files at destination are never overwritten.
6. **Process Termination Safety:** Targets are matched only by executable image path within the application install directory. Critical Windows processes (`csrss.exe`, `explorer.exe`, `services.exe`, etc.) are unconditionally protected. PID creation times (`GetProcessTimes`) are re-validated immediately before termination to prevent PID recycling races.
7. **Leftovers & Multi-Type Registry Backup:** Registry subkey deletion requires an automated, recursive `.reg` export supporting all 6 value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`). System roots and shared vendor keys (`Software\Microsoft`, `Software\Google`, etc.) are protected.
8. **App Discovery:** Win32 (HKLM/HKCU 64/32-bit), Microsoft Store (`AppModel\Repository\Packages`, read-only inventory), Steam (`libraryfolders.vdf`), and portable heuristics (`%LOCALAPPDATA%\Programs`). Store apps display guidance to Windows Settings / `winget`.
9. **Shell Integration (`explorer.rs`):** Per-user shell verbs registered under `HKCU\Software\Classes` without elevation. CLI startup routes (`--analyze`, `--duplicates`, `--storage`, etc.) route to views; they never execute destructive actions directly.
10. **Zero Telemetry:** The application contains no network stack or tracking code.

## Git Workflow: Strict No-Rebase Policy

**Rebasing is permanently prohibited.** Never run `git rebase` or force-push. All branch integrations use standard merge commits (`git merge --no-ff`) or squash merges. This preserves immutable commit provenance, cryptographic hashes, and release audit trails. Enforced locally via `scripts/setup-git-hooks.ps1` and remotely by GitHub branch rules (`allow_rebase_merge: false`, `allow_force_pushes: false`).

## Essential Commands

```powershell
npm install
npm run dev:desktop        # Desktop window with Vite dev server
npm run build:desktop      # Standalone debug executable (no bundle)
npm run typecheck          # TypeScript check (0 errors)
npm test                   # Frontend Vitest suite
npm run docs:check         # Verify documentation integrity
npm run build:unsigned     # Unsigned release bundles (NSIS & MSI)
npm run build:dev-signed   # Build and sign with local dev certificate
npm run verify:signatures  # Authenticode signature & negative checks
npm run verify:metadata    # Windows PE VERSIONINFO & MSI audit
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/verify-cleanup.mjs
node scripts/verify-e2e.mjs
node scripts/verify-release-bundles.mjs
```

## Critical Gotchas

- **App Data vs WebView2 Cache:** Application data (`history.json`, `quarantine/`, `cleanup-log.json`) lives in `%APPDATA%\dev.cyron.uninstaller\` (`app.path().app_data_dir()`). The WebView2 browser runtime cache lives in `%LOCALAPPDATA%\dev.cyron.uninstaller\EBWebView\`.
- **Read-Only Mode Scope:** Cyron starts with `read_only = true`. This switch specifically gates cache cleanup (`start_cleanup`) and leftover residual quarantine (`clean_app_leftovers`). Process termination and uninstaller execution are independently governed by single-use server confirmation plans and process snapshot verification.
- **Quarantine Limits:** Storage limit is capped at 20 GB (`MAX_QUARANTINE_TOTAL_BYTES`) with a 512 MB low-disk-space abort threshold.
- **MFT vs Walk Counting:** MFT counts physical records; directory walk counts accessible filesystem paths (deduplicating up to 1M hard-link IDs). Volume totals differ by design.
- **Test Fixtures:** Cleanup fixtures use `%TEMP%\cyron-cleanup-<pid>` and are cleaned up on drop. Never test against real user caches.

## Authoritative Documentation Map

- Architecture & IPC: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Security & Threat Model: [docs/SECURITY.md](docs/SECURITY.md)
- Feature Matrix & Acceptance: [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md)
- Build, Packaging & Signing: [docs/BUILD_AND_SIGNING.md](docs/BUILD_AND_SIGNING.md)
- Verification Gates Ledger: [GATES.md](GATES.md)
- End-User Guide: [docs/USER_GUIDE.md](docs/USER_GUIDE.md)
- Benchmarks & Counting Rules: [docs/BENCHMARKS.md](docs/BENCHMARKS.md)
- Active Engineering State: [HANDOFF.md](HANDOFF.md)
