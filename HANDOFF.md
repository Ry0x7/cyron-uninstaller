# Capability work — 2026-11-09

Implementation adds separate Analyzer selection plans for recycling/permanent deletion, selected-drive Cleaner scanning and evidence-based removed-app review, WER diagnostic targets, cross-volume quarantine/restore, stronger stale-file refusal, and truthful single/batch uninstall results. Standard uninstall reviews the server-resolved command and skips the process page when no processes need review. Ten Luna subagents handled scoped implementation, reviews and fixtures; the orchestrator integrated and independently verified their work. Existing uncommitted Uninstaller work was retained and corrected.

Verification completed for the scoped acceptance gates G57–G61 (5/5 met). TypeScript, 92 frontend tests, the full Rust suite (207 passed, 4 ignored), documentation integrity, the MSVC desktop debug build and isolated cleanup/uninstall/process/registry/recovery fixtures passed. The ignored interactive Recycle Bin marker fixture was separately invoked and passed. All ten Luna leaves were parent-reviewed and their ownership leases released. Existing release/signing evidence below predates these changes.

Desktop verification covered the normal 1320x860 and minimum 1000x680 client sizes, plus expanded Uninstaller: loading, empty search and recovery, protected-target error, review, live progress, safe cancellation, partial completion, stale-selection invalidation, C-only Cleaner scope and Store guidance. A 4000-file disposable folder stopped after 2115 deletions; an independent count confirmed 1885 remaining files and preserved unrelated siblings. Two final UI findings were corrected and reinspected: the permanent-action error title and the drive selector hidden behind inventory loading. The [visual receipt](.unlazy/capabilities/visual/visual-evidence.json) contains eight checks, captures and source hashes; its integrity check passed. Ignored .unlazy artifacts are local evidence and are not shipped.

Not exercised: real user cache mutation, real vendor uninstallation, Store removal, exhaustive screen-reader/provider/state coverage, signing and installer-release verification. Mutation, process and registry tests used disposable fixtures only; real inventory/cache measurements were read-only. No commits, pushes, rebases or telemetry changes were made.

Known scope limits: removed-app identities last for this session; remnant age is not in the existing candidate contract; unknown stale installers are not classified; Store removal stays in Windows Settings/winget. Cross-volume quarantine/restore is supported by Cleaner, including its known-removed-app review. Uninstaller's existing residual-file engine still uses same-volume renames and reports failures for files on another volume; those filesystem remnants can instead be reviewed through Cleaner. Windows shell recycling cannot atomically bind its final pathname operation to Cyron's file handle against another process controlling the same user session. Scanning stays read-only; Cleaner and leftovers stay quarantined. Tests only modify disposable fixtures.

---
# Cyron — Active Handoff

Updated: 2026-11-08 (Milestone 7.4: Hunter Navigation & Explorer Destructive Actions)

## 1. Current Verified State

- **Branch / Commit:** `main` (incorporating Milestone 7.4 Hunter Mode sidebar global action, confirmation dialog, external HWND targeting, and Explorer Force Delete / Locking Processes / Kill Process verbs with Windows Restart Manager)
- **Current Version:** `0.1.0` (Release Candidate 1)
- **Tag / Published Prerelease:** `v0.1.0` at `https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.1.0`
- **Published Release Assets:**
  - `Cyron Uninstaller_0.1.0_x64-setup.exe` (NSIS Installer)
  - `Cyron Uninstaller_0.1.0_x64_en-US.msi` (WiX MSI Installer)
  - `cyron.exe` (Standalone Portable Executable)
  - `SHA256SUMS.txt` (Cryptographic verification manifest)
- **IPC Surface:** 51 registered commands in `src-tauri/src/lib.rs` and `commands.rs` (including `inspect_delete_target`, `find_locking_processes`, `prepare_force_delete_plan`, `execute_force_delete`, `prepare_kill_process_plan`, and `terminate_matching_process`).
- **Git History Audit:** Complete. Clean commit tree verified with zero `.chryypt` session files in Git history. `.chryypt/` is permanently excluded via `.gitignore`. Strict No-Rebase policy enforced via local hooks and GitHub settings.

## 2. Recent Test Evidence

| Verification Target | Command | Result |
|---|---|---|
| TypeScript Typecheck | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend Unit Tests | `npm test` | 83/83 pass across 9 files |
| Frontend Production Build | `node scripts/verify-dist.mjs` | `DIST_OK` |
| Rust Core & Unit Tests | `node scripts/msvc-env.mjs cargo test --lib` | 154/154 pass (2 ignored) |
| Security Boundaries | `... --test security -- --nocapture` | 26/26 pass (includes snapshot isolation and explorer boundaries) |
| Cleanup Stress Tests | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Isolated Cleanup Fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real External HWND E2E Fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK` (8/8 pass with real GUI HWND) |
| Hunter Benchmark & Stability | `node scripts/verify-hunter-perf.mjs` | `HUNTER_BENCHMARK_OK` (p50: ~0.80 µs Win32 targeting, bounded memory) |
| Scanner Stress Suite | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored) |
| Total Rust Test Suite | `... cargo test --manifest-path src-tauri/Cargo.toml` | 186 passed, 3 ignored |
| Desktop Binary Verification | `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (16.92 MiB) |
| Windows Metadata Audit | `npm run verify:metadata` | `WINDOWS_METADATA_OK` (5/5 pass) |
| Signature Integrity & Negative Tests | `npm run verify:signatures` | `SIGNATURES_VERIFIED_OK` (3/3 pass, 3/3 negative pass) |
| Documentation Integrity | `npm run docs:check` | `DOCS_CHECK: PASSED` |

## 3. Organizational Code Signing Policy

1. **Chryypt Organization Code Signing:** Per operator directive, code signing is officially conducted under the Chryypt Organization (`CN=Chryypt Organization`, publisher `Chryypt`).
2. **External CA Dependency Removed:** Third-party external CA dependencies and workflows have been removed, retaining full organizational control over build distribution and signing.

## 4. Next Engineering Tasks

1. Maintain documentation synchronization via automated checks (`scripts/verify-docs.mjs` / `npm run docs:check`).
2. Await operator evaluation of the published v0.1.0 installer bundles on Windows 10/11 VMs.
