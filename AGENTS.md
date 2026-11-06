# AGENTS.md — cyron-uninstaller

## What this is

A Revo uninstaller copy that can uninstall AI and clean up coding dev caches or AI. Have a feature
to clean up bloat or unnecessary files similar to bleachbit but with proper indications on what is
safe to remove but also have the ability to force uninstall and kill process. In all sense it's a
Revo uninstaller, bleachbit, and windirstat but better indications, material UI, clean simple
usable for non-tech people.

**Current state: Phases 1, 2, 3, 4, 5, 6, and 7 are implemented and verified.** Application inventory across Win32, Microsoft Store,
Steam games, and portable apps; disk analysis with a treemap plus persistent synchronized folder tree, MFT/directory scanning,
duplicate verification, storage categorization, direct Analyzer → Cleaner review integration, extensible declarative cleanup providers,
review-first cache cleaning with a crash-consistent recoverable quarantine, application uninstallation with exit code 3010 reboot recognition,
process discovery and snapshot-verified termination with critical-process protection and PID recycling denial, leftover scanning
with confidence & risk ratings, automatic recursive multi-type .reg backup before subkey deletion, sequential batch uninstallation,
optional Windows Explorer context menu integration, standardized Windows PE/MSI metadata verification (`WINDOWS_METADATA_OK`),
local self-signed code signing (`DEV_TEST_SIGNED`, `SIGNATURES_VERIFIED_OK`), SignPath Foundation release automation preparation,
and release installer bundling (NSIS and WiX MSI) are implemented and tested.
Priority 0 hardening (crash-consistent write-ahead recovery journal with sync_all, atomic MoveFileExW no-overwrite
restoration, approved snapshot binding, purge preservation, quarantine storage caps) is complete.
The desktop build and launch are verified (`DESKTOP_BINARY_OK`, `DESKTOP_SMOKE_OK`), as are the
isolated cleanup fixture (`CLEANUP_FIXTURE_OK`), the end-to-end fixture (`RC1_E2E_FIXTURE_OK`), all test suites (71 frontend, 171 Rust), and release installer
bundles (`RELEASE_BUNDLES_OK`).

## Stack

- Tauri v2 + Rust (`windows-sys`, `winreg`, `tauri-plugin-dialog`).
- React 19 + TypeScript + Vite 7. No router, no chart library. Vitest for frontend tests.
- Windows 10/11. The Rust backend calls Win32 APIs unconditionally; there is no cross-platform
  path.
- Design tokens and shared primitives are hand-built (`src/styles`, `src/components`).

## How it works (the non-obvious parts)

- **IPC surface** is thirty-four commands in `src-tauri/src/commands.rs`, registered in `lib.rs`:
  discovery/scan/duplicate/measurement/reveal, `clear_history`, eight cleanup commands
  (`get_cleanup_mode`, `set_cleanup_mode`, `preview_cleanup`, `start_cleanup`, `list_quarantine`,
  `restore_quarantine`, `purge_quarantine`, `get_cleanup_log`), nine Phase 4/5 uninstaller commands
  (`prepare_uninstall`, `prepare_batch_uninstall`, `discover_app_processes`, `terminate_app_processes`,
  `parse_uninstall_command`, `execute_uninstall`, `scan_app_leftovers`, `clean_app_leftovers`, `execute_batch_uninstall`),
  and six Phase 6 expansion commands (`get_startup_route`, `get_explorer_integration_status`, `set_explorer_integration`,
  `repair_explorer_integration`, `list_cleanup_providers`, `inspect_cleanable_directory`).
  A running job id is rejected rather than silently re-registered. Payloads use `#[serde(rename_all = "camelCase")]`;
  `src/lib/types.ts` mirrors the data contract. Change both together.
- **Explorer Integration** (`explorer.rs`) registers per-user shell verbs in `HKCU\Software\Classes` without requiring elevation.
  Supports Folders & Drives (Analyze, Duplicates, Storage) and Executables (Uninstall Review, Related Files). Safe startup argument
  parsing routes the application to the requested view without ever executing destructive actions directly from Explorer.
- **Extensible Providers** (`providers.rs`) validates versioned provider definitions (`schema_version: 1`), enforcing approved
  environment roots, rejecting path traversals and wildcards, and preventing sensitive data from being marked safe.
