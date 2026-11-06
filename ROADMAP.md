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
      skipping (strict memory bounds for arbitrary inputs are not guaranteed).
- [x] Interactive squarified treemap with drill-down and synchronized selection details.
- [x] Synchronized folder tree in the analyzer details rail: expandable real scan nodes,
      treemap↔tree selection, scroll-to-selection and keyboard tree navigation (500-row render cap).
- [x] Largest-file and extension-statistics reporting from real scan data.
- [x] Developer/AI/Windows cache catalog (48 locations) with Safe / Review / Protected classification.
- [x] Overview with real volume capacity, application counts, measured cache sizes by risk and recent scan activity.
- [x] Local scan history persisted in the app-data directory (max 100 entries).
- [x] Settings for theme, hidden/system files and exclusion paths (scanner skips hidden/system entries when the option is off).
- [x] Frontend and Rust unit/integration suites. Latest measured results: 38 frontend tests pass;
      38 Rust tests pass with 1 ignored; the real-system integration test passes; the 5-test
      read-only safety suite passes.
- [x] Native desktop compile and launch: `npm run build:desktop`, `verify-desktop-binary.mjs`
      (`DESKTOP_BINARY_OK`) and `smoke-desktop.mjs` (`DESKTOP_SMOKE_OK`, window title "Cyron").
- [~] Visual review of the running native window in both themes is still pending.
- [x] Windows CI workflow for typecheck, tests, build and `cargo test` (workflow file added;
      remote run has not been observed).
- [x] Documentation: README, architecture, implementation status, agent memory, handoff.
- [x] No user-data cleanup, uninstall, process termination or registry-write operations. The
      `clear_history` action removes only Cyron's own history file.

Deferred from Phase 1: a persistent tree column beside the treemap (the tree lives in the
analyzer rail), MSIX/Store inventory, duplicate files and benchmarks.

## Phase 2 — Disk intelligence

- [ ] MFT-based NTFS scanning with a safe directory-walk fallback.
- [ ] Benchmark harness: cold/warm scans of representative roots, memory ceiling and duration.
- [ ] A persistent tree column beside the treemap (Phase 1 ships a synchronized Tree tab in the details rail).
- [ ] Advanced filters (size, age, extension, modification date); largest-file discovery exists in the current scanner.
- [ ] Duplicate-file candidates using size buckets, staged partial hashes and full hashes.
- [ ] Storage categorization: applications, games, downloads, documents, temporary files,
      developer tools and AI models.
- [ ] Present richer inaccessible-folder and reparse-point diagnostics in the UI (the scanner already skips reparse points and reports capped errors).
- [ ] Scan performance and memory passes on very large roots.
- [ ] Cancellation and error handling stress-hardening under very large scans.

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
