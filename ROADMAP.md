# Cyron — Roadmap

Cyron is a Revo Uninstaller, BleachBit and WinDirStat replacement for Windows: an application
uninstaller, disk-space analyzer and system/developer/AI cleaner with plain-language risk
explanations, built for people who are not sysadmins.

Status legend: `[x]` implemented and verified by code/tests, `[~]` implemented but final
native-build or visual verification is pending, `[ ]` not implemented. Checkboxes describe work
state, not a claim that an unobserved remote CI run passed.

## Phase 1 — Foundation and read-only scanning

- [x] Scaffold Tauri v2 + Rust + React + TypeScript with a modular backend.
- [x] Design system, light/dark themes, collapsible sidebar and six-section navigation.
- [x] Discover installed applications from the Windows uninstall registry (machine/user, 64/32-bit).
- [x] Show publisher, version, install date, architecture, estimated size and install location.
- [x] Read-only application detail drawer with real folder measurement and Explorer reveal.
- [x] Native directory enumeration with logical and allocated size (fallback reports logical size as allocated and has no file ids).
- [x] Depth/output-capped, cancellable filesystem scan with progress, exclusions and junction
      skipping (output and work are bounded; see Phase 2 for the streaming rewrite).
- [x] Interactive squarified treemap with drill-down and synchronized selection details.
- [x] Synchronized folder tree: expandable real scan nodes, treemap↔tree selection,
      scroll-to-selection and keyboard tree navigation (Phase 2 moved it to a persistent,
      virtualized column).
- [x] Largest-file and extension-statistics reporting from real scan data.
- [x] Developer/AI/Windows cache catalog (48 locations) with Safe / Review / Protected classification.
- [x] Overview with real volume capacity, application counts, measured cache sizes by risk and recent scan activity.
- [x] Local scan history persisted in the app-data directory (max 100 entries).
- [x] Settings for theme, hidden/system files and exclusion paths (scanner skips hidden/system entries when the option is off).
- [x] Frontend and Rust unit/integration suites. Latest measured results: 51 frontend tests pass;
      84 Rust tests pass (73 unit, 5 read-only, 1 real-system, 5 stress) with 3 ignored.
- [x] Native desktop compile and launch: `npm run build:desktop`, `verify-desktop-binary.mjs`
      (`DESKTOP_BINARY_OK`) and `smoke-desktop.mjs` (`DESKTOP_SMOKE_OK`, window title "Cyron").
- [x] Visual review of the running native window in both themes (six views, light and dark,
      with a real scan loaded).
- [x] Windows CI workflow for typecheck, tests, build and `cargo test` (workflow file added;
      remote run has not been observed).
- [x] Documentation: README, architecture, implementation status, agent memory, handoff.
- [x] No user-data cleanup, uninstall, process termination or registry-write operations. The
      `clear_history` action removes only Cyron's own history file.

Deferred from Phase 1 and delivered in Phase 2: a persistent tree column, duplicate files and
benchmarks. Still deferred: MSIX/Store inventory.

## Phase 2 — Disk intelligence (implemented, see docs/IMPLEMENTATION_STATUS.md)

- [x] MFT-based NTFS scanning with a safe directory-walk fallback, automatic selection at drive
      roots and an explicit `methodNote` whenever a requested method falls back.
- [x] Benchmark harness (`scripts/bench-scan.mjs`, `examples/scan_bench.rs`) reporting duration,
      peak working set, files, directories and logical bytes. Warm-cache measurements recorded in
      [docs/BENCHMARKS.md](docs/BENCHMARKS.md); cold-cache timing is not measured.
- [x] A persistent, resizable, virtualized tree column beside the treemap, synchronized with
      treemap selection, breadcrumbs and details.
- [x] Advanced filters (name, size, age, extension, type, sort field/direction) applied to the
      tree, largest files and treemap, with filtered totals.
- [x] Duplicate-file candidates using size buckets, 4 KiB edge hashing and full streaming
      SHA-256; cancellation, progress, hard-link handling and read-only reporting.
- [x] Storage categorization: ten categories with known/inferred confidence, age buckets and
      explainable insights; Docker, WSL, package caches and AI models are identified and described
      as data, not as removal targets.
- [x] Inaccessible-folder, reparse-point and partial-result diagnostics in the analyzer, including
      scanner method and fallback reason.
- [x] Scan performance and memory passes on very large roots; the walker is streaming, hard-link
      tracking is bounded, and the MFT reader enforces a record-count budget before allocating.
- [x] Cancellation and error handling stress-hardening under very large scans
      (`tests/stress_scan.rs`: wide directories, deep nesting, access-denied paths, exclusions,
      cancellation).

Still deferred: MFT measurement on volumes above the record budget, Store/MSIX inventory, and a
cold-cache benchmark methodology.


## Phase 3 — Safe cleaning

- [ ] Cleanup previews for every operation, grouped by provider and risk.
- [ ] Windows cleanup provider (temporary files, update cache, crash dumps, browser caches).
- [ ] Application cache providers for supported tools.
- [ ] Developer/AI cleanup providers for the Phase 1 catalog, including re-download cost warnings.
- [ ] Explicit confirmation, exclusion lists and protected-data enforcement.
- [ ] Registry backup before any registry modification (if introduced).
- [ ] Rollback for supported file operations and a detailed operation log.
- [ ] Dry-run mode and a read-only switch.

## Phase 4 — Uninstall engine

- [ ] Standard uninstall using registered uninstall commands.
- [ ] Process discovery and controlled termination for programs holding locks, with critical-process
      protection.
- [ ] Leftover file, directory and registry detection with preview.
- [ ] Shared-dependency and system-component protection.
- [ ] Advanced and force uninstall with staged safeguards.
- [ ] Batch uninstall queue with progress and failure handling.
- [ ] Uninstall history and recovery guidance.

## Phase 5 — Optimization and release

- [ ] Performance profiling of scanning, rendering and startup.
- [ ] UI/UX refinement with accessibility and keyboard review.
- [ ] Security review of every future destructive path.
- [ ] Signed Windows binaries when signing infrastructure is available.
- [ ] GitHub release automation with MSI/NSIS installers.
- [ ] Crash reporting that stays opt-in and local-first (no telemetry by default).
- [ ] Public documentation, screenshots and a contribution guide.

## Non-negotiable constraints for every phase

- Never recursively delete arbitrary directories based on names.
- Never offer conversation histories, model weights, credentials, Docker volumes, WSL distros or
  repositories for bulk removal.
- Preview every destructive operation and require explicit confirmation.
- Cap scan output and hard-link tracking, and never follow junction/symlink cycles. Do not claim
  strict memory bounds unless the per-directory entry vectors and extension map are bounded too.
- Deduplicate hard links by file id for allocated space where the native enumerator provides ids;
  disclose fallback and file-id-cap limitations rather than promise perfect deduplication.
- Handle permission failures without crashing.
- Keep the UI responsive during large scans.
