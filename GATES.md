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

## Not gated

- [ ] Windows CI remote run: workflow now runs the security, cleanup-stress and fixture steps;
  no GitHub Actions run has been observed from this machine.
- [ ] Release MSI/NSIS packaging and signing: not produced.
- [ ] Real cleanup of a user cache on this machine: deliberately not performed during verification;
  the engine was exercised only against isolated fixtures.
- [ ] Elevation: Cyron never requests administrator rights for cleaning; locked or system-owned
  files are reported and skipped.
