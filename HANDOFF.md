# Cyron — Active Handoff

Updated: 2026-11-06 (Phase 6 Competitive Expansion & RC1 Hardening complete)

## Current Status

Phases 1–6, Priority 0 security hardening, and Release Candidate 1 (RC1) hardening are implemented and verified across the Rust backend
and React desktop interface. Cyron provides a complete, review-first alternative to Revo Uninstaller, WinDirStat/WizTree, BleachBit, and BCUninstaller.

### Competitive Capabilities Delivered:
1. **Windows Explorer Context Menu Integration**:
   - Optional native shell integration registered exclusively in `HKEY_CURRENT_USER\Software\Classes` without requiring administrator privileges.
   - For Folders & Drives: *Analyze with Cyron*, *Find Duplicates with Cyron*, *View Storage Breakdown with Cyron*.
   - For Executables: *Uninstall with Cyron*, *Find Related Files with Cyron*.
   - Safe CLI routing (`--analyze`, `--duplicates`, `--storage`, `--uninstall-target`, `--find-related`) validates paths and never triggers destructive operations directly from Explorer.
   - Settings management panel provides 1-click status checking, enable, disable (with clean unregistration), and repair.
2. **Enhanced Multi-Source Application Discovery**:
   - Win32 (64-bit and 32-bit views, machine and user scopes) with broken uninstaller detection when executables no longer exist on disk.
   - Microsoft Store / MSIX / AppX read-only inventory discovered via `AppModel\Repository\Packages` with Windows Package Manager guidance.
   - Steam installed-game detection parsing `libraryfolders.vdf` and `appmanifest_*.acf` across all library drives.
   - Bounded heuristic detection of portable executables in user program folders (`%LOCALAPPDATA%\Programs`).
   - Uninstaller UI filters (`All`, `Win32`, `Store`, `Steam`, `Portable`, `Broken`) with source badges and explainable lifecycle guidance.
3. **Disk Analyzer → Cleaner Direct Integration**:
   - Selecting recognized cache directories in the treemap or folder tree surfaces a **Review for Cleanup** action.
   - Displays expected reclaimable bytes alongside total occupied bytes.
   - Reuses existing review-first confirmation plans, quarantine, and atomic restoration workflows.
   - Protected directories (such as chat histories, configurations, and Windows Update downloads) clearly explain why Cyron prevents automated deletion.
4. **Extensible Declarative Cleanup Providers**:
   - Versioned (`schema_version: 1`) provider definitions with strict path validation, safe environment root enforcement, and automated rejection of malicious path definitions.
   - Added verified developer and AI tool providers (Ollama models/cache, Hugging Face Hub, Cursor workspace storage, pip, Poetry, uv, Rustup, Gradle, vcpkg).
5. **Competitive Performance Benchmarking**:
   - Created `scripts/benchmark-competitive.mjs`, measuring cold vs warm scan latency, peak working set, file counting rules, and cancellation response against WinDirStat/WizTree metrics.
   - Documented in `docs/BENCHMARKS.md`.
6. **UI/UX Audit & Accessibility Polish**:
   - High-contrast accessible design tokens, responsive drawers, ARIA keyboard tree navigation, loading states, and clear explanations for nontechnical users.

---

## Verification Table

| Check | Command | Result |
|---|---|---|
| TypeScript Typecheck | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend Unit Tests | `npm test` | 71/71 pass across 7 files |
| Frontend Production Build | `node scripts/verify-dist.mjs` | `DIST_OK` (2 assets, 458.8 KiB) |
| Rust Unit & Lib Tests | `node scripts/msvc-env.mjs cargo test --lib` | 134/134 pass, 2 ignored |
| Security Boundaries | `... --test security -- --nocapture` | 25/25 pass |
| Cleanup Stress Suite | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup Fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| End-to-End Fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK` |
| Real System Scanner | `... --test real_system -- --nocapture` | Pass |
| Scanner Stress Suite | `... --test stress_scan -- --nocapture` | 5/5 pass, 1 ignored |
| Total Rust Test Suite | `... cargo test --manifest-path src-tauri/Cargo.toml` | **171 tests pass**, 3 ignored |
| Desktop Binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (15.89 MiB) |
| Launch Smoke Test | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron") |
| Competitive Benchmarks | `node scripts/benchmark-competitive.mjs` | `BENCHMARK_OK` |
| Release Packaging | `node scripts/verify-release-bundles.mjs` | `RELEASE_BUNDLES_OK` (NSIS: 1.42 MiB, MSI: 2.00 MiB) |
| Remote Windows CI | `.github/workflows/ci.yml` | Pass (Run #42188091108) |

---

## Key Files & Modules

- `src-tauri/src/explorer.rs`: native Windows context menu registration under `HKCU\Software\Classes`, startup routing parser, path validation.
- `src-tauri/src/providers.rs`: declarative cleanup provider system, schema validation, safe environment root checks, malicious definition tests.
- `src-tauri/src/apps.rs`: multi-source application discovery (Win32, Store packages, Steam libraries, and portable heuristics).
- `src-tauri/src/commands.rs`: 34 validated IPC commands.
- `src-tauri/src/process.rs`: process discovery, snapshot tracking, PID creation time checks (`GetProcessTimes`), critical process deny list.
- `src-tauri/src/uninstall.rs`: server-side command resolution, `UninstallPlan` & `BatchUninstallPlan` creation, safe uninstaller tokenization.
- `src-tauri/src/leftovers.rs`: server-side `LeftoverPlan`, bounded non-following directory walker, recursive multi-type `.reg` backup and restore.
- `src-tauri/src/cleanup.rs`: cleanup engine, durable write-ahead recovery journal (`sync_all`), quarantine, atomic no-overwrite restore.
- `src/features/analyzer/Analyzer.tsx`: treemap, folder tree, filter bar, direct Review for Cleanup integration.
- `src/features/uninstaller/Uninstaller.tsx`, `AppDrawer.tsx`: uninstaller UI, source filters, batch queue, process manager, residual reviewer.
- `src/features/settings/Settings.tsx`: Explorer context menu management, safety controls, theme switcher.
- `scripts/benchmark-competitive.mjs`: comparative benchmark runner.
- `docs/USER_GUIDE.md`, `docs/BENCHMARKS.md`, `docs/SECURITY.md`, `docs/ARCHITECTURE.md`: synchronized technical documentation.

---

## Known Considerations

- **Store / MSIX Lifecycle**: Store applications are managed by Windows Package Manager; Cyron displays clear guidance to use Windows Settings or `winget`, while providing read-only inventory, file walk, and leftover detection.
- **Authenticode Signing**: Binaries are unsigned release candidates. Production signing is automated in GitHub Actions once certificate secrets (`CSC_LINK` / `CSC_KEY_PASSWORD`) are supplied.
- **Git History Rewrite**: Purging historical internal session data (`62414cc`) via `git-filter-repo` is documented and awaits explicit operator authorization.
