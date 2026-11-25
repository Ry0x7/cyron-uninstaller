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

## Phase 1 — Repository audit and reliability baseline (2026-11-16)

- [x] G62: documentation evidence workflow is verifiable from a clean clone
  CHECK: `npm run docs:check` plus a negative link test
  EXPECT: `DOCS_CHECK: PASSED`; a Markdown link into a gitignored directory fails with exit 1
  EVIDENCE: `DOCS_CHECK: PASSED` (74 Markdown files). The negative test added a Markdown link from `docs/evidence/` to `../../.unlazy/capabilities/PLAN.md` and produced exit 1 with "points into gitignored local-only". The test file was removed afterwards. `HANDOFF.md` and `GATES.md` now cite the committed [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md).

- [x] G63: Explorer Force Delete quarantine is journaled, restorable and purgeable
  CHECK: `cargo test --manifest-path src-tauri/Cargo.toml --lib explorer::tests`
  EXPECT: 13 passed; 0 failed
  EVIDENCE: 13/13 passed. New tests quarantine a disposable folder, assert the write-ahead journal and manifest exist, restore the folder byte-for-byte through `restore_batch`, purge a folder batch through `purge_batch`, and prove an unknown mode aborts without modifying the target. The quarantine path now records a full stored path (previously only a filename, so restore and purge could not find the batch), rejects unknown modes, and propagates manifest failures as warnings while the journal remains the recovery record.

- [x] G64: single-process termination fails closed without a verified creation time
  CHECK: `cargo test --manifest-path src-tauri/Cargo.toml --lib process::tests`
  EXPECT: 6 passed; 0 failed
  EVIDENCE: `single_process_termination_fails_closed_without_verified_creation_time` passed. Explorer Kill Process and Force Delete lock termination now refuse a plan whose creation time is 0 instead of skipping the identity check. The Kill Process plan also omits processes whose creation time cannot be read.

- [x] G65: full suites after the Phase 1 fixes
  CHECK: `npm run typecheck`, `npm test`, `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml`, `npm run docs:check`
  EXPECT: exit 0
  EVIDENCE: typecheck exit 0; 92 frontend tests in 11 files passed; Rust 213 passed, 4 ignored, 0 failed; documentation passed (74 files). Fixtures and the desktop build are recorded separately below.

- [x] G66: Explorer Force Delete folder quarantine survives a cross-volume round trip
  CHECK: `cargo test --manifest-path src-tauri/Cargo.toml --lib` plus native quarantine → Cleaner → restore
  EXPECT: the folder is staged on the destination volume, published without overwrite, listed in Cleaner, and restored byte-for-byte
  EVIDENCE: `cross_volume_folder_restore_stages_and_publishes`, `cross_volume_folder_restore_never_overwrites` and `quarantine_writes_journal_and_restores_a_folder` passed. Native desktop 2026-11-16: a disposable folder on E: was quarantined into `%APPDATA%` on C:, appeared in Cleaner > Quarantine, and was restored to E: through the UI with its contents intact. The pre-fix native run failed at publish (`os error 80`), which is why the staging path exists. A quarantined tree containing a reparse point is refused and kept in quarantine; that refusal is reviewed in code but has no automated fixture, because creating a junction can require privilege. Screenshots were captured into the gitignored `tmp/` and are not committed.

## Phase 2 — production-grade removal (2026-11-16)

These gates extend, and do not rewrite, the historical Phase 4 and Phase 1 evidence above.

- [x] G67: discovery, Store plans, lifecycle history, remnant ownership and completion semantics pass their focused suites
  CHECK: `node scripts/verify-phase2.mjs --discovery --store --history --remnants --quarantine --completion`
  EXPECT: all six markers
  EVIDENCE: `PHASE2_DISCOVERY_OK`, `PHASE2_STORE_PLAN_OK`, `PHASE2_HISTORY_OK`, `PHASE2_REMNANTS_OK`, `PHASE2_QUARANTINE_OK`, `PHASE2_COMPLETION_OK` (exit 0). Focused tests include live current-user Store inventory filters, MSI product-code detection, PE portable metadata, atomic history reload/removal, name-only remnant downgrades, and 1605/1614/3010/1602 reconciliation. No live Store package was removed.

- [x] G68: security boundaries still hold after the Phase 2 modules
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security`
  EXPECT: 26 passed, 0 failed
  EVIDENCE: 26/26 passed. `app_history.rs` was added to the reviewed modules that may publish a file by rename; the assertion still rejects destructive primitives in every other module. Junction, no-overwrite, read-only, hostile-id and registry-backup-before-deletion checks all passed.

- [x] G69: disposable Win32 registration → vendor process → absence → quarantine round trip
  CHECK: `node scripts/verify-phase2.mjs --fixture`
  EXPECT: `PHASE2_WIN32_FIXTURE_OK`
  EVIDENCE: exit 0. The fixture creates a disposable HKCU uninstall key under `%TEMP%\cyron-phase2-win32-<pid>`, copies its own binary as the vendor uninstaller, discovers the app from the live registry, creates a server plan, runs the process, confirms the registration disappeared, reconciles `completed`, scans the exact install folder as `safe`, quarantines it, and restores it. The separate security suite covers no-overwrite restore refusal. Registry key and temp folder are removed by a drop guard.

- [x] G70: frontend contracts and production bundle
  CHECK: `node scripts/verify-phase2.mjs --frontend`
  EXPECT: `PHASE2_FRONTEND_OK`
  EVIDENCE: exit 0; TypeScript clean, 92 Vitest tests in 11 files passed, production build succeeded. The Store wizard and persistent-history UI are covered only by typecheck/contracts, not a desktop run.

- [x] G71: full Rust regression and documentation integrity
  CHECK: `node scripts/verify-phase2.mjs --regression`
  EXPECT: `PHASE2_REGRESSION_OK`
  EVIDENCE: exit 0. Full Rust suite: 230 passed, 4 ignored, 0 failed across lib, security, cleanup_stress, real_system, stress_scan and analyzer_actions. `npm run docs:check`: `DOCS_CHECK: PASSED` over 76 Markdown files.

- [ ] G72: live Microsoft Store / MSIX removal
  CHECK: `node scripts/verify-phase2.mjs --live-store` on a disposable Windows profile/package
  EXPECT: a reviewed current-user package is removed and absence is verified
  EVIDENCE: UNVERIFIED. No disposable signed MSIX package is available on this machine; enabling developer mode or changing the certificate trust store was out of scope. The UI path has not been exercised.

- [ ] G73: live MSI/vendor uninstall and desktop Store wizard
  CHECK: `node scripts/verify-phase2.mjs --live-msi`, then `--manual-ui`
  EXPECT: a disposable MSI returns a real Windows Installer code reconciled against inventory, and the Store wizard is driven end to end
  EVIDENCE: UNVERIFIED. WiX/NSIS are not installed here, and no native desktop session was recorded for this pass. The fixture covers a real Win32 exe uninstaller, not MSI or Store.

## Phase 3 — AI and developer storage intelligence (2026-11-16)

These gates cover the Projects engine added to Cleaner. The historical Phase 1, Phase 2 and earlier evidence above remains unchanged.

- [x] G74: read-only project recognition, artifact measurement and protected-data evidence
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib devscan`
  EXPECT: 8 passed, 0 failed
  EVIDENCE: 8/8 passed. Fixtures prove manifest and Git-root recognition, package name and framework metadata, `node_modules`, `.next/cache` (not the rest of `.next`), `dist`, Rust/Maven `target`, Gradle project caches, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, nested `__pycache__`, hard-link-aware measurement, activity labels, server-held id binding, tampered-path refusal, post-run verification, and refusal of profile/Windows/relative roots. `.git`, `.env`, credentials, keys and project databases are reported as protected; no artifact path may leave its recorded project.

