# Gates: Cyron Uninstaller Phase 3 — Safe cleaning

OWNS: src/**, src-tauri/**, scripts/**, .github/**, package.json, package-lock.json, GATES.md,
README.md, ROADMAP.md, HANDOFF.md, AGENTS.md, docs/**

Scope: a review-first cleanup engine over the fixed 48-location cache catalog — backend-enforced
eligibility, protected-path refusal, user exclusions enforced server-side, recoverable quarantine
with restore/purge, a local cleanup log, a backend read-only switch, per-item results, cancellation,
and the Cleaner UI that drives it. No uninstall, process termination, registry write, or
administrator escalation.

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
  EVIDENCE: exit 0. 105 lib tests (2 ignored), 15 security, 5 cleanup_stress, 1 real_system,
  5 stress_scan (1 ignored) → 131 passed, 3 ignored, 0 failed.

- [x] G3: frontend typecheck
  CHECK: npm run typecheck
  EXPECT: exit 0
  EVIDENCE: exit 0, no diagnostics.

- [x] G4: frontend unit tests
  CHECK: npm test
  EXPECT: exit 0
  EVIDENCE: exit 0. 70 tests in 7 files, including `cleanup.test.ts` (16) and the cleanup IPC
  contract in `api.test.ts` (9).

- [x] G5: frontend production bundle
  CHECK: node scripts/verify-dist.mjs
  EXPECT: DIST_OK
  EVIDENCE: `DIST_OK`, 2 assets, 434.3 KiB, index.html 444 B.

## Cleanup safety

- [x] G6: permission-boundary and protected-path tests pass
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security
  EXPECT: exit 0
  EVIDENCE: exit 0, 15/15. Covers: destructive calls only in reviewed modules; every catalog
  cleanup path passes the guard; protected/ancestor paths refused; hostile ids and run ids refused;
  junction not followed; links and directories never moved; restore never overwrites; purge removes
  only batch files; read-only blocks execution; hard links counted once; fixture round trip.
  NOTE: read-only gates execution only. `preview_cleanup` no longer calls `ensure_cleanup_enabled`
  because a preview reads sizes and paths and never modifies anything; `start_cleanup` still calls
  it first, and the embedded read-only visual pass in G13 exercised the preview path live.

- [x] G7: preview, execution, quarantine, restore and purge work on isolated fixtures
  CHECK: node scripts/verify-cleanup.mjs
  EXPECT: CLEANUP_FIXTURE_OK
  EVIDENCE: `CLEANUP_FIXTURE_OK`. Fixture at `%TEMP%\cyron-cleanup-<pid>`: exact preview counts and
  bytes, quarantine of 4 files, outside-file untouched, manifest written, batch listed, byte-exact
  restore, batch retired on full restore, locked file skipped while the rest move, purge accounting,
  read-only refusal.

- [x] G8: large fixtures stay bounded, cancellable and partial-failure tolerant
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress
  EXPECT: exit 0
  EVIDENCE: exit 0, 5/5 (wide fixture preview bounded while execution moves everything, locked file,
  cancelled run claims nothing it did not move, consecutive batches independent, empty/missing
  targets).

- [x] G9: protected catalog entries can never be planned or executed
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib cleanup::
  EXPECT: exit 0
  EVIDENCE: exit 0, 22/22 cleanup unit tests, including protected refusal, exclusion enforcement,
  truncated-listing reporting and both exclusion validators.

## Native behaviour

- [x] G10: debug desktop binary builds
  CHECK: npm run build:desktop
  EXPECT: exit 0
  EVIDENCE: exit 0, `Built application at: src-tauri\target\debug\cyron.exe`.

- [x] G11: built executable is recognizable as the Cyron desktop app
  CHECK: node scripts/verify-desktop-binary.mjs
  EXPECT: DESKTOP_BINARY_OK
  EVIDENCE: `DESKTOP_BINARY_OK`, 15.21 MiB.

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

## Not gated

- [ ] Windows CI remote run: workflow now runs the security, cleanup-stress and fixture steps;
  no GitHub Actions run has been observed from this machine.
- [ ] Release MSI/NSIS packaging and signing: not produced.
- [ ] Real cleanup of a user cache on this machine: deliberately not performed during verification;
  the engine was exercised only against isolated fixtures.
- [ ] Elevation: Cyron never requests administrator rights for cleaning; locked or system-owned
  files are reported and skipped.