- **Application discovery** (`apps.rs`) reads HKLM/HKCU uninstall keys in the 64-bit and 32-bit views, discovers modern Store
  packages (`AppModel\Repository\Packages`), parses Steam game libraries (`libraryfolders.vdf`), and heuristically identifies
  standalone portable binaries in `%LOCALAPPDATA%\Programs`. Provides explicit source tags, uninstall support classifications, and
  lifecycle explanations.
- **Directory enumeration** (`fs.rs`) uses `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`
  for logical size, allocated size and a stable file id, with a `std::fs` fallback. Reparse points
  (junctions/symlinks/mount points) are never followed.
- **Scan method selection** (`scan.rs`): `auto` uses the MFT reader only for a drive root and the
  directory walk elsewhere; MFT is whole-volume work and loses badly on subfolders (System32:
  3.9 s vs 0.28 s). A requested method that falls back reports `methodNote`. Both paths return the
  same `ScanResult` shape with `method`, `methodNote`, `outcome` and `storage`.
- **MFT reader** (`mft.rs`) parses the boot sector, `$MFT` run list, fixups and
  `FILE_NAME`/`$DATA`/`$STANDARD_INFORMATION`. It rejects a record count above its derived 768 MiB
  budget (4,910,404 records) before allocating, uses one fixed 1,024-byte buffer, polls
  cancellation every 4,096 records and dedupes hard links with a per-record bitset. It needs an
  elevated process; otherwise it returns a fallback reason.
- **Directory walk** (`scan.rs`) is depth-first, single-threaded and streaming: directory entries
  are processed as read, not collected. It tracks at most 1,000,000 hard-link ids, retains at most
  40 file nodes per folder, 200 children per folder, 100,000 tree nodes and 4,096 extension keys,
  and reports top-24 extensions and top-100 largest files. Cancellation is an `AtomicBool` polled
  per directory and entry; progress is throttled to 120 ms.
- **Duplicate verification** (`duplicates.rs`) groups by size, hashes 4 KiB edges, then full
  streaming SHA-256. Hard links are listed but do not count toward recoverable bytes. Caps: 1M
  candidates, 200 groups, 50 files per group, with `truncated`.
- **Storage intelligence** (`storage.rs`) assigns every byte to one of ten categories with
  `known` vs `inferred` confidence, five age buckets and explained insights. Category text comes
  from `category_meta`; it describes data, never removal advice.
- **Cache catalog** (`caches.rs`) is 48 fixed env-templated locations. Risk is `safe` (regenerates),
  `caution` (re-download/reset cost) or `protected` (history, models, settings, Docker/WSL data).
  Protected entries are measured but never offered for removal. Each target is measured with
  `scan::measure_path`.
- **History** (`history.rs`) is `<Tauri app_data_dir>/history.json`, newest first, capped at 100,
  with temp-file replacement. Only disk scans are recorded; cache discovery is not. Clearing the
  Activity list deletes this app-owned file only.
- **Frontend shell** (`App.tsx`) keeps visited views mounted but hidden so scan state survives
  navigation. Settings are persisted in `localStorage["cyron.settings.v1"]` and resolve
  system/light/dark into `data-theme` on `<html>` before React mounts (no theme flash).
- **Analyzer tree** (`features/analyzer/TreeView.tsx`) is a persistent, resizable column beside
  the treemap (220–520 px, persisted in `localStorage["cyron.analyzer.treeWidth"]`), virtualized
  with no row cap, ARIA tree keyboard navigation and per-root reset. It is the summarized IPC tree,
  not a full file listing.
- **Analyzer modes and filters**: Map / Duplicates / Storage share the scan result;
  `lib/analyzerFilters.ts` filters the tree, largest files and treemap and computes filtered
  totals. The Duplicates panel cancels and resets when the scan scope changes.
- **Cache request dedupe**: `lib/api.ts` memoizes `list_caches` so Overview and Cleaner share one
  disk measurement; `invalidateCaches()` forces a fresh pass.

## Conventions

- Scanning, duplicates and storage stay read-only. The only file-modifying path is the cleanup
  engine; do not add a second one or a command that accepts a free-form path. Uninstall, process
  termination and registry writes belong to Phase 4 and must reuse the plan → confirmation →
  quarantine/recovery pattern, with their own security tests.
