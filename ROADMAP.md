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


## Phase 3 — Safe cleaning (implemented, see docs/IMPLEMENTATION_STATUS.md)

- [x] Cleanup previews for every operation, grouped as Windows / Applications / Developer tools /
      AI tools and by risk, listing the exact planned file paths with totals.
- [x] Cleanup engine (`src-tauri/src/cleanup.rs`): catalog-resolved targets only, backend guard,
      per-file revalidation, quarantine instead of deletion, per-item results, progress and
      cancellation. Plan ids are single-use, held server-side, and expire after 10 minutes.
- [x] Windows cleanup provider: user temporary files, Windows temporary files, crash dumps and
      the INetCache folder. The Windows Update download cache is measured but protected — Cyron
      does not manipulate servicing state; Storage Sense/Disk Cleanup remain the supported route.
- [x] Browser cache providers: Chrome and Edge caches are cleanable; Firefox profiles are
      protected. Browser profiles, history, cookies and passwords are never offered.
- [x] Application cache providers: VS Code, Cursor and Antigravity caches keep only cache, GPU
      cache and log entries cleanable; workspace storage and local history stay protected.
- [x] Developer/AI providers for the catalog, with an explicit impact note per entry describing
      whether the data regenerates, is re-downloaded, or costs a reset.
- [x] Explicit confirmation (checkbox + backend plan), backend-enforced user exclusions from
      Settings, and protected-data enforcement in the backend rather than the UI.
- [x] Recoverable quarantine (`<app_data>\quarantine\<run-id>`) with a manifest, byte-exact
      restore that never overwrites, purge of a single batch, and 7-day retention metadata.
- [x] Cleanup operation log (`<app_data>\cleanup-log.json`, max 100) plus Activity entries; restore
      and purge are logged too. No registry writes exist, so no registry backup is required yet.
- [x] Dry-run preview (available in read-only mode, since it only reads) and a backend read-only
      switch that is on by default and blocks execution.
- [x] Permission-boundary security suite (`tests/security.rs`) replacing the blanket read-only
      source audit, cleanup stress suite, an isolated end-to-end cleanup fixture, and updated CI.

Deferred to Phase 4: registry cleaning and therefore registry backup, process termination and
locked-file forcing, uninstall commands, and duplicate management. Quarantine has no automatic
size cap or expiry sweep; batches stay until purged.

## Phase 4 — Uninstall engine (implemented)

- [x] Standard uninstall using registered uninstall commands with safe tokenization, MSI uninstaller
      support (`msiexec.exe /x`), shell-wrapper rejection (`cmd.exe`, `powershell.exe`), and exit code /
      reboot-required tracking (code 3010).
- [x] Process discovery and controlled termination for programs executing from the application directory,
      with critical-process protection (system processes, `explorer.exe`, Windows root binaries) and
      graceful `WM_CLOSE` preceding `TerminateProcess`.
- [x] Leftover file, directory and registry detection with reviewable confidence and risk ratings.
- [x] Shared-dependency, system-component and common vendor protection to prevent orphaned shared runtimes.
- [x] Advanced and force uninstall with staged safeguards, durable write-ahead recovery journal, and
      automated registry `.reg` backup before any registry subkey deletion.
- [x] Batch uninstall queue with progress, sequential execution, failure isolation, and separate manual
      review required before leftover cleaning.
- [x] Priority 0 cleanup recovery hardening: crash-consistent write-ahead recovery journal
      (`recovery_journal.jsonl`), atomic Win32 `MoveFileExW` no-overwrite restore, approved preview snapshot
      binding, safe purge retry with manifest preservation on locked files, and quarantine storage & low-disk-space caps.

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
