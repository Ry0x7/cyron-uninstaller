# AGENTS.md — cyron-uninstaller

## What this is

A Revo uninstaller copy that can uninstall AI and clean up coding dev caches or AI. Have a feature
to clean up bloat or unnecessary files similar to bleachbit but with proper indications on what is
safe to remove but also have the ability to force uninstall and kill process. In all sense it's a
Revo uninstaller, bleachbit, and windirstat but better indications, material UI, clean simple
usable for non-tech people.

**Current state: Phases 1, 2 and 3 are implemented.** Application inventory, disk analysis with a
treemap plus persistent synchronized folder tree, MFT/directory scanning, duplicate verification,
storage categorization, developer/AI cache inspection and **review-first cache cleaning with a
recoverable quarantine** work. Scanning remains read-only; the only destructive path is the
cleanup engine in `cleanup.rs`, which accepts catalog ids (never paths), moves files into
`<app_data>\quarantine\<run-id>\`, and can restore or purge a batch. There is still no uninstall,
process termination or registry write. `clear_history` deletes only Cyron's own history file. The
debug desktop build and launch are verified on this machine (`DESKTOP_BINARY_OK`,
`DESKTOP_SMOKE_OK`), as are the isolated cleanup fixture (`CLEANUP_FIXTURE_OK`) and every suite;
release installers, signing and a standalone release build are not. The Cleaner was driven live in
both themes, which found and fixed two defects: preview was blocked by read-only mode, and
Overview's cache warnings rendered as one run-on sentence.

## Stack

- Tauri v2 + Rust (`windows-sys`, `winreg`, `tauri-plugin-dialog`).
- React 19 + TypeScript + Vite 7. No router, no chart library. Vitest for frontend tests.
- Windows 10/11. The Rust backend calls Win32 APIs unconditionally; there is no cross-platform
  path.
- Design tokens and shared primitives are hand-built (`src/styles`, `src/components`).

## How it works (the non-obvious parts)

- **IPC surface** is nineteen commands in `src-tauri/src/commands.rs`, registered in `lib.rs`:
  discovery/scan/duplicate/measurement/reveal, `clear_history`, and eight cleanup commands
  (`get_cleanup_mode`, `set_cleanup_mode`, `preview_cleanup`, `start_cleanup`, `list_quarantine`,
  `restore_quarantine`, `purge_quarantine`, `get_cleanup_log`). A running job id is rejected rather
  than silently re-registered. Payloads use `#[serde(rename_all = "camelCase")]`;
  `src/lib/types.ts` mirrors the data contract. Change both together.
- **Cleanup engine** (`cleanup.rs`) is the only module that modifies user files. `preview_cleanup`
  resolves catalog ids, guards each path, walks it for exact totals (400 listed paths, 100,000
  files per target) and stores a `CleanupPlan` server-side for 10 minutes. `start_cleanup` consumes
  that plan once, re-checks its selection, the TTL and the read-only switch, re-guards every target,
  re-validates each file (regular file, no reparse attribute, canonical parent inside the target)
  and renames it into the run's quarantine batch. Nothing is deleted except by `purge_quarantine`,
  which only removes manifest items still inside that batch. Restore never overwrites. The backend
  starts read-only; Settings exclusions are validated and enforced in the backend.
- **Cleanup catalog** is the 48-entry `caches::CATALOG`: 27 safe, 9 caution, 12 protected. Protected
  entries (histories, models, settings, Docker/WSL data, Firefox profiles, Windows Update
  downloads) are measured and never planned; the Windows Update cache is explicitly not
  manipulated. Per-entry `impact` text explains whether data regenerates or is re-downloaded.
- **Application discovery** (`apps.rs`) reads HKLM/HKCU uninstall keys in the 64-bit view plus
  `WOW6432Node`, drops updates/patches/system components, deduplicates by name+version+location
  (preferring machine-64), and reports `estimatedSize: null` when the installer recorded nothing.
  Windows Store/MSIX apps are not covered yet.
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
