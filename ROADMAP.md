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

## Phase 4.1 — Priority 0 security hardening (implemented)

- [x] Uninstaller execution boundary: frontend parameters are untrusted; uninstaller commands are resolved
      server-side from registered keys with single-use expiring plans (`UninstallPlan`, 10m TTL), unquoted path
      parsing, MSI argument normalization, and live-registry revalidation.
- [x] Process termination verification: executable path ownership, 64-bit creation time (`GetProcessTimes`), and
      PID identity are verified against a recent snapshot before controlled termination; critical system processes
      are denied unconditionally.
- [x] Leftover scanner server-side plans: residual candidates are generated server-side with single-use plans
      (`LeftoverPlan`), bounded non-following traversal skipping junctions and symlinks, and durable write-ahead
      recovery journals with physical synchronization (`sync_all`).
- [x] Registry safety & multi-type backup: complete recursive `.reg` export supporting `REG_SZ`, `REG_DWORD`,
      `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, and `REG_MULTI_SZ` with backup integrity verification before
      deletion, verified restore helper, and shared vendor key denial.
- [x] Recovery correctness: physical disk synchronization (`flush()` and `sync_all()`) on recovery journals,
      destination ancestor reparse checks, and honest same-volume vs cross-volume restoration semantics.

## Phase 5 — Optimization and release (implemented)

- [x] Performance profiling of scanning, rendering and startup (`scripts/bench-scan.mjs`, sub-2s startup in smoke tests).
- [x] UI/UX refinement with accessibility, responsive drawer tabs, and keyboard navigation review across all six views.
- [x] Security review of all destructive paths with 22 automated boundary audit tests in `tests/security.rs`.
- [x] MSI and NSIS installer bundle generation verified locally (`makensis` and WiX `candle`/`light`, `RELEASE_BUNDLES_OK`).
- [x] Application data preservation across in-place upgrades (`%APPDATA%\dev.cyron.uninstaller`).
- [x] Signed Windows binaries: two-track signing architecture implemented with local `DEV_TEST_SIGNED` self-signed pipeline and prepared `SIGNPATH_SIGNED` SignPath Foundation workflow.
- [x] GitHub release automation with reproducible SHA-256 checksums (`.github/workflows/release.yml`, `scripts/verify-release-bundles.mjs`).
- [x] Privacy audit: historical internal session data purged from repository history via `git-filter-repo`.
- [x] Store/MSIX inventory investigation and compatibility boundary documented (`docs/ARCHITECTURE.md`).
- [x] User documentation, contribution guide, and release checklist (`docs/USER_GUIDE.md`, `CONTRIBUTING.md`, `docs/RELEASE_CHECKLIST.md`).

## Release Candidate 1 (RC1) — Release Hardening (implemented)

- [x] Batch uninstall confirmation plan: server-held, single-use, 10-minute expiring confirmation plan (`BatchUninstallPlan`), live registry command revalidation, and explicit operator approval of executables and arguments before execution.
- [x] Fail-closed leftover file cleanup: durable write-ahead journal entry with physical disk synchronization (`sync_all`) must succeed before moving any file; failures abort file movement to prevent unrecoverable data loss.
- [x] Recursive registry backup error propagation: all subkey enumeration, value reading, and file write errors propagate immediately instead of silently skipping entries.
- [x] Registry backup integrity verification & multi-type roundtrip: checks header, file length, and syntax structure across all 6 Windows value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) and nested subkeys before deleting any key.
- [x] Registry restoration error reporting: reports exact counts of restored keys and values, captures every failure, and never claims success on partial restoration.
- [x] Process creation-time fail-closed verification: process termination rejects processes whose creation time cannot be determined or does not match the snapshot, preventing PID recycling races.
- [x] CI runner environment fix: patched MSVC toolchain wrapper (`scripts/msvc-env.mjs`) to preserve Windows `Path` and locate `cargo` in CI runners, resolving the `verify-cleanup.mjs` failure.
- [x] End-to-end fixture test suite: added `rc1_e2e_fixture.rs` and `scripts/verify-e2e.mjs` (`RC1_E2E_FIXTURE_OK`) covering uninstallation, process management, registry backup/restore, and residual cleanup.
- [x] UI/UX audit & branding cleanup: removed obsolete "Phase 1 is read-only" from bundle descriptions and Settings, verified all six views.
- [x] Release automation gating: `.github/workflows/release.yml` enforces test suites, runs installer verification, and publishes unsigned release candidates as draft pre-releases with explicit disclaimers.

## Phase 6 — Competitive Expansion & Explorer Integration (implemented)

- [x] Native Windows Explorer context menu integration: per-user `HKCU\Software\Classes` shell verbs for folders, drives, and executables (Analyze, Find Duplicates, Storage Breakdown, Uninstall Review, Related Files) with icon binding, clean unregistration, and Settings toggle/repair controls.
- [x] Enhanced multi-source application discovery: Microsoft Store / AppX / MSIX read-only inventory with Windows Package Manager lifecycle guidance, Steam multi-library game discovery (`libraryfolders.vdf` + `appmanifest_*.acf`), and bounded heuristic portable executable detection.
- [x] Disk Analyzer → Cleaner direct review: Selecting cleanable cache directories from treemap or folder tree surfaces "Review for Cleanup", displaying expected reclaimable bytes and launching the review-first workflow.
- [x] Extensible Declarative Cleanup Providers: Versioned (`schema_version: 1`) provider definitions with strict path template validation, safe environment root enforcement, and automated rejection of malicious path definitions.
- [x] Competitive Performance & Benchmarking: Added `scripts/benchmark-competitive.mjs`, measuring cold vs warm latency, peak working set, file counting rules, and cancellation response against WinDirStat/WizTree metrics.
- [x] UI/UX Audit & Accessibility Polish: Source filter tabs, status badges, clear Store/Portable app lifecycle guidance, high-contrast dark/light tokens, and full keyboard navigation.

## Phase 7 — Code Signing Architecture & Release Provenance (implemented)

- [x] Standardized Windows metadata: enforced product name (`Cyron Uninstaller`), version dynamically derived from `package.json`, publisher `Ry0x7`, UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`, internal name `cyron.exe`, legal copyright `Copyright © 2026 Ry0x7`.
- [x] Metadata verification: `npm run verify:metadata` (`scripts/verify-windows-metadata.ps1`) asserts PE VERSIONINFO, WiX MSI database properties, NSIS installer headers, and app identity.
- [x] Local development signing (`DEV_TEST_SIGNED`): automated certificate lifecycle in `Cert:\CurrentUser\My` via `scripts/create-dev-cert.ps1` (RSA 3072, SHA-256), signing via official Windows SDK `signtool.exe` (`scripts/sign-dev-build.ps1`), and `npm run build:dev-signed`.
- [x] Authenticode signature verification & negative testing: `npm run verify:signatures` (`scripts/verify-signatures.ps1`) verifies cryptographic file integrity separately from root authority, with negative tests for unsigned files, tampered binaries, and rogue certificate subjects (`SIGNATURES_VERIFIED_OK`).
- [x] SignPath Foundation readiness assessment: requirement-by-requirement evaluation in `docs/SIGNPATH_READINESS.md` tracking OSI license, public repository, and project reputation prerequisites.
- [x] SignPath release automation: `.github/workflows/signpath-release.yml` with manual dispatch, full test suite gates, unsigned hash provenance, SignPath request submission, signature verification, and draft release generation.
- [x] Code signing and release policy: documented in `docs/CODE_SIGNING_POLICY.md` with SignPath attribution, contributor responsibilities, privacy policy, and user verification instructions.
- [x] Git history privacy sanitization: executed operator directive to purge historical `.chryypt/` data from GitHub history using `git-filter-repo`.

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