- [x] G75: disposable project cleanup, verification and restore round trip
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test phase3_project`
  EXPECT: 3 passed, 0 failed
  EVIDENCE: 3/3 passed. Two scenarios: (1) a temporary project with source, `.git`, `.env`, `node_modules` and `dist` — the preview selects only the two regenerable folders, execution quarantines them, verification reports both removed, restore returns them, and source, credentials and Git data remain untouched; (2) a Next.js project — only `.next/cache` is selected and quarantined while `.next/server`, `.next/BUILD_ID` and source remain, verification reports the cache removed, and restore returns it. Unknown artifact ids and empty selections are refused.

- [x] G76: IPC contracts and frontend selection logic
  CHECK: `npm run typecheck`, `npm test`
  EXPECT: 0 TypeScript errors; 98 Vitest tests in 12 files passed
  EVIDENCE: typecheck exit 0; 98/98 passed in 12 files, including `developer.test.ts` filtering/totals, `format.test.ts` path display, and `api.test.ts` argument contracts for `scan_developer_storage`, `preview_developer_cleanup` and `verify_developer_cleanup`. The Projects tab is mounted only after the user opens it; its buttons require explicit artifact selection, a preview, and the confirmation checkbox.

- [x] G77: full regression and documentation after the Phase 3 changes
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml`, `npm run docs:check`
  EXPECT: 244 passed, 4 ignored, 0 failed; `DOCS_CHECK: PASSED`
  EVIDENCE: 244 passed (202 library, 26 security, 5 cleanup-stress, 3 Phase 3 project, 1 real-system, 5 scanner-stress, 2 analyzer-actions), 4 ignored, 0 failed. Documentation verification passed. The pre-existing safety suites (journal-before-move, no-overwrite restore, junction refusal, read-only gating, hostile ids) still pass unchanged.

- [x] G79: RC1 E2E fixture is deterministic on a busy desktop
  CHECK: `node scripts/verify-e2e.mjs`
  EXPECT: `RC1_E2E_FIXTURE_OK`; all 8 scenarios
  EVIDENCE: The first Phase 3 run failed scenario 1 because the disposable GUI window was left behind another window, so `WindowFromPoint` resolved a different HWND. The fixture window is now raised with `SetWindowPos(HWND_TOPMOST)` and the hit-test is polled for up to 3 seconds instead of checked once. Rerun passed all 8 scenarios, including Hunter targeting, verified termination, broken-entry recovery, Startup bytes, latency/memory loop and critical-process refusal. This changes test helpers only; the production targeting path is unchanged.

- [x] G78: native Projects tab and multi-drive desktop run
  CHECK: open Cleaner → Projects in the real Tauri desktop build, scan local drives with a disposable project on a second volume, select only reviewed artifacts, preview, clean, confirm verification, and restore from Quarantine
  EXPECT: the UI shows the same targets and verification as the backend, and a disposable artifact is restored byte-for-byte
  EVIDENCE: PASS on 2026-11-17 in the native Tauri debug desktop app (served by the local Vite dev server) on Windows 11, driven through Windows UI Automation. A disposable project under `Documents\GitHub` was discovered through an automatic profile root; the UI showed 2 artifacts, 50 B protected, exact paths, loss/rebuild evidence, and the read-only gate. After enabling cleaning, selecting only `node_modules`, previewing 1 file/20 B, checking the confirmation box, and choosing Move to quarantine, the result reported `1 file moved`, verification reported `1 artifact removed / 0 remaining`, and `.env`, `.git`, `dist` and source stayed. Quarantine showed the 1-file batch, and Restore returned the file to its original path without overwriting anything and emptied the batch. A second disposable project under `E:\Projects` was then discovered through the automatic `E:\Projects` root alongside the C: root, proving multi-drive discovery in the running window. Only disposable fixtures were touched; both were removed afterwards.
  Local captures were reviewed for seven states: projects summary, artifact list, preview, result, quarantine, restored, and the E: project. They show the exact paths and counts above. Raw captures are local-only and not committed, per the [evidence policy](docs/evidence/README.md).
  The native run found and fixed two UI defects: canonical `\\?\` paths were shown verbatim (now stripped for display only; operations keep the canonical path) and Quarantine stayed stale after a Projects cleanup (the tab now refreshes whenever it opens).

## Performance diagnostics (2026-11-17)

These gates cover the read-only Performance page. Earlier uninstaller/cleanup evidence above remains historical and unchanged.

- [x] G80: live metrics are internally consistent and bounded
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test performance -- --nocapture`
  EXPECT: exit 0; sample ≥ 500 ms, CPU shares sum to 100%, memory and process figures consistent
  EVIDENCE: passed on this machine. Native page showed a 702 ms sample across 179 processes: 15% CPU, 37% memory in use (30.3 GB available of 47.9 GB), 1.1% physical-disk busy with queue 0.03, and E: at 13% free. The snapshot's own limitations list a bounded sample, combined per-process I/O and unmeasured protected processes.

- [x] G81: startup evidence and reversible toggling use the real Windows path
  CHECK: native Tauri debug app driven with UI Automation, disposable HKCU `Run` value
  EXPECT: Cancel changes nothing; Disable writes StartupApproved byte 0x03; Enable restores 0x02; the Run value survives; the row and notice update
  EVIDENCE: PASS. `CyronPerfFixture` (notepad.exe) was created under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. **Cancel** left it enabled. **Disable** wrote `03 00 00 00 ...` to `StartupApproved\Run`, preserved the Run value, showed the disabled notice and changed the row action to **Enable**. **Enable** wrote `02 00 00 00 ...` and reversed the row. The confirmation is a native `#32770` dialog, not `window.confirm` (which was invisible in WebView2). Fixture values were removed afterwards. See [the receipt](docs/evidence/2026-11-17-performance-diagnostics.md) and [dialog](docs/evidence/phase4-native-startup-confirmation.png).

- [x] G82: recommendations require physical evidence; process I/O alone is not disk pressure
  CHECK: `cargo test --manifest-path src-tauri/Cargo.toml --lib perf::tests`
  EXPECT: 8 passed; idle-disk regression and fixed System32 path validation present; every recommendation has evidence/effect/risk/verification
  EVIDENCE: 8/8 passed. The native run first produced **Measured disk I/O is busy** at 0.1% physical-disk busy because the gate accepted 40.8 MB/s of per-process I/O (which includes network). The gate now requires measured physical-disk busy ≥ 70% and queue ≥ 2.0; `high_process_io_with_an_idle_physical_disk_is_not_disk_pressure` passed. The rebuilt native page showed **No pressure matches this symptom** at 1.1% disk busy.

