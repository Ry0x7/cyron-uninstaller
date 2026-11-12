# Gates: Cyron Uninstaller Phase 4 — Uninstall engine & Priority 0 hardening

OWNS: src/**, src-tauri/**, scripts/**, .github/**, package.json, package-lock.json, GATES.md,
README.md, ROADMAP.md, HANDOFF.md, AGENTS.md, docs/**

Scope: application uninstallation engine (standard uninstaller execution, MSI normalization, shell
rejection), process manager (application-linked discovery, critical process deny list, graceful
termination), leftover detection (filesystem + registry, confidence and risk tiers, shared vendor
guard), staged residual removal (automated .reg registry backup before deletion, journaled quarantine),
batch uninstallation with failure isolation, and Priority 0 cleanup recovery hardening (crash-consistent
write-ahead journal, atomic Win32 MoveFileExW no-overwrite restore, snapshot binding, purge preservation,
quarantine storage & low-space limits).

Last run: 2026-11-06, Windows 10/11 development machine, MSVC toolchain via
`scripts/msvc-env.mjs`.

## Build and suites

- [x] G1: Rust backend compiles for every target without warnings
  CHECK: node scripts/msvc-env.mjs cargo check --manifest-path src-tauri/Cargo.toml --all-targets
  EXPECT: exit 0
  EVIDENCE: exit 0, `Finished dev profile … in 4.90s`, no warnings emitted.

- [x] G2: the complete Rust suite passes
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
  EXPECT: exit 0
  EVIDENCE: exit 0. 141 lib tests (2 ignored), 25 security, 5 cleanup_stress, 1 real_system,
  5 stress_scan (1 ignored) → 177 passed, 3 ignored, 0 failed.

- [x] G3: frontend typecheck
  CHECK: npm run typecheck
  EXPECT: exit 0
  EVIDENCE: exit 0, no diagnostics.

- [x] G4: frontend unit tests
  CHECK: npm test
  EXPECT: exit 0
  EVIDENCE: exit 0. 78 tests in 8 files, including `api.test.ts` (12), `cleanup.test.ts` (16),
  `analyzerFilters.test.ts` (14), `treemap.test.ts` (7), and Hunter / System IPC contracts.

- [x] G5: frontend production bundle
  CHECK: node scripts/verify-dist.mjs
  EXPECT: DIST_OK
  EVIDENCE: `DIST_OK`, 2 assets, 507.8 KiB, index.html 444 B.

## Cleanup safety & Priority 0 Hardening

- [x] G6: permission-boundary and protected-path tests pass
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security
  EXPECT: exit 0
  EVIDENCE: exit 0, 25/25. Covers: destructive calls confined to reviewed modules (`cleanup.rs`,
  `history.rs`, `leftovers.rs`); critical process termination protection; shell injection rejection;
  leftover guard blocking system roots and profile folders; registry backup creation before deletion;
  every catalog cleanup path passes the guard; protected/ancestor paths refused; hostile ids and run ids refused;
  junction not followed; links and directories never moved; restore never overwrites; purge removes
  only batch files; read-only blocks execution; hard links counted once; fixture round trip; and two
  plan-to-execution races move nothing outside the target and report the swap instead of failing silently.

- [x] G7: preview, execution, quarantine, restore and purge work on isolated fixtures
  CHECK: node scripts/verify-cleanup.mjs
  EXPECT: CLEANUP_FIXTURE_OK
  EVIDENCE: `CLEANUP_FIXTURE_OK`. Fixture at `%TEMP%\cyron-cleanup-<pid>`: exact preview counts and
  bytes, quarantine of 4 files, outside-file untouched, manifest and recovery journal written,
  batch listed, byte-exact restore, batch retired on full restore, locked file skipped while the rest move,
  purge accounting, read-only refusal.

- [x] G8: large fixtures stay bounded, cancellable and partial-failure tolerant
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress
  EXPECT: exit 0
  EVIDENCE: exit 0, 5/5 (wide fixture preview bounded while execution moves everything, locked file,
  cancelled run claims nothing it did not move, consecutive batches independent, empty/missing
  targets).

- [x] G9: protected catalog entries can never be planned or executed
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib cleanup::
  EXPECT: exit 0
  EVIDENCE: exit 0, 26/26 cleanup unit tests, including protected refusal, exclusion enforcement,
  snapshot binding, recovery journal consistency, atomic restore, and storage limits.

## Native behaviour

- [x] G10: debug desktop binary builds
  CHECK: npm run build:desktop
  EXPECT: exit 0
  EVIDENCE: exit 0, `Built application at: src-tauri\target\debug\cyron.exe`.

- [x] G11: built executable is recognizable as the Cyron desktop app
  CHECK: node scripts/verify-desktop-binary.mjs
  EXPECT: DESKTOP_BINARY_OK
  EVIDENCE: `DESKTOP_BINARY_OK`, 15.73 MiB.

- [x] G12: the built app launches a window
  CHECK: node scripts/smoke-desktop.mjs
  EXPECT: DESKTOP_SMOKE_OK
  EVIDENCE: `DESKTOP_SMOKE_OK`, window title "Cyron".

## Frontend integration

- [x] G13: the Cleaner drives real preview, confirmation, progress, results, quarantine and history
  CHECK: the compiled desktop window against the running Vite dev server, screenshots captured with
  `scripts/capture-window.ps1`
  EXPECT: every state renders; protected rows are unselectable; confirmation is required; no
  destructive action runs without a preview
  EVIDENCE: passed in both themes on 2026-11-06. Measured data from this machine (28 of 48 catalog
  locations exist, 24.8 GB detected, 13.4 GB safe / 121 MB review / 11.3 GB protected, 8 protected
  rows with a lock instead of a checkbox). Selecting *User temporary files* showed the sticky
  "1 location selected · 4.46 GB measured · moved to quarantine, not deleted" bar; **Preview
  cleanup** opened the drawer with "4.14 GB in 100,000 files will move to quarantine", the
  read-only banner, target path plus impact copy, group/files/size/expiry rows, the planned-file
  list with per-file sizes, and a dimmed **Move to quarantine** (disabled while unconfirmed and
  read-only). Dark-theme Cleaner page captured before the fix, light-theme page and preview drawer
  after it.
  NOTE: two defects were found and fixed by this pass. `preview_cleanup` refused while read-only,
  which made the whole review-first workflow unreachable until cleaning was enabled; previewing only
  reads, so the guard now applies to execution alone (see G6). Overview joined its cache warnings
  into one run-on sentence; it now renders them as a list (`ov-warning-list`). Execution against a
  real user cache was **not** performed — that remains fixture-only evidence in G7/G8.

- [x] G14: no earlier feature claims that cleaning is impossible
  CHECK: manual copy review of Overview, Cleaner, Settings, Analyzer and README
  EXPECT: no "Phase 3 arrives later" or read-only-only claims left in shipped copy
  EVIDENCE: README banner/status, Settings stage label and the Cleaner copy now describe
  review-first cleaning with quarantine. Remaining "read-only" strings describe the Analyzer's
  duplicate/storage reports and the backend read-only *mode*, both accurate.

## Documentation

- [x] G15: milestone docs describe exactly what shipped
  CHECK: manual review of ROADMAP, HANDOFF, docs/IMPLEMENTATION_STATUS, docs/SECURITY,
  docs/ARCHITECTURE, README and AGENTS
  EXPECT: provider list, safety limits and verification evidence match the code; no unverified
  cleanup claim, no executed-run claim against personal data
  EVIDENCE: updated in the same commit. Numbers quoted (48 catalog locations, 27 safe / 9 caution /
  12 protected, 19 commands, preview cap 400, 100,000 files per target, 10-minute plan TTL, 7-day
  retention, 100-entry log) are read from `caches.rs`, `cleanup.rs`, `commands.rs` and `lib.rs`.

## Phase 4 Application Uninstallation & Priority 0 Hardening

- [x] G16: process manager discovery and critical process protection
  CHECK: `src-tauri/src/process.rs`, unit tests and `tests/security.rs`
  EXPECT: discover only processes executing from the app install location; critical system processes
  (`csrss.exe`, `explorer.exe`, `services.exe`, etc.) and system directory executables cannot be terminated.
  EVIDENCE: verified by `critical_system_processes_cannot_be_terminated` in `tests/security.rs`.

- [x] G17: standard uninstaller execution and shell command rejection
  CHECK: `src-tauri/src/uninstall.rs`, unit tests and `tests/security.rs`
  EXPECT: commands tokenized safely, MSI `/x` normalized, shell scripts (`cmd.exe`, `pwsh`, `powershell.exe`)
  strictly rejected. Exit code 3010 recognized as reboot required.
  EVIDENCE: verified by `uninstaller_runner_rejects_arbitrary_shell_commands` in `tests/security.rs` and
  uninstall unit tests.

- [x] G18: leftover detection with root guards and vendor protection
  CHECK: `src-tauri/src/leftovers.rs`, unit tests and `tests/security.rs`
  EXPECT: scan likely directories and registry subkeys; guard refuses roots, Windows, ProgramData, and
  top-level profile folders.
  EVIDENCE: verified by `leftover_scanner_guards_system_roots_and_user_profiles` in `tests/security.rs`.

- [x] G19: automated registry backup before modification
  CHECK: `src-tauri/src/leftovers.rs` and `tests/security.rs`
  EXPECT: `.reg` export created in quarantine batch directory prior to any registry key deletion.
  EVIDENCE: verified by `leftover_cleanup_creates_registry_backup_before_deletion` in `tests/security.rs`.

- [x] G20: batch uninstallation sequential queue with failure isolation
  CHECK: `src/features/uninstaller/Uninstaller.tsx`, `AppDrawer.tsx`
  EXPECT: sequential execution, individual exit code tracking, uninstaller errors do not halt queue,
  separate user confirmation required for leftover review.
  EVIDENCE: verified by batch drawer UI flow and API contract tests.

- [x] G21: durable crash-consistent recovery journal
  CHECK: `src-tauri/src/cleanup.rs`
  EXPECT: `recovery_journal.jsonl` written and flushed before any file rename; corrupt or missing
  `manifest.json` reconstructed from journal.
  EVIDENCE: verified by `recovery_journal_allows_manifest_reconstruction_after_crash` in unit tests.

- [x] G22: atomic no-overwrite restoration & low space protection
  CHECK: `src-tauri/src/cleanup.rs`
  EXPECT: Win32 `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, destination race protection,
  quarantine storage cap (20 GiB) and minimum free disk space check (512 MiB).
  EVIDENCE: verified by `restore_never_overwrites_an_existing_file` in `tests/security.rs` and
  `quarantine_enforces_storage_cap_and_disk_free_space` in unit tests.

## Phase 4.1 Priority 0 Hardening & Phase 5 Release Gates

- [x] G23: uninstaller execution boundary & server-side resolution
  CHECK: `src-tauri/src/uninstall.rs`, `commands.rs`
  EXPECT: frontend cannot supply arbitrary executable or arguments; command resolved server-side from
  registry; single-use expiring `UninstallPlan` (10m TTL); revalidation against live registry before execution.
  EVIDENCE: verified by `uninstall::tests` and `prepare_uninstall`/`execute_uninstall` command contract.

- [x] G24: process termination snapshot and PID creation time verification
  CHECK: `src-tauri/src/process.rs`, `tests/security.rs`
  EXPECT: PIDs must match verified snapshot for target app; creation time (`GetProcessTimes`) re-verified
  to defeat PID recycling; image path verified inside app root; critical processes denied.
  EVIDENCE: verified by `process_termination_rejects_unlisted_pids_and_mismatched_apps` and
  `critical_system_processes_cannot_be_terminated` in `tests/security.rs`.

- [x] G25: leftover scanner server-side plans & non-following traversal
  CHECK: `src-tauri/src/leftovers.rs`, `tests/security.rs`
  EXPECT: single-use `LeftoverPlan` (10m TTL); bounded directory walker (depth <= 32, max 100k files)
  skips junctions, symlinks, and reparse points; journal writes flushed and synchronized (`sync_all`).
  EVIDENCE: verified by `leftover_scanner_guards_system_roots_and_user_profiles` and leftover tests.

- [x] G26: recursive multi-type registry backup and restore roundtrip
  CHECK: `src-tauri/src/leftovers.rs`
  EXPECT: exports all subkeys and value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`,
  `REG_MULTI_SZ`); verifies backup integrity before deletion; restores keys and values cleanly.
  EVIDENCE: verified by `registry_backup_and_restore_roundtrip_fixture` in `src-tauri/src/leftovers.rs`.

- [x] G27: release installer generation & reproducible checksums
  CHECK: `node scripts/verify-release-bundles.mjs`
  EXPECT: both NSIS (`.exe`) and WiX MSI (`.msi`) installers build and verify with exact SHA-256 hashes.
  EVIDENCE: `RELEASE_BUNDLES_OK`:
  - NSIS: `Cyron Uninstaller_0.1.0_x64-setup.exe` (1.46 MiB)
  - WiX MSI: `Cyron Uninstaller_0.1.0_x64_en-US.msi` (2.04 MiB)

## Release Candidate 1 (RC1) Hardening Gates

- [x] G28: batch uninstallation confirmation plan & command validation
  CHECK: `src-tauri/src/uninstall.rs`, `commands.rs`, `tests/security.rs`
  EXPECT: batch uninstalls require server-side plan (`BatchUninstallPlan`, 10m TTL); exact executables and
  arguments are reviewed and approved; live registry commands are revalidated before each execution;
  forged/shell commands and empty/excessive batches are rejected.
  EVIDENCE: `batch_uninstall_plan_requires_valid_commands_and_bounds_count` passes in `tests/security.rs`.

- [x] G29: fail-closed leftover cleanup journal operations
  CHECK: `src-tauri/src/leftovers.rs`
  EXPECT: file is never moved to quarantine unless its write-ahead recovery journal entry is durably written
  and physically synchronized with `sync_all()`; journal errors abort file move to preserve data recoverability.
  EVIDENCE: verified by journal error handling in `leftovers::clean_leftovers`.

- [x] G30: recursive registry backup error propagation & multi-type verification
  CHECK: `src-tauri/src/leftovers.rs`, `tests/security.rs`
  EXPECT: all subkey enumeration, value reading, and write errors propagate without silent skipping;
  backup syntax and all 6 value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`)
  and subkeys are validated prior to deleting any registry key; restore reports all failures.
  EVIDENCE: `registry_backup_verification_detects_malformed_and_empty_files` and `registry_backup_and_restore_roundtrip_fixture` pass.

- [x] G31: fail-closed process creation time verification
  CHECK: `src-tauri/src/process.rs`, `tests/security.rs`
  EXPECT: process termination fails closed if creation time cannot be determined or does not match snapshot,
  preventing PID recycling race attacks.
  EVIDENCE: `process_creation_time_fails_closed_when_unverified` passes in `tests/security.rs`.

- [x] G32: end-to-end uninstallation & residual recovery fixture
  CHECK: `node scripts/verify-e2e.mjs`
  EXPECT: exit 0, prints `RC1_E2E_FIXTURE_OK`.
  EVIDENCE: `verify-e2e: RC1_E2E_FIXTURE_OK` passed against isolated disposable `%TEMP%` and `HKCU` fixtures.

- [x] G33: CI environment & PATH normalization fix
  CHECK: `scripts/msvc-env.mjs`, `scripts/verify-cleanup.mjs`
  EXPECT: preserves inherited Windows `Path` and `.cargo/bin` in child processes without case-clash shadowing;
  `verify-cleanup.mjs` passes cleanly without `spawnSync cargo ENOENT`.
  EVIDENCE: `verify-cleanup: CLEANUP_FIXTURE_OK` passed.

## Phase 6 — Competitive Expansion & Windows Integration Gates

- [x] G34: Windows Explorer context menu integration & clean unregistration
  CHECK: `src-tauri/src/explorer.rs`, `commands.rs`, Settings UI
  EXPECT: registers per-user verbs in `HKCU\Software\Classes` without elevation; unregisters all verbs cleanly;
  parses CLI arguments safely; never executes destructive operations directly from Explorer.
  EVIDENCE: `explorer::tests` pass; `get_explorer_integration_status`, `set_explorer_integration`, and
  `repair_explorer_integration` verified.

- [x] G35: multi-source application discovery (Store, Steam, Portable heuristics)
  CHECK: `src-tauri/src/apps.rs`, `Uninstaller.tsx`, `AppDrawer.tsx`
  EXPECT: reads Store packages (`AppModel\Repository\Packages`) with read-only inventory; parses Steam libraries
  (`libraryfolders.vdf` + `.acf`); detects bounded portable binaries (`%LOCALAPPDATA%\Programs`); tags source badges
  and filters; provides clear lifecycle guidance.
  EVIDENCE: `apps::tests::parses_steam_manifest_accurately` and inventory discovery pass; Store and Portable
  restrictions verified.

- [x] G36: Disk Analyzer to Cleaner direct review workflow
  CHECK: `src-tauri/src/commands.rs`, `src/features/analyzer/Analyzer.tsx`
  EXPECT: selecting a recognized cache directory in Analyzer tree/treemap surfaces "Review for Cleanup";
  calculates expected reclaimable bytes vs total occupied; explains protected caches; passes target to Cleaner preview.
  EVIDENCE: `commands::tests::inspect_cleanable_directory_identifies_unrecognized_folders` passes; Analyzer UI
  card integration verified.

- [x] G37: extensible declarative cleanup provider schema validation
  CHECK: `src-tauri/src/providers.rs`
  EXPECT: strict schema validation (`schema_version: 1`); allows only approved environment roots; strictly
  rejects path traversal, wildcards in roots, system roots, and attempts to mark protected data safe.
  EVIDENCE: `providers::tests` (6/6 pass, including `validates_all_default_providers`, `rejects_path_traversal`,
  `rejects_disallowed_roots`, `rejects_safe_risk_for_protected_data`).

- [x] G38: competitive performance benchmarks & architectural analysis
  CHECK: `node scripts/benchmark-competitive.mjs`
  EXPECT: produces reproducible measurements of cold/warm duration, peak working set, file counting rules,
  and cancellation latency against WinDirStat/WizTree metrics; updates `docs/BENCHMARKS.md`.
  EVIDENCE: `BENCHMARK_OK` recorded in `docs/BENCHMARKS.md`.

## Remote CI & Packaging Evidence

- [x] G39: Windows remote GitHub Actions CI run succeeds
  CHECK: `gh run view 42188091108`
  EXPECT: all build, test, and fixture jobs complete with success.
  EVIDENCE: GitHub Actions Run #42188091108 completed with `success` (6m 51s) on commit `0592792`.

## Phase 7: Code Signing Architecture & Release Provenance

- [x] G40: Windows metadata consistency across binary and installers
  CHECK: `npm run verify:metadata` (`scripts/verify-windows-metadata.ps1`)
  EXPECT: PE VERSIONINFO, WiX MSI database properties, and NSIS headers match `Cyron Uninstaller`, version `0.1.0`, publisher `Chryypt`, UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`.
  EVIDENCE: `WINDOWS_METADATA_OK` (5/5 checks passed).

- [x] G41: Chryypt Organization code signing (`CHRYYPT_SIGNED`)
  CHECK: `npm run build:signed` (`scripts/create-signing-cert.ps1` and `scripts/sign-build.ps1`)
  EXPECT: Idempotent certificate generation/reuse in `Cert:\CurrentUser\My` (CN=Chryypt Organization, RSA 3072, SHA-256); signing of `cyron.exe`, NSIS installer, and WiX MSI via official Windows SDK `signtool.exe`.
  EVIDENCE: `CHRYYPT_SIGNED_OK` (3/3 artifacts signed).

- [x] G42: Authenticode signature integrity & negative test boundary
  CHECK: `npm run verify:signatures` (`scripts/verify-signatures.ps1 -RunNegativeTests`)
  EXPECT: Separates cryptographic digest validity from root trust; rejects unsigned binaries, tampered binaries, and rogue certificate subjects; verifies all 3 release artifacts.
  EVIDENCE: `SIGNATURES_VERIFIED_OK` (3/3 artifacts passed; all 3 negative tests passed).

- [x] G43: Chryypt Organization public code signing policy
  CHECK: `docs/CODE_SIGNING_POLICY.md`
  EXPECT: Public policy governing Chryypt Organization Authenticode code signing, cryptographic verification, maintainer responsibilities, and zero-telemetry commitments.
  EVIDENCE: Documented and enforced in `docs/CODE_SIGNING_POLICY.md`.

- [x] G44: Automated release packaging & verification workflow
  CHECK: `.github/workflows/release.yml`
  EXPECT: GitHub Actions release workflow building installers, running test suites, verifying metadata and packaging, computing SHA-256 digests, and publishing release assets.
  EVIDENCE: Workflow configured and verified.

- [x] G45: Git history privacy audit & exclusion
  CHECK: Audit repository history across all branches and commits for `.chryypt/` session data.
  EXPECT: `git log --all --name-only` confirms no `.chryypt` files exist in Git history, and `.gitignore` excludes `.chryypt/`.
  EVIDENCE: Verified: 0 `.chryypt` files exist in Git history; `.gitignore` rule active.

## Phase 7.1: Hunter Mode, Startup Manager & Security Repair

- [x] G46: Broken inventory cleanup, Hunter targeting & Startup Manager E2E verification
  CHECK: `node scripts/verify-e2e.mjs` (`src-tauri/examples/rc1_e2e_fixture.rs`)
  EXPECT: `RC1_E2E_FIXTURE_OK` across all 8 disposable fixture scenarios:
  1. Hunter targeting external disposable application window.
  2. Application identity matching & strict directory boundary protection (`C:\Apps\Foo` vs `C:\Apps\FooBar`).
  3. Verified process snapshot & safe termination with PID/creation-time checks.
  4. Unregistered application review-first forced removal with backend-verified snapshots.
  5. Broken registry entry removal, 2-step single-use plan, multi-type `.reg` backup in quarantine, write-ahead journal, and replay/tamper rejection.
  6. Startup entry reversible disable (`0x03`) and enable (`0x02`) via Windows standard `StartupApproved` keys, plus system security component protection.
  7. High-performance repeated targeting without inventory rescans (50 calls in 44.7 µs; avg <1 µs).
  8. Critical Windows process & shared system directory refusal.
  EVIDENCE: `RC1_E2E_FIXTURE_OK` (8/8 scenarios passed).

## Phase 7.2: Release-Blocking Correctness & Desktop Validation

- [x] G47: Broken entry live metadata fingerprint binding & in-app registry restoration
  CHECK: `src-tauri/src/leftovers.rs`, `src-tauri/src/commands.rs`, `rc1_e2e_fixture.rs` Scenario 5, and isolated HKCU unit tests
  EXPECT: Binds BrokenEntryPlan to SHA-256 fingerprint over all normalized live registry metadata; re-reads and compares live metadata immediately before deletion; rejects altered, recovered, or no-longer-broken entries; generates collision-resistant plan IDs; records manifest.json and recovery_journal.jsonl in quarantine; verifies in-app registry restoration workflow (`restore_registry_backup` IPC command).
  EVIDENCE: Passed all isolated HKCU unit tests and Scenario 5 of RC1 E2E fixture.

- [x] G48: Hunter process snapshot isolation & target token isolation
  CHECK: `src-tauri/src/hunter.rs`, `src-tauri/src/commands.rs`, `src-tauri/tests/security.rs` (`concurrent_hunter_and_uninstaller_snapshots_stay_isolated`), and `rc1_e2e_fixture.rs` Scenario 3
  EXPECT: Generates unique opaque tokens (`hunter-target-...`); isolates Hunter process snapshots in `AppState::hunter_snapshots` and `AppState::hunter_app_snapshots`; guarantees concurrent Hunter cursor polling never overwrites or mutates active Uninstaller process snapshots; revalidates PID, creation time, and image path; rejects stale targets (> 300s).
  EVIDENCE: `concurrent_hunter_and_uninstaller_snapshots_stay_isolated` passed; `rc1_e2e_fixture.rs` Scenario 3 passed.

- [x] G49: Real external window targeting with disposable Win32 GUI target (Local Desktop: PASS; Headless CI: NOT VERIFIED)
  CHECK: `src-tauri/examples/disposable_gui_target.rs`, `src-tauri/examples/rc1_e2e_fixture.rs` Scenario 1 & 2
  EXPECT: Spawns real Win32 window with valid HWND; verifies `WindowFromPoint` cursor resolution, process image matching, crosshair drag/release, locking/cancellation, and directory boundary collision protection (`C:\Apps\Foo` vs `C:\Apps\FooBar`).
  EVIDENCE: Verified on local Windows 11 desktop: real HWND created, targeted, locked, and terminated cleanly. In headless CI without desktop window manager / interactive session, recorded as NOT VERIFIED.

- [x] G50: Startup Manager elevation awareness & 12-byte payload preservation
  CHECK: `src-tauri/src/system.rs` and `rc1_e2e_fixture.rs` Scenario 6
  EXPECT: Differentiates HKCU vs HKLM and per-user vs all-users Startup folders; checks write permissions via `is_hklm_startup_writable`; disables toggles (`can_toggle: false`) when unelevated instead of false success; preserves exact 12-byte raw binary payload structure with FILETIME updates in bytes 4..12 matching Windows Task Manager.
  EVIDENCE: Passed `rc1_e2e_fixture.rs` Scenario 6 and unit tests.

- [x] G51: Honest Hunter Mode performance & memory stability benchmark
  CHECK: `node scripts/verify-hunter-perf.mjs` (`src-tauri/examples/hunter_benchmark.rs`)
  EXPECT: Measures min/mean/p50/p95/p99 targeting latencies; reports in-memory matching (<1 ms) vs cold discovery (~62 ms); verifies bounded working set memory stability across 1,000 sustained continuous cycles (< 5 MB variance).
  EVIDENCE: `HUNTER_BENCHMARK_OK` (in-memory matching p50=674.8 µs, window resolution p50=0.6 µs, cold discovery=62 ms, speedup=83x, memory delta=0 KB within paging bounds).

## Phase 7.3: Recovery Integrity & Release Validation

- [x] G52: Fail-closed broken entry recovery, atomic manifest persistence & scope enforcement
  CHECK: `src-tauri/src/leftovers.rs`, `src-tauri/src/cleanup.rs`, `src-tauri/examples/rc1_e2e_fixture.rs` Scenario 5, and unit tests
  EXPECT: Eliminates ignored manifest write errors; writes write-ahead recovery journal (`sync_all`) and verified manifest before registry deletion; manifest binds backup SHA-256 checksum, hive, subkey, and application identity; parses and verifies all section headers in `.reg` file, rejecting any out-of-scope sections outside the approved key; re-reads and re-verifies live metadata immediately before deletion to minimize race window.
  EVIDENCE: Passed `manifest_write_failure_prevents_registry_deletion`, `registry_tampering_and_conflict_prevention_tests`, and RC1 E2E Scenario 5.

- [x] G53: Cryptographic OS-backed random IDs and tokens (`BCryptGenRandom`)
  CHECK: `src-tauri/src/leftovers.rs`, `src-tauri/src/hunter.rs`, `src-tauri/src/uninstall.rs`, `src-tauri/src/cleanup.rs`
  EXPECT: Uses Windows CNG `BCryptGenRandom` (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`) to provide 128-bit cryptographic entropy for all server-held plan IDs (`generate_collision_resistant_plan_id`), Hunter target tokens (`hunter-target-...`), and uninstallation batch tokens, maintaining collision detection and bounded TTL expiration.
  EVIDENCE: Verified across all plan generation calls and token verification tests.

- [x] G54: Live registry key conflict protection & safe recovery decision
  CHECK: `src-tauri/src/leftovers.rs` (`restore_registry_backup_with_policy`), `src-tauri/src/commands.rs`, `src/features/uninstaller/UninstallWizard.tsx`
  EXPECT: Checks if the target subkey already exists in the live registry prior to restoration; refuses silent overwrites by returning a clear conflict error unless explicit operator confirmation (`overwriteConflict: true`) is provided; UI offers safe "Overwrite & Restore" option upon conflict.
  EVIDENCE: Passed in unit tests and RC1 E2E Scenario 5.

- [x] G55: Strict quarantine recovery isolation
  CHECK: `src-tauri/src/cleanup.rs` (`restore_batch`, `purge_batch`, `read_journal`), `rc1_e2e_fixture.rs` Scenario 5
  EXPECT: Restoring file quarantine never deletes or invalidates an outstanding registry backup; purging a mixed batch purges only quarantined files while preserving `registry_backup.reg` and its manifest entry; journal records distinguish file entries from `registry_backup` entries.
  EVIDENCE: Passed in `rc1_e2e_fixture.rs` Scenario 5.

- [x] G56: Strict external GUI targeting verification with real HWND (Local Desktop: PASS; Headless CI: NOT VERIFIED)
  CHECK: `src-tauri/examples/disposable_gui_target.rs`, `src-tauri/examples/rc1_e2e_fixture.rs` Scenario 1 & 3
  EXPECT: Requires real GUI target executable `disposable_gui_target.exe` (non-GUI fallback strictly prohibited); asserts `WindowFromPoint` resolves the exact HWND, PID, and title; tests locking, cancellation, and verified termination. If desktop interaction is unavailable (headless CI), recorded as NOT VERIFIED rather than false PASS.
  EVIDENCE: Local desktop verification: PASS (disposable GUI target HWND and PID matched, process terminated). Headless CI without interactive desktop: recorded as NOT VERIFIED.

## Not gated

- [ ] Real cleanup of a user cache on this machine: deliberately not performed during verification;
  the engine was exercised only against isolated fixtures.
- [ ] Elevation: Cyron never requests administrator rights for cleaning; locked or system-owned
  files are reported and skipped.

## Capability improvements — 2026-11-09

Acceptance contract recorded before implementation in .unlazy/capabilities/PLAN.md. Existing gates above remain historical evidence. Task gates cover Analyzer actions/safety, Cleaner scope/classification/recovery, Uninstaller outcomes, visual states, frontend/Rust/docs/build and fixture integration. Results will be appended after verification.

## Capability acceptance — 2026-11-09

Task-specific acceptance was recorded before implementation in .unlazy/capabilities/PLAN.md; the historical gates above are preserved. The current verification receipts are separate from historical release evidence.

- [x] G57: Analyzer supports reviewed file/folder selections, separate Recycle Bin and permanent actions, expiring single-use plans, protected/reparse/stale-target refusal, progress, cancellation, partial results and scan invalidation.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test analyzer_actions
  EXPECT: test result: ok
  EVIDENCE: Public permanent/cancellation fixture tests passed; module tests cover expiration/reuse, protected/reparse paths, altered targets and partial failures. The separately invoked interactive test confirmed its unique marker in the real Windows Recycle Bin. Desktop review and permanent execution removed only the disposable 8 KB marker, preserved its sibling and invalidated the scan. Windows shell pathname identity has the documented final same-name race. Rendering coverage is qualified in G61.
- [x] G58: Cleaner scopes supported locations to selected local drives, shows evidence/age/risk/consequences, preserves unrelated fixture files, quarantine journaling, limits and no-overwrite restoration across volumes.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib cleanup::tests
  EXPECT: test result: ok
  EVIDENCE: Parent cache classification and cleanup regression suites passed, including changed-file refusal, unrelated single-file siblings, hard links, recovery and no-overwrite conflicts. Cross-volume quarantine/restore fixture passed where another writable local volume was available. Remnants now enter the same capped, journaled CleanupPlan engine and recheck app absence before execution. Unknown stale installers are intentionally not inferred; known removed-app metadata is session-scoped and its existing candidate contract has no age.
- [x] G59: Uninstaller reviews live registered commands, skips an empty process step, separates vendor exit from verified inventory absence, distinguishes cancellation/restart/remaining/failure, and supports stop-after-current batch results.
  CHECK: npx vitest run src/features/uninstaller/uninstallerFlow.test.ts src/lib/capabilities.test.ts
  EXPECT: Test Files
  EVIDENCE: Frontend outcome/IPC regressions passed (92 tests across the complete frontend suite). Rust server reconciliation and changed-command tests passed. Root reviewed batch cancellation, Store guidance and the empty-process shortcut. Minimum-window search, empty results and clear-filter recovery were inspected in the desktop app. No real application uninstall was performed.
- [x] G60: frontend, Rust, documentation, desktop build and isolated cleanup/uninstall/recovery integration checks succeed using the MSVC wrapper where required.
  EVIDENCE: 92 frontend tests passed across 11 files; TypeScript passed. Full Rust suite passed (207 passed, 4 ignored); the ignored interactive Recycle Bin fixture was separately invoked and passed. Documentation, MSVC desktop debug build, isolated cleanup/recovery fixture and eight-scenario uninstall/process/registry/recovery fixture passed. Signing, installer bundles and release metadata were not rerun for this change. Current automatic command receipts live in .unlazy/capabilities/GATES.md.
- [x] G61: actual UI reviewed at representative window sizes with loading, empty, error, progress and result evidence; unexercised vendor/Store-specific scenarios named explicitly.
  EVIDENCE: Eight scoped visual/interaction checks recorded in .unlazy/capabilities/visual/visual-evidence.json; integrity verification passed (76 files, including 59 source hashes). Inspected 1320x860 and minimum 1000x680 client windows and expanded Uninstaller. Covered Cleaner loading/C-only scan/inaccessible warning/session-remnant empty state, Uninstaller empty search/clear-filter recovery/Store guidance, and Analyzer protected-target refusal/review/live progress/cancel/result/rescan. A disposable 4000-file folder stopped with 2115 files deleted and an independently counted 1885 preserved; unrelated siblings remained. No real vendor uninstall, real cache cleanup, full screen-reader traversal or exhaustive provider/state matrix was exercised.
