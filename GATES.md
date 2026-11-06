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
  EVIDENCE: exit 0. 116 lib tests (2 ignored), 21 security, 5 cleanup_stress, 1 real_system,
  5 stress_scan (1 ignored) → 148 passed, 3 ignored, 0 failed.

- [x] G3: frontend typecheck
  CHECK: npm run typecheck
  EXPECT: exit 0
  EVIDENCE: exit 0, no diagnostics.

- [x] G4: frontend unit tests
  CHECK: npm test
  EXPECT: exit 0
  EVIDENCE: exit 0. 71 tests in 7 files, including `api.test.ts` (10), `cleanup.test.ts` (16)
  and Phase 4 command contracts.

- [x] G5: frontend production bundle
  CHECK: node scripts/verify-dist.mjs
  EXPECT: DIST_OK
  EVIDENCE: `DIST_OK`, 2 assets, 448.7 KiB, index.html 444 B.

## Cleanup safety & Priority 0 Hardening

- [x] G6: permission-boundary and protected-path tests pass
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security
  EXPECT: exit 0
  EVIDENCE: exit 0, 21/21. Covers: destructive calls confined to reviewed modules (`cleanup.rs`,
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
  - NSIS: `Cyron_0.1.0_x64-setup.exe` (1.42 MiB, SHA-256: `29d884334a871c310047bc5191c9d2c2b9a1410b7450967943635c419bdcc0e0`)
  - WiX MSI: `Cyron_0.1.0_x64_en-US.msi` (2.00 MiB, SHA-256: `29453c42694a87723fe30b4827d4e9a92e52413038e9daded7592fd277271dc8`)

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
  EXPECT: PE VERSIONINFO, WiX MSI database properties, and NSIS headers match `Cyron Uninstaller`, version `0.1.0`, publisher `Ry0x7`, UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`.
  EVIDENCE: `WINDOWS_METADATA_OK` (5/5 checks passed).

- [x] G41: Local development code signing (`DEV_TEST_SIGNED`)
  CHECK: `npm run build:dev-signed` (`scripts/create-dev-cert.ps1` and `scripts/sign-dev-build.ps1`)
  EXPECT: Idempotent certificate generation/reuse in `Cert:\CurrentUser\My` (CN=Cyron Uninstaller Development, RSA 3072, SHA-256); signing of `cyron.exe`, NSIS installer, and WiX MSI via official Windows SDK `signtool.exe`.
  EVIDENCE: `DEV_TEST_SIGNED_OK` (3/3 artifacts signed).

- [x] G42: Authenticode signature integrity & negative test boundary
  CHECK: `npm run verify:signatures` (`scripts/verify-signatures.ps1 -RunNegativeTests`)
  EXPECT: Separates cryptographic digest validity from root trust; rejects unsigned binaries, tampered binaries, and rogue certificate subjects; verifies all 3 release artifacts.
  EVIDENCE: `SIGNATURES_VERIFIED_OK` (3/3 artifacts passed; all 3 negative tests passed).

- [x] G43: SignPath Foundation readiness assessment & public policy
  CHECK: `docs/SIGNPATH_READINESS.md` and `docs/CODE_SIGNING_POLICY.md`
  EXPECT: Requirement-by-requirement audit against SignPath eligibility criteria; explicit SignPath attribution; maintainer/approver responsibilities; zero-telemetry privacy policy; release verification instructions.
  EVIDENCE: Full PASS/FAIL/BLOCKED matrix documented; prerequisites explicitly scheduled.

- [x] G44: SignPath automated release workflow
  CHECK: `.github/workflows/signpath-release.yml`
  EXPECT: Disabled-by-default manual workflow dispatch; builds from exact tag; runs full test suite; records unsigned SHA-256 hashes & provenance; submits signing request via official action; verifies returned Authenticode chain; creates draft release.
  EVIDENCE: Workflow file configured with all 11 required safety gates.

- [x] G45: Git history privacy sanitization
  CHECK: Purge historical `.chryypt/` data from repository history via `git-filter-repo`.
  EXPECT: `git log --all --full-history -- ".chryypt"` returns empty.
  EVIDENCE: Executed per operator directive; `.chryypt/` completely eradicated from history.

## Not gated

- [ ] SignPath Foundation external acceptance: Cyron Uninstaller has prepared the technical integration but has not yet received external approval from SignPath Foundation; public releases prior to approval remain `UNSIGNED_RC` (or `DEV_TEST_SIGNED` for isolated test builds).
- [ ] Real cleanup of a user cache on this machine: deliberately not performed during verification;
  the engine was exercised only against isolated fixtures.
- [ ] Elevation: Cyron never requests administrator rights for cleaning; locked or system-owned
  files are reported and skipped.