- Never widen cleanup eligibility outside `caches::CATALOG` without a reviewed catalog entry, and
  never let the frontend supply a path to clean. Protected entries and ancestors of protected data
  must be refused in the backend.
- Files are moved into quarantine, never deleted in place. `purge_quarantine` is the only permanent
  removal, and it must stay manifest-scoped.
- Match the existing UI language: paper/graphite surfaces, mono micro-labels, square risk badges,
  no gradients or decorative card slop. Reuse `components/ui.tsx` and tokens.
- View-specific CSS uses a scoped prefix (`ov-`, `un-`, `az-`, `treemap-`, `cl-`, `act-`, `set-`).
  Shared layout belongs in `src/styles/app.css`; don't fork primitives per view.
- Every new non-trivial rule needs a test near the code: Rust tests in the module, frontend tests
  in `src/lib/*.test.ts`.
- Validate paths and job ids at the command boundary; never interpolate user input into a shell.
- **Git workflow rule: Rebasing is strictly prohibited.** All branch integration must use standard merge commits (`git merge --no-ff`) or squash merges. Never run `git rebase` or force-push; this preserves immutable commit provenance, cryptographic hashes, and release audit trails. Local clones enforce this via the `pre-rebase` hook (`scripts/setup-git-hooks.ps1`).
- Keep docs honest: do not claim benchmarks, screenshots, signed builds or verification that did
  not run. Record limitations.

## Commands

```powershell
npm install
npm run dev:desktop        # Tauri dev window
npm run build:desktop      # debug exe, no installer
npm run typecheck
npm test
npm run build
npm run tauri -- build     # release MSI/NSIS (needs bundler download; unverified)
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test real_system -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test stress_scan -- --nocapture
node scripts/verify-cleanup.mjs
node scripts/verify-e2e.mjs
node scripts/verify-dist.mjs
node scripts/verify-desktop-binary.mjs
node scripts/smoke-desktop.mjs
```

## Gotchas

- **MSVC toolchain is mandatory.** Git for Windows ships a coreutils `link.exe` that shadows the
  real linker. Use `scripts/msvc-env.mjs` (it finds VS through `vswhere`) for any cargo/Tauri
  command on a machine without a Developer prompt.
- **Do not trust `estimatedSize`.** Many installers omit it. Show “—” and use *Measure folder* for
  the real size.
- **The treemap is a pruned view**, not a complete file list. Aggregated `group` nodes have an
  empty path and cannot be revealed in Explorer.
- Scanning a drive root without exclusions can take a long time and hit permission errors; those
  are reported (max 20). Output structures are capped, and the MFT reader checks its record budget
  before allocating. Benchmark MFT only from an elevated shell; the harness refuses to label a
  fallback run as MFT.
- **MFT and walk totals differ** because one counts records and the other counts paths. Do not
  publish either as "the" size of a volume without the counting rule.
- **No network code exists.** Keep it that way unless a feature explicitly requires it, and never
  add telemetry by default.
- **Store apps are missing** from the uninstaller. This is known, not a regression.
- Quarantine and `cleanup-log.json` are per Windows profile. A sole restored batch retires itself,
  so purging it afterwards is refused with "No quarantine batch named …" — an emptied batch is not
  an error state. Tests and docs must keep that contract.
- Cleanup test fixtures live under `%TEMP%\cyron-cleanup-<pid>` and are deleted on drop. Never point
  a cleanup test, benchmark or manual check at a real cache or the user profile.

## Docs map

- `README.md`: product, setup, verification and limitations.
- `ROADMAP.md`: phase plan with honest checkboxes.
- `HANDOFF.md`: current save state, verification results and next directive.
- `docs/ARCHITECTURE.md`: components, IPC, scan pipeline, safety model.
- `docs/IMPLEMENTATION_STATUS.md`: Phase 1 and Phase 2 acceptance tables and evidence.
- `docs/BENCHMARKS.md`: measured scanner results, environment and caveats.
- `DESIGN.md`: tokens, themes, component anatomy, treemap palette and accessibility rules.
- `docs/SECURITY.md`: threat model and future destructive-operation gates.
- `docs/BUNDLE.md`: build/installer targets, WebView2, signing status and app-data paths.
- `GATES.md`: Phase 2 acceptance ledger (tracked).