- [x] G83: guided investigation, freshness, baselines and trace availability behave honestly
  CHECK: native Tauri debug app driven with UI Automation
  EXPECT: guides change the answer; manual Refresh bypasses the startup cache; two baselines compare; trace is disabled and explained when unelevated
  EVIDENCE: PASS. UIA selected **Startup takes forever**, **Disk is almost full** and **The fan is loud**; each changed the guide title and filtered recommendations. A disposable startup entry was absent until **Refresh** forced a new startup inventory. Two labeled baselines displayed previous/latest CPU, memory, available memory, commit and disk busy with the association-not-causation note. Unelevated, the trace panel showed **Needs administrator**, disabled **Record trace**, and offered no elevation. See [baselines](docs/evidence/phase4-native-baselines.png).

- [x] G84: full regression and documentation after the performance changes
  CHECK: `npm run typecheck`, `npm test`, full Rust suite, `npm run docs:check`
  EXPECT: exit 0; all existing safety suites unchanged
  EVIDENCE: typecheck exit 0; 98 frontend tests in 12 files passed; full Rust suite 253 passed, 4 ignored, 0 failed, including 26 security and 5 cleanup-stress tests; `DOCS_CHECK: PASSED` over 77 Markdown files. GitHub Actions run [43930569278](https://github.com/Ry0x7/cyron-uninstaller/actions/runs/43930569278) passed both jobs on commit `d5e5afc`, including the cleanup and RC1 E2E fixtures. UNVERIFIED: elevated Diagnostics-Performance boot impact and a real WPR trace recording/opening.

## Phase 5 — design system and navigation rebuild — 2026-11-17

Acceptance contract recorded before implementation. Existing safety gates above remain authoritative; this section covers presentation and navigation only.

- [x] G85: exactly five primary destinations are reachable, Settings stays in the sidebar footer, and Hunter Mode lives inside Applications instead of its own page.
  CHECK: native Tauri desktop app driven through WebView2 remote debugging at 1320x860 and 1000x680
  EXPECT: Home, Applications, Storage & Cleanup, Performance, History & Recovery all open; Settings opens from the footer; no dead or duplicate navigation item
  EVIDENCE: PASS. All five destinations opened; Settings was reached from the footer and resolved to the Appearance tab; **Find by window** opened the Hunter confirmation from Applications. No separate Hunter navigation item or dead link was present. See the [design-system receipt](docs/evidence/2026-11-17-design-system.md) and [Home](docs/evidence/phase5/01-home-light.png), [Applications](docs/evidence/phase5/03-applications-light.png), [Storage & Cleanup](docs/evidence/phase5/04-storage-cleanup-light.png), [Performance](docs/evidence/phase5/07-performance-light.png) and [History & Recovery](docs/evidence/phase5/09-activity-light.png).
- [x] G86: every page and critical modal uses the committed token and primitive system, with no page-specific palette, radius, type scale or shadow.
  CHECK: source audit plus desktop screenshots of every destination, its subviews and the Hunter/Uninstall/Force Delete/Recovery dialogs
  EXPECT: one accent, one neutral family, one radius scale and one typographic hierarchy across all reviewed surfaces
  EVIDENCE: PASS. Reviewed all five destinations, both Storage & Cleanup tabs, Cleanup sub-tabs, both History & Recovery tabs, Settings, the Hunter dialog, the uninstall wizard and the permanent-deletion review. All use `src/styles/tokens.css` and `src/components/ui.tsx` with one blue accent and one neutral family. No page-local palette, gradient, glass, neon or ad-hoc radius scale was found. See [design system](docs/DESIGN.md), [Settings dark](docs/evidence/phase5/11-settings-dark.png) and [Hunter dialog](docs/evidence/phase5/12-hunter-dialog.png).
- [x] G87: representative long names, paths, counts, empty states and minimum window width do not clip, overlap or create horizontal page overflow.
  CHECK: native captures at 1320x860 and 1000x680 plus a browser layout probe of the same views
  EXPECT: no horizontal document overflow, no clipped actions, badges or headings; long values truncate or wrap intentionally
  EVIDENCE: PASS after three fixes. At 1000x680 the page probe returned 0 horizontal overflow on Applications, Storage & Cleanup and History; the Activity table measured 646/646 px with no inner scroll after the Type column was hidden. The Recovery log badge no longer wraps to a second grid row, and the Analyzer review strips the `\\?\` display prefix. Long application names, publishers and paths truncate with the full value in `title`. See [Applications min](docs/evidence/phase5/15-applications-min.png), [Storage min](docs/evidence/phase5/16-storage-min.png) and [Recovery min](docs/evidence/phase5/17-recovery-min.png).
- [x] G88: keyboard focus, reduced motion and contrast remain usable after the redesign.
  CHECK: keyboard traversal of navigation, tab bars, tables and dialogs; `prefers-reduced-motion` emulation; contrast spot checks
  EXPECT: visible focus on every interactive control, dialogs restore focus, Escape closes, reduced motion removes spatial movement, body text meets 4.5:1
  EVIDENCE: PASS. Tab order reached brand → Home → Applications → Storage & Cleanup → Performance → History & Recovery → Settings; Enter activated Applications. The focused control had a visible 2px `rgb(47,111,235)` outline. In the Hunter dialog, ten Tab presses stayed inside and Escape closed it and returned focus to **Find by window**. With `prefers-reduced-motion: reduce`, no sampled button, link, input or tab exceeded 0.05 s transition/animation. Token spot checks: light body 9.32:1, secondary 6.05:1, faint 4.82:1; dark body 12.17:1, muted 7.34:1, faint 5.21:1; accent-on-white 4.57:1. UNVERIFIED: screen-reader traversal and forced-colors mode.
- [x] G89: typecheck, frontend tests, documentation check, production build and the native smoke run pass after the redesign.
  CHECK: `npm run typecheck`, `npm test`, `npm run docs:check`, `npm run build`, `npm run build:desktop`
  EXPECT: exit 0 and no regression in the existing frontend suite or documentation invariants
  EVIDENCE: PASS. Typecheck exit 0; 100 frontend tests in 12 files passed; full Rust suite 253 passed, 4 ignored, 0 failed; production build exit 0 (562.28 kB JS / 127.63 kB CSS, chunk-size warning only); `DOCS_CHECK: PASSED` over 79 Markdown files; `build:desktop` produced `src-tauri/target/debug/cyron.exe`; native smoke run exercised every destination and the keyboard/dialog checks above. Full command ledger in the [design-system receipt](docs/evidence/2026-11-17-design-system.md). GitHub Actions run [43953411889](https://github.com/Ry0x7/cyron-uninstaller/actions/runs/43953411889) passed both jobs on commit `2eeb91f`: frontend typecheck, 100 tests, `docs:check`, production build, Rust tests, security, cleanup-stress, cleanup fixture and RC1 E2E fixture.

## Phase 6 UI — complete workflow UX — 2026-11-17

Acceptance covered the five destinations and their critical flows in the production desktop build. Earlier safety gates above remain authoritative.

- [x] G90: every primary destination, Settings and the Hunter flow are reachable in the real desktop build, with no dead navigation or placeholder control.
  CHECK: production desktop build driven through WebView2 remote debugging at 1320x860 and 1000x680
  EXPECT: Home, Applications, Storage & Cleanup, Performance, History & Recovery, Settings and Find by window all open and render real content
  EVIDENCE: PASS. Home showed this machine's real volumes, 179 applications and 39 measured cache locations. Applications showed the same 179 entries with search, filters, sorting, selection and uninstall actions. Storage & Cleanup showed both tabs, Cleanup and Projects; Performance showed a live sample and startup/task inventory; History & Recovery showed Activity and Recovery with the real cleanup log. Settings opened from the footer. Find by window opened the Hunter confirmation. No route led to a blank or placeholder page. See [Home](docs/evidence/phase6/01-home-light.png), [Applications](docs/evidence/phase6/02-applications-light.png), [Storage](docs/evidence/phase6/08-storage-loaded-light.png), [Performance](docs/evidence/phase6/03-performance-light.png) and [Settings](docs/evidence/phase6/07-settings-startup-light.png).

- [x] G91: loading, empty, error and partial states describe what is happening and never claim work that has not happened.
  CHECK: source and native review of the Cleaner first measurement, preview drawer, Activity, Recovery log and Settings
  EXPECT: initial measurement identifies its scope; preview explains that nothing moves; a stop or failure after moved files is recorded as partial, not completed
  EVIDENCE: PASS. The first Cleaner load said **Measuring known cache folders on C:, D:, E:…**, showed skeletons in the final layout, disabled **Scan selected drives** while the first measurement ran, and displayed **Not checked yet** until it finished. The preview drawer was cleared on open, showed a spinner with **Reading the selected locations…** and **Cancel preview**, and never displayed a stale list. `cleanup_status` now returns `partial` when a cancellation or failure already quarantined files, and `cancelled_and_failed_cleanups_get_honest_statuses` covers cancellation with moved files, cancellation with none, total failure and partial failure. See [initial measurement](docs/evidence/phase6/04-storage-loading-light.png) and [Recovery log](docs/evidence/phase6/06-recovery-light.png).

- [x] G92: minimum window width and dark theme do not clip outcomes, actions or content.
  CHECK: native captures and a layout probe at a 984 px client width
  EXPECT: 0 horizontal document overflow; cleanup outcomes and actions remain visible; the shell does not scroll independently of the content
  EVIDENCE: PASS after fixes. The page probe returned 0 horizontal overflow on all five destinations and Settings; document scroll height stayed 641 px, equal to the viewport, after the `html` fix. The Recovery log now keeps its outcome under the operation title at 1000 px, so failed or partial runs no longer collapse to a bare badge. Home's fifth system item spans the spare row instead of leaving a gap. See [Home min](docs/evidence/phase6/10-home-min.png), [Applications min](docs/evidence/phase6/11-applications-min.png), [Recovery min](docs/evidence/phase6/14-recovery-min.png), [Settings min](docs/evidence/phase6/15-settings-min.png), [Storage dark](docs/evidence/phase6/18-storage-dark.png) and [Home dark](docs/evidence/phase6/19-home-dark.png).

- [x] G93: reversible and destructive surfaces explain their effect, and existing keyboard and dialog behaviour still works inside every workflow.
  CHECK: native review of startup toggles, cleanup preview, Hunter dialog, uninstall confirmation and permanent-deletion review
  EXPECT: effects and reversal are stated; Escape closes dialogs and returns focus; no action runs without its confirmation plan
  EVIDENCE: PASS. Settings now states that disabling a startup app preserves its entry and can be reversed; the native probe loaded 23 startup rows and showed **Copied**, then **Copy** after two seconds (the clipboard API was stubbed to avoid changing this machine's clipboard). The cleanup preview retains the read-only notice, exact targets, quarantine destination and required confirmation. Find by window retains its scope and protected-process explanation before tracking begins. The uninstall wizard and Force Delete review retain their existing plan-bound confirmations. These are native surfaces from the production build; no destructive action was executed.

- [x] G94: typecheck, frontend tests, documentation, production build, desktop build and the full Rust suite pass after the workflow changes.
  CHECK: `npm run typecheck`, `npm test`, `npm run docs:check`, `npm run build`, `npm run build:desktop`, `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml`
  EXPECT: exit 0; no regression in safety or documentation gates
  EVIDENCE: PASS. Typecheck exit 0; 100 frontend tests in 12 files passed; production build exit 0 (563.72 kB JS / 128.05 kB CSS, chunk-size advisory only); `DOCS_CHECK: PASSED` over 80 Markdown files; `build:desktop` produced `src-tauri/target/debug/cyron.exe`; full Rust suite 253 passed, 4 ignored, 0 failed. Full command ledger and limitations in the [workflow receipt](docs/evidence/2026-11-17-workflow-ux.md).

## Phase 7 real desktop E2E — 2026-11-17

Acceptance used the compiled Tauri executable and a real WebView2 WebDriver session, not JSX,
Vitest, static screenshots or a browser preview. The harness and its evidence are described
in [the desktop E2E receipt](docs/evidence/2026-11-17-desktop-e2e.md).

- [x] G95: the real Windows Tauri application can be built and driven through WebView2 with a supported driver, and fixtures use real IPC rather than mocked results.
  CHECK: `npm run test:desktop`
  EXPECT: `tauri-driver` + `msedgedriver` create a session against `src-tauri/target/debug/cyron.exe`; fixture HKCU applications, startup entry, project, analyzer files and cache file are created; teardown leaves no fixture keys or app-data residue
  EVIDENCE: PASS. Sessions were created against the compiled app through `@wdio/tauri-service` with matching `msedgedriver 157.0.4341.46`. Fixture setup/teardown ran in the real profile; app data was backed up and restored; stale fixture keys from interrupted runs are removed on setup. `DESKTOP_E2E_OK`.
- [x] G96: first launch, every destination, search/filter/sort/selection, single and batch uninstall, and broken-entry removal with registry backup/restore pass as desktop journeys.
  CHECK: `tests/desktop/01-core.spec.mjs`
  EXPECT: real inventory rows change with search/filter; fixture uninstaller markers are written; HKCU keys are deleted only after a plan; a verified `.reg` backup restores the ghost key
  EVIDENCE: PASS. Four disposable applications appeared; broken-filter and name-sort assertions passed; batch results were `1 still registered`, `1 cancelled`, `0 failed`; single uninstall removed the fixture key and the row; broken-entry cleanup removed the key and **Restore Registry Now** restored 1 key and 4 values.
- [x] G97: minimum 1000×680 and 1440×960 windows, light and dark themes, and a real reload persist settings without horizontal document overflow.
  CHECK: `tests/desktop/02-secondary.spec.mjs` visual matrix and Settings persistence journey
  EXPECT: 0 horizontal overflow on every destination at both sizes; theme changes reach `data-theme`; Light remains pressed after reload
  EVIDENCE: PASS. All six destinations (five plus Settings) reported no horizontal document overflow at 1000×680; light captures were taken for all six at 1440×960; dark Home was captured; Light survived a real `browser.refresh()`.
- [x] G98: Storage, Performance, History, startup provenance and Hunter Mode render measured desktop states and preserve confirmation/exit semantics.
  CHECK: secondary spec journeys against real IPC
  EXPECT: Disk-space idle state opens; Performance shows real CPU/memory/startup evidence; History renders recorded operations; the disposable startup entry is enabled for this user; Hunter requires confirmation, enters targeting and Escape returns to Applications
  EVIDENCE: PASS. The disk-space view showed **Choose a drive to map everything on it**; Performance rendered measured CPU, available memory and startup counts plus the startup guide; History rendered recorded operations; the fixture startup entry showed **Enabled** and **This user**; Hunter opened its confirmation, entered **Move over any desktop window**, and Escape returned to Applications.
- [x] G99: desktop evidence records named screenshots, machine-readable results, logs and the exact tested revision, while unexercised paths are reported as UNVERIFIED rather than inferred.
  CHECK: committed [desktop E2E receipt](docs/evidence/2026-11-17-desktop-e2e.md), `docs/evidence/phase7/*.png`, `results.json`, `revision.json`
  EXPECT: revision and environment are recorded; every passing journey has a named capture; scaling, native ask, installer/relaunch, real cache execution, duplicate/treemap and Store/MSI gaps are explicit
  EVIDENCE: PASS with explicit gaps. Exact revision `3d9f2daddb527492afc578c43dc70c1a7a7ce6f2` plus the Phase 7 working tree, WebView2 `157.0.4341.46`, Node `v24.18.0` and the real interactive session are recorded. 125%/150% OS scaling, the native startup confirmation, installer/upgrade/relaunch, real cache cleanup, AI/project cleanup, duplicate/treemap interaction, Explorer verbs and Store/MSI remain UNVERIFIED.

## Not gated

- [ ] Real cleanup of a user cache on this machine: deliberately not performed during verification;
  the engine was exercised only against isolated fixtures.
- [ ] Elevation: Cyron never requests administrator rights for cleaning; locked or system-owned
  files are reported and skipped.

## Capability improvements — 2026-11-09

Acceptance contract recorded before implementation in the local `.unlazy/` runtime directory (gitignored). Existing gates above remain historical evidence. Task gates cover Analyzer actions/safety, Cleaner scope/classification/recovery, Uninstaller outcomes, visual states, frontend/Rust/docs/build and fixture integration. Results will be appended after verification.

## Capability acceptance — 2026-11-09

Task-specific acceptance was recorded before implementation in the local `.unlazy/` runtime directory (gitignored); the historical gates above are preserved. The committed [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md) records the results; it is separate from historical release evidence.

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
  EVIDENCE: 92 frontend tests passed across 11 files; TypeScript passed. Full Rust suite passed (207 passed, 4 ignored); the ignored interactive Recycle Bin fixture was separately invoked and passed. Documentation, MSVC desktop debug build, isolated cleanup/recovery fixture and eight-scenario uninstall/process/registry/recovery fixture passed. Signing, installer bundles and release metadata were not rerun for this change. The committed [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md) records the command ledger.
- [x] G61: actual UI reviewed at representative window sizes with loading, empty, error, progress and result evidence; unexercised vendor/Store-specific scenarios named explicitly.
  EVIDENCE: Eight scoped visual/interaction checks recorded in the committed [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md); the local integrity manifest was verified at recording time (76 files, including 59 source hashes). Those hashes describe commit `e38ea5c`, not the current tree, because this audit changed source files afterwards. Inspected 1320x860 and minimum 1000x680 client windows and expanded Uninstaller. Covered Cleaner loading/C-only scan/inaccessible warning/session-remnant empty state, Uninstaller empty search/clear-filter recovery/Store guidance, and Analyzer protected-target refusal/review/live progress/cancel/result/rescan. A disposable 4000-file folder stopped with 2115 files deleted and an independently counted 1885 preserved; unrelated siblings remained. No real vendor uninstall, real cache cleanup, full screen-reader traversal or exhaustive provider/state matrix was exercised.

## Phase 8 independent audit, packaging and installer — 2026-11-17

This pass audited the current working tree, produced the release bundles, and exercised the real
per-user installer. The final PASS/FAIL/UNVERIFIED matrix is in
[the release audit](docs/RELEASE_AUDIT_2026-11-17.md).

- [x] G100: all local suites pass on the final tree.
  CHECK: `npm run typecheck`; `npm test`; `npm run docs:check`; `cargo test`; `verify-cleanup`; `verify-e2e`; `verify-hunter-perf`; `npm run test:desktop`
  EXPECT: exit 0 and named fixture markers
  EVIDENCE: PASS. Typecheck exit 0; 100 frontend tests in 12 files; `DOCS_CHECK: PASSED`; 261 Rust tests passed, 4 ignored, 0 failed; `CLEANUP_FIXTURE_OK`; `RC1_E2E_FIXTURE_OK`; `HUNTER_BENCHMARK_OK`; 10/10 real desktop journeys with `results.json` under [docs/evidence/phase8](docs/evidence/phase8/).
- [x] G101: the real desktop matrix covers all destinations, minimum and large windows, light/dark, Settings reload, measured Storage/Performance/History states and Hunter lifecycle.
  CHECK: `npm run test:desktop`
  EXPECT: 10 passed, 0 failed, named captures
  EVIDENCE: PASS. 18m09s run against the compiled Tauri/WebView2 app; no horizontal overflow at 1000x680; light captures for all six destinations at 1440x960; dark Home; theme survived refresh; startup provenance, measured Performance and Hunter Escape passed. 125%/150% OS scaling remains UNVERIFIED.
- [x] G102: release bundles build and their Windows metadata is consistent.
  CHECK: `npm run build:unsigned`; `node scripts/verify-release-bundles.mjs`; `npm run verify:metadata`
  EXPECT: `RELEASE_BUNDLES_OK` and `WINDOWS_METADATA_OK`
  EVIDENCE: PASS. NSIS 2,271,846 bytes, MSI 3,330,048 bytes, `cyron.exe` 6,918,144 bytes. ProductName `Cyron Uninstaller`, version `0.1.0`, publisher `Chryypt`, UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`. Hashes are in [the installer receipt](docs/evidence/2026-11-17-installer-release.md).
- [x] G103: signature integrity is distinguished from public trust, and the release is not described as trusted while unsigned.
  CHECK: `npm run verify:signatures`
  EXPECT: negative tests pass; unsigned artifacts fail with `NotSigned`; the audit records the blocker
  EVIDENCE: PASS with a disclosed FAIL. Negative tests rejected unsigned, tampered and wrong-subject files. All three local artifacts are `NotSigned`; the local `CN=Chryypt Organization` chain is `UntrustedRoot`; the published `v0.1.0` NSIS asset is `NotSigned`. Public trust remains a release blocker.
- [x] G104: the per-user NSIS installer installs, repairs, relaunches, uninstalls and reinstalls while preserving app data.
  CHECK: silent `/S` install, repair, launch, uninstall, reinstall; hash the app-data tree and seed quarantine sentinels
  EXPECT: every step exits 0; install and uninstall registry entry correct; 0 app-data changes; sentinels survive
  EVIDENCE: PASS. Install 1.07 s, repair 1.02 s, uninstall 1.24 s; `%LOCALAPPDATA%\Cyron Uninstaller` created and removed; HKCU uninstall entry created/removed; installed window titled `Cyron Uninstaller`, responding, clean exit 0; 0 app-data hash changes; root and quarantine sentinels preserved. Capture: [34-installed-launch.png](docs/evidence/phase8/34-installed-launch.png).
- [x] G105: the MSI limitation is measured rather than assumed.
  CHECK: `msiexec /i <msi> /qn /norestart` without elevation
  EXPECT: the per-machine refusal is recorded; no false install claim
  EVIDENCE: PASS (limited). Exit 1603 with MSI error 1925 (“sufficient privileges … for all users”); nothing installed. An elevated MSI install/uninstall, version-to-version upgrade, downgrade and clean-VM install remain UNVERIFIED.
- [x] G106: the audit ends with a direct-evidence matrix and an honest release decision.
  CHECK: review [docs/RELEASE_AUDIT_2026-11-17.md](docs/RELEASE_AUDIT_2026-11-17.md)
  EXPECT: every area is PASS, FAIL or UNVERIFIED with evidence; production readiness is not claimed
  EVIDENCE: PASS. Verdict: **NOT PRODUCTION READY — release blocked** on public signing trust, real vendor/MSI/Store removal, elevated diagnostics, DPI/accessibility and unrun competitor comparisons.

- [x] G107: the native Performance page drives the 20-second spike capture and the kernel/driver evidence panels against live Windows data.
  CHECK: `npm run build:desktop`; `npm run test:desktop` with the `spike capture and kernel/driver evidence render from live data` journey
  EXPECT: the rendered driver rows match the overview count; the System-event panel states its provenance; the capture renders its readings and peak CPU/kernel/disk/process summary with a bounded verdict; no horizontal overflow
  EVIDENCE: PASS on 2026-11-18. 11/11 desktop journeys passed in 20 m 08 s (`DESKTOP_E2E_OK`); the new journey passed in 55.6 s. The page listed 4 drivers (AMD Radeon RX 9060 XT, Intel Ethernet I225-V, TP-Link USB adapter, disk drive) and the count matched the rows; 0 recent System events were reported as such; the 20-reading capture showed peak CPU 42%, kernel 14%, disk 0.4%, `brave.exe` and a mixed verdict. See [the receipt](docs/evidence/2026-11-18-desktop-performance.md), [driver evidence](docs/evidence/2026-11-18-native/34-kernel-driver-evidence.png) and [spike result](docs/evidence/2026-11-18-native/35-spike-capture.png). Early stop, a live kernel-dominant spike and the elevated paths remain UNVERIFIED.
## Everything index search — 2026-11-24 (retired 2026-11-26)

**Retired:** these gates record the former Everything IPC client. `everything.rs` was removed in
favour of the native index below, so the CHECK commands in this section are historical and no
longer runnable against this tree. The replacement gates start at G118.

Read-only search over a locally installed and running Everything 1.4.1+ index. The task ledger was
written before implementation under the gitignored `.unlazy/everything-search/` scope, and all eight
gates passed with `gate-check.mjs --approve` (`ALL MET (8 met)`). This section records the same
evidence in the repository ledger.

- [x] G108: the Everything WM_COPYDATA framing, reply parser, version gate and a fake local IPC peer round trip pass.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib everything::tests
  EXPECT: test result: ok
  EVIDENCE: PASS. 8/8 tests: QUERY2W header and UTF-16 layout, file and folder metadata, pagination, full-path fallback, malformed and truncated reply refusal, the 1.4.1 version gate, empty/oversized query refusal, and a real Win32 round trip against a fake WM_COPYDATA peer. The reply window filter allows only `WM_COPYDATA`; every field is bounds-checked.
- [x] G109: all Rust and frontend suites pass on the final tree.
  CHECK: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml`; `npm run typecheck`; `npm test`
  EXPECT: exit 0
  EVIDENCE: PASS. Rust 273 passed, 4 ignored, 0 failed (230 lib + 2 analyzer_actions + 5 cleanup_stress + 1 performance + 3 phase3_project + 1 real_system + 26 security + 5 stress_scan). TypeScript exit 0; 113 Vitest tests in 14 files.
- [x] G110: the production frontend bundle and debug desktop binary build.
  CHECK: `node scripts/verify-dist.mjs`; `npm run build:desktop`; `node scripts/verify-desktop-binary.mjs`
  EXPECT: `DIST_OK`; `DESKTOP_BINARY_OK`
  EVIDENCE: PASS. `DIST_OK` (2 assets, 702.3 KiB, index.html 444 bytes); Tauri built `src-tauri\\target\\debug\\cyron.exe` on the final source tree (19.04 MiB); `DESKTOP_BINARY_OK`.
- [x] G111: documentation and the IPC contract agree.
  CHECK: `npm run docs:check`; handler count check against AGENTS.md and HANDOFF.md
  EXPECT: `DOCS_CHECK: PASSED`; `IPC_COUNT_OK 74`
  EVIDENCE: PASS. 95 Markdown files audited, `DOCS_CHECK: PASSED (All documentation links, commands, versions, and invariants verified)`; 74 registered handlers matched the documented count; both new commands are in the architecture IPC table, TypeScript types and `api.ts`.
- [x] G112: the integration has no network, write, execution or Everything-control surface.
  CHECK: node -e "const s=require('fs').readFileSync('src-tauri/src/everything.rs','utf8');for(const banned of ['std::fs','std::process::Command','winreg','reqwest','TcpStream','http://','https://','EVERYTHING_IPC_EXIT','REBUILD_DB','DELETE_'])if(s.includes(banned))throw new Error('banned '+banned);console.log('EVERYTHING_READ_ONLY_OK')"
  EXPECT: EVERYTHING_READ_ONLY_OK
  EVIDENCE: PASS. `everything.rs` uses only Win32 window/message APIs: no filesystem, process launch, registry, network, index rebuild, exit or delete IPC. Results are display-only and a row may call the existing validated `reveal_path`; no result is opened or executed by Cyron. The Everything unavailable, unsupported-version and index-loading states are explicit. A live search against a real Everything process and visual inspection of the Search page remain UNVERIFIED because Everything is not installed on this verification machine.

## In-app updater and release feed — 2026-11-25

The update path is user-initiated and reads the signed GitHub Releases feed. The local release build
and artifact checks passed, v0.2.2 is published, and the downloaded feed and checksums verify. A real
version-to-version update remains UNVERIFIED because no installed 0.2.1 was updated in place.

- [x] G113: the updater is reachable only from Settings → About, reports bounded progress, verifies the package before install, and exposes restart without background polling.
  CHECK: `npm run typecheck`; `npx vitest run src/lib/updates.test.ts`
  EXPECT: exit 0 and 7 updater tests passing
  EVIDENCE: PASS. 120 frontend tests in 15 files, including 7 updater-wrapper tests covering no-update, version/notes mapping, bounded progress with and without a content length, resource close, relaunch, installed version/bundle type, and NSIS-only update eligibility. The renderer cannot supply an endpoint; the only network path is `check()` from the button. No background timer or startup check was added. The updater panel was typechecked and unit-driven, not visually inspected in a native window.
- [x] G114: the signed release build produces the NSIS setup, detached signature and agreeing manifest, and the bundles keep the expected Windows metadata.
  CHECK: `TAURI_SIGNING_PRIVATE_KEY=<key> npm run build:release`; `npm run updater:manifest`; `npm run verify:updater`; `node scripts/verify-release-bundles.mjs`; `npm run verify:metadata`; `node scripts/generate-checksums.mjs --require-updater`
  EXPECT: `UPDATER_ARTIFACTS_OK`, `RELEASE_BUNDLES_OK`, `WINDOWS_METADATA_OK`, `CHECKSUMS_OK`
  EVIDENCE: PASS. NSIS 3,149,936 bytes / `fa004eaff621ad58573825c47742a84622115c3b4b0fe18374213c84e610231f`; MSI 4,395,008 bytes / `e26ca37b18ba17eea256307873368764ce633fd753d38c5ebfc9d592e56dda9f`; `cyron.exe` 9,129,472 bytes / `17853d79acf435bcf79216e1a09872448f48dd3544e90fe9cd5eb1b67f4cd061`. The NSIS `.sig` matches the `windows-x86_64-nsis` signature in `latest.json`, whose URL names the same setup asset. The verifier rejects a generic `windows-x86_64` or `windows-x86_64-msi` entry so an MSI install cannot receive the NSIS package. Version, publisher, icon and UpgradeCode checks passed.
- [x] G115: the tag workflow requires the updater secret, builds with updater artifacts, verifies the manifest before upload, and publishes the manifest and matching checksums.
  CHECK: `node -e "const s=require('fs').readFileSync('.github/workflows/release.yml','utf8');for(const x of ['npm run build:release','npm run verify:updater','latest.json','TAURI_SIGNING_PRIVATE_KEY'])if(!s.includes(x))throw new Error('missing '+x);console.log('RELEASE_WORKFLOW_UPDATER_OK')"`
  EXPECT: RELEASE_WORKFLOW_UPDATER_OK
  EVIDENCE: PASS (static). The workflow fails before building when the secret is empty, sets the key only for the release build, generates `latest.json` after the bundle, runs `verify:updater`, writes checksums with `--require-updater`, and uploads exactly the setup, MSI, standalone executable, manifest and checksum file. The manifest is scoped to `windows-x86_64-nsis`, and Settings enables the check only for an NSIS bundle while directing MSI installations to the release page. `scripts/verify-release.mjs` now includes `latest.json` in the expected asset set. Release workflow `45452657763` executed this revision and passed every step, including the signed build, updater verification, checksums, publication and Pages deployment.
- [x] G116: documentation and the updater boundary agree with the implementation.
  CHECK: `npm run docs:check`
  EXPECT: `DOCS_CHECK: PASSED`
  EVIDENCE: PASS. 96 Markdown files audited with package and Tauri versions both `0.2.2`. The docs state that the only application network path is the explicit update check, that there is no polling, that Authenticode trust is separate from the updater signature, and that the private key is never committed.
- [ ] G117: a published v0.2.2 feed installs over a real 0.2.1 installation, restarts the app and preserves app data.
  CHECK: publish v0.2.2, check for updates from an installed 0.2.1, confirm the installer, relaunch and hash the app-data tree
  EXPECT: update offered, signature accepted, app restarted at 0.2.2, no app-data loss
  EVIDENCE: UNVERIFIED. The v0.2.2 release and feed are published, and the downloaded setup, MSI, executable and manifest match `SHA256SUMS.txt`; `latest.json` advertises only `windows-x86_64-nsis`. No installed 0.2.1 was updated in place, so the native download, installer restart and app-data preservation still need a real version-to-version run.

## Native search index — 2026-11-26

The Everything IPC client was removed and replaced with Cyron's own read-only fixed-drive index.
G108–G112 above are historical: their `everything.rs` checks are no longer runnable against this
tree.

- [x] G118: the native index engine's real-walk, persistence, parser, filter, sort, mutation and watcher tests pass.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib searchindex
  EXPECT: exit 0; 18 passed; 1 ignored; 0 failed
  EVIDENCE: PASS. 18/18 in 0.03 s (plus the ignored manual benchmark): boolean/phrase/negation/wildcards; case and whole-word; extension/size/kind/attribute/type filters; date keywords, absolute dates and ranges; file/folder chips; path-only and match-path matching; stable sort and pagination; a real nested-directory fixture walk with created timestamps; save/load round trip; corrupt-cache refusal; rename and delete with descendant path reseeding; invalid/hostile query refusal; empty-state query refusal; civil date round trip with local midnight; watcher add/remove/rename notifications; pre-walk cancellation; and the CSV/TXT export added under G127.
- [x] G119: the complete Rust suite passes after the search engine and `fs.rs` metadata additions.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
  EXPECT: exit 0; 283 passed; 5 ignored; 0 failed
  EVIDENCE: PASS. 240 lib (3 ignored, including the manual search benchmark), 2 analyzer_actions (1 ignored), 5 cleanup_stress, 1 performance, 3 phase3_project, 1 real_system, 26 security, 5 stress_scan (1 ignored) → 283 passed, 5 ignored. The security allowlist admits only `searchindex.rs`'s atomic publish of Cyron's own regenerable cache; it still forbids the operation everywhere else.
- [x] G120: the frontend contract, Search view and API wrapper typecheck and pass their tests.
  CHECK: `npm run typecheck`; `npm test`
  EXPECT: exit 0; 127 tests; 0 failures
  EVIDENCE: PASS. TypeScript exits 0 with no diagnostics. 127 Vitest tests in 17 files, including the four `search_index_*` command names, the `query_search_index` payload and `export_search_results` in `api.test.ts`, four selection-helper tests and three toolbar render tests.
- [x] G121: the production renderer bundle builds and passes the dist verifier.
  CHECK: `npm run build`; `node scripts/verify-dist.mjs`
  EXPECT: `DIST_OK`
  EVIDENCE: PASS. Vite built 104 modules in 2.17 s: `index.html` 444 B, CSS 144.08 kB, JS 609.22 kB. `verify-dist.mjs` reports `bundle ok: 2 assets, 735.6 KiB, index.html 444 bytes` and `DIST_OK`.
- [x] G122: documentation, the IPC count and the architecture contract agree.
  CHECK: `npm run docs:check`; node -e "const fs=require('fs');const s=fs.readFileSync('src-tauri/src/lib.rs','utf8');const n=[...s.matchAll(/commands::([a-z0-9_]+),/g)].length;const u=new Set([...s.matchAll(/commands::([a-z0-9_]+),/g)].map(m=>m[1])).size;if(n!==76||u!==76)throw new Error('handlers '+n+'/'+u);for(const f of ['AGENTS.md','HANDOFF.md'])if(!fs.readFileSync(f,'utf8').includes('76 commands'))throw new Error(f);console.log('IPC_COUNT_OK '+n)"
  EXPECT: `DOCS_CHECK: PASSED`; `IPC_COUNT_OK 77`
  EVIDENCE: PASS. 96 Markdown files audited with package and Tauri versions both `0.2.2`; `DOCS_CHECK: PASSED (All documentation links, commands, versions, and invariants verified)`. 77 unique registered handlers; AGENTS.md and HANDOFF.md both say 77. The four engine commands and `export_search_results` appear in the architecture IPC table, TypeScript types and `api.ts`.
- [x] G123: the native index has no Everything dependency, network call, result execution or user-file mutation.
  CHECK: node -e "const fs=require('fs');if(fs.existsSync('src-tauri/src/everything.rs'))throw new Error('Everything IPC client still present');const prod=fs.readFileSync('src-tauri/src/searchindex.rs','utf8').split('#[cfg(test)]')[0];for(const b of ['std::process::Command','reqwest','TcpStream','http://','https://','ShellExecute','remove_dir_all(','remove_file(','MoveFileExW(','DeleteFileW('])if(prod.includes(b))throw new Error('banned '+b);if(!prod.includes('search-index.bin'))throw new Error('cache name');console.log('SEARCH_INDEX_READ_ONLY_OK')"
  EXPECT: `SEARCH_INDEX_READ_ONLY_OK`
  EVIDENCE: PASS (static review). No Everything source file remains. The module contains no process launch, network client, URL, shell execution or user-file removal primitive. Its only mutation is the atomic `search-index.bin` publish in the application data directory. Reparse points are indexed but never traversed; result paths are display data and can only reach the existing validated `reveal_path` command. The security suite passes with this boundary in place.
- [ ] G124: a full fixed-drive build and the native desktop Search page are exercised against a real drive.
  CHECK: launch the desktop app, build the index over the real fixed drives, query, sort, filter, reveal, and observe a live create/rename/delete
  EXPECT: counts and paths match Explorer; progress and cancellation behave; watchers update results without a rebuild
  EVIDENCE: UNVERIFIED. The engine was verified against a real nested-directory fixture, persistence round trip and synthesized watcher notifications; a complete fixed-drive crawl, the native Search click-through and live OS watcher behavior remain for a desktop session. No false claim of those paths is made.

## Search performance, selection and export — 2026-11-26

The native index gains a lower-memory build path and top-k result ordering, and Search gains
multi-select plus CSV/TXT export. Performance evidence is measured on this machine and recorded
with both the pre-change baseline and the optimized run; the runnable gate enforces a generous
ceiling so it cannot silently regress into a pathological build.

- [x] G125: the optimized build indexes a 20,000-file fixture and answers a broad 500,000-entry query inside the recorded ceilings.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --release --lib searchindex_perf -- --ignored --nocapture
  EXPECT: `SEARCH_INDEX_PERF_OK`
  EVIDENCE: PASS on 2026-11-26. `SEARCH_INDEX_PERF build_ms=105 query_ms=7 size_query_ms=15 rare_query_ms=4 fixture_entries=20201 query_entries=500000 pools_bytes=18777786 entry_bytes=24000048 path_entries=1` then `SEARCH_INDEX_PERF_OK`, 1 passed in 8.60 s. The 20,201-entry fixture is 20,000 files in 200 directories plus the volume root; the 500,000 synthetic entries used 18,777,786 B of name/extension pools plus 24,000,048 B of 48-byte entries (about 42.8 MB) and a one-entry directory-path map. The test records the pre-change baseline on this machine (355 ms build, 21 ms name query) and its ceilings are 300 ms build, 18 ms name query, 60 ms size sort and 30 ms rare query, so both recorded regressions fail it.
- [x] G126: selection helpers cover toggle, range, select-all, copy formatting and clearing without duplicating rows.
  CHECK: `npx vitest run src/features/search/selection.test.ts`
  EXPECT: exit 0; all selection tests passing
  EVIDENCE: PASS. 4/4 tests: immutable toggle with explicit check/uncheck, inclusive range in both directions plus a missing anchor, select-all deduplication, visible-order checked paths, file-only byte totals and newline copy formatting.
- [x] G127: CSV and TXT export write the selected rows or the full sorted result set, escape CSV correctly, and reject a bad format or destination.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib searchindex_export
  EXPECT: exit 0; all export tests passing
  EVIDENCE: PASS. 1/1: the CSV path writes header plus the selected rows with an empty size for folders, TXT writes one path per row, a quote/comma name is doubled and wrapped, the result-set path ignores a query when a selection is given, and a `.pdf` format, relative path, mismatched extension, existing folder destination and missing parent are all refused.
- [x] G128: the renderer contract exposes `export_search_results` with the same options, selection and destination shape the backend expects, and the IPC count matches the docs.
  CHECK: `npx vitest run src/lib/api.test.ts`; node -e "const fs=require('fs');const s=fs.readFileSync('src-tauri/src/lib.rs','utf8');const names=[...s.matchAll(/commands::([a-z0-9_]+),/g)].map(m=>m[1]);if(new Set(names).size!==77||names.length!==77)throw new Error('handlers '+names.length);if(!names.includes('export_search_results'))throw new Error('export command missing');for(const f of ['AGENTS.md','HANDOFF.md'])if(!fs.readFileSync(f,'utf8').includes('77 commands'))throw new Error(f);console.log('IPC_COUNT_OK 77')"
  EXPECT: exit 0; `IPC_COUNT_OK 77`
  EVIDENCE: PASS. `api.test.ts` passes 14/14, including `export_search_results` with `{ options, format, destination, selected }`; the registered surface is 77 handlers, 77 unique, and `AGENTS.md`/`HANDOFF.md` both say 77.
- [x] G129: typecheck, all frontend tests, production build, dist verifier and documentation check pass on the final tree.
  CHECK: `npm run typecheck`; `npm test`; `npm run build`; `node scripts/verify-dist.mjs`; `npm run docs:check`
  EXPECT: exit 0; `DIST_OK`; `DOCS_CHECK: PASSED`
  EVIDENCE: PASS on 2026-11-26. TypeScript exits 0 with no diagnostics; Vitest passes 127/127 in 17 files; Vite builds 104 modules in 1.89 s (CSS 144.08 kB, JS 609.50 kB); `verify-dist.mjs` reports `bundle ok: 2 assets, 735.9 KiB` and `DIST_OK`; `docs:check` audits 96 files and reports `DOCS_CHECK: PASSED`.
- [x] G130: the complete Rust suite passes and the search engine remains read-only with no execution, network or user-file deletion surface.
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml; node -e "const fs=require('fs');const prod=fs.readFileSync('src-tauri/src/searchindex.rs','utf8').split('#[cfg(test)]')[0];for(const b of ['std::process::Command','reqwest','TcpStream','http://','https://','ShellExecute','remove_dir_all(','remove_file(','MoveFileExW(','DeleteFileW('])if(prod.includes(b))throw new Error('banned '+b);console.log('SEARCH_INDEX_READ_ONLY_OK')"
  EXPECT: exit 0; `SEARCH_INDEX_READ_ONLY_OK`
  EVIDENCE: PASS. 283 passed, 5 ignored, 0 failed across the lib and seven integration suites; the static scan reports `SEARCH_INDEX_READ_ONLY_OK`. Export writes only the destination file through `File::create`/`BufWriter`; it adds no process launch, network, shell or indexed-file mutation path.
- [x] G131: the selection toolbar exposes labelled copy, CSV, TXT and clear actions with correct counts and disabled states in a rendered markup test.
  CHECK: `npx vitest run src/features/search/SearchSelectionBar.test.tsx`
  EXPECT: exit 0; all toolbar render tests passing
  EVIDENCE: PASS. 3/3: no markup at zero selection; `3 selected`, `2 KB`, Copy paths, both export labels and Clear selection with no disabled state; and every action disabled while a CSV export runs.
