# Cyron — Active Handoff

Updated: 2026-11-18 (v0.2.0-rc.1 published as an unsigned GitHub prerelease after a green release workflow; 0.2.0 version bump, release notes and NSIS install/launch/uninstall/reinstall verification; Phase 6-8 feature commits and audit; Phase 7 real desktop E2E, Phase 6 workflow UX, Phase 5, Phase 4, Phase 3, Phase 2 and Phase 1 records preserved)

## 1. Current Verified State

- **Branch:** `main`. Phase 5 is committed as `2eeb91f` with documentation follow-up `3d9f2da`. The Phase 8 safety hardening is `18cd4df`, the Phase 7 desktop E2E harness is `bd0de9d`, and the Phase 6-8 audit/evidence is `6c8defa`; release prep is `870d753` and the checksum-workflow fix is `ca51baf`. No rebase was made.
- **Phase 8 verdict:** **NOT PRODUCTION READY — production release remains blocked.** The unsigned 0.2.0-rc.1 testing prerelease is published; the trust and native-path blockers remain open. The full audit, evidence and blockers are in [docs/RELEASE_AUDIT_2026-11-17.md](docs/RELEASE_AUDIT_2026-11-17.md). The NSIS installer path is verified end to end; signing trust and real vendor/MSI/Store removal are the remaining release blockers.
- **Version:** `0.2.0`. `v0.2.0-rc.1` was published on 2026-11-18 as an unsigned GitHub prerelease ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.0-rc.1)) after release workflow `44146575399` passed. It is not a trusted production release. The previous `v0.1.0` assets remain `NotSigned` and predate this tree.
- **IPC surface:** 72 commands registered in `src-tauri/src/lib.rs` (including Phase 2
  `list_app_history`, `prepare_store_uninstall`, `execute_store_uninstall`, Phase 3
  `scan_developer_storage`, `preview_developer_cleanup`, `verify_developer_cleanup`, and Phase 4
  `capture_performance_snapshot`, `list_performance_baselines`, `record_performance_baseline`,
  `start_performance_trace`, `stop_performance_trace`, `open_performance_settings`), documented in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), and wrapped in `src/lib/api.ts`. The current surface
  is checked against its handlers by the verification commands below.
- **Code signing:** the published `v0.1.0` NSIS installer and the local release artifacts are
  `NotSigned`. The `CN=Chryypt Organization` certificate is self-signed (Subject == Issuer), so a
  locally signed build is cryptographically intact but not publicly trusted and earns no SmartScreen
  reputation. See [docs/CODE_SIGNING_POLICY.md](docs/CODE_SIGNING_POLICY.md).
- **CI:** `main` run `44150254690` is green: typecheck, 100 Vitest tests, `docs:check`, the Rust suite and the cleanup/security fixture scripts. Release run `44146575399` is green and published the prerelease assets. The earlier run `43200921453` failed only `docs:check` because `HANDOFF.md` linked into gitignored `.unlazy/`; that link now points to the repository-local
  [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md), and
  `scripts/verify-docs.mjs` rejects links into gitignored directories.
- **Release bundles:** `npm run build:unsigned` produced NSIS `2,270,729` bytes / `705b6378…e616`, MSI `3,260,416` bytes / `313a089f…fe57`, and `cyron.exe` `6,918,144` bytes / `ca78b575…ebfa`. Metadata audit passed; signature verification failed because all three artifacts are unsigned. The NSIS package installed (1.56 s), launched (37.1 MB, exit 0), uninstalled (1.25 s) and reinstalled; the 60,024-file app-data tree was unchanged. See [the 0.2.0 receipt](docs/evidence/2026-11-18-release-0.2.0.md). The published prerelease was rebuilt by CI from tag `v0.2.0-rc.1`; its downloaded `SHA256SUMS.txt` verified all three assets. Post-release commit `ca51baf` fixes future checksum naming and line endings, and the published checksum asset was replaced with the corrected file.

## 2. Phase 2 implementation state — 2026-11-16

The Phase 2 source changes are uncommitted. They extend the existing safety model; none of them weaken server-held plans, quarantine, protected paths or the no-shell rule.

- **Store/MSIX:** `store.rs` now reads current-user main packages through `PackageManager` and removes one reviewed package through `RemovePackageAsync(...).join()`. It never calls `RemoveForAllUsers`, deprovisions, elevates automatically or launches a shell. Access-denied, in-use and not-found outcomes are classified. Plans bind full name, family name and publisher ID; execution revalidates all three and checks the live inventory afterward.
- **Lifecycle history:** `app_history.rs` persists `<app_data_dir>/app-history.json` atomically, newest activity first, capped at 2,000 records. A partial inventory can refresh current apps but can never mark a missing app removed. Records are descriptive and never authorize cleanup.
- **Completion semantics:** `uninstall.rs` classifies 1605/1614 as not-installed, 1602 as cancelled, 3010/1641 as restart-required, and never treats exit 0 alone as removal. A bounded post-exit inventory poll reconciles single and batch runs; incomplete inventory or remaining registration stays `remaining`.
- **Discovery accuracy:** `apps.rs` detects validated MSI product codes in commands or GUID-named uninstall subkeys and reads PE `ProductName`/`CompanyName`/`FileVersion` metadata for portable executables.
- **Remnant evidence:** name-only file, shortcut and registry matches are `caution` and unselected; exact install-location or exact registration identity is the only `safe` preselection. An incomplete inventory blocks removed-app classification, and history records never authorize cleanup.
- **Quarantine:** cleanup and uninstaller-residual moves use `MoveFileExW` with `MOVEFILE_COPY_ALLOWED` for cross-volume sources and without `MOVEFILE_REPLACE_EXISTING`, so existing destinations are never overwritten. A failed move removes its write-ahead journal entry and leaves the source in place.
- **Removed-app history and explicit scans:** `list_removed_apps` merges retained identity snapshots with persistent removal history, and `scan_named_remnants` reviews apps removed before Cyron using an explicit name, publisher or path. Every explicit result is caution and unselected. The Cleaner lists persistent history and provides the explicit form.
- **UI:** the uninstall wizard has a Store mode that reviews the exact package identity before invoking Windows, reports permission denial truthfully, and offers Settings/winget fallback. Store rows and the app drawer open that mode. The UI path is typechecked but has not been driven in a native desktop session.

UNVERIFIED in this tree: a real Store package removal, a real MSI uninstall with 1605/1614 reconciliation, and the Store wizard/history UI in a native desktop session. Run the Phase 2 gates in `scripts/verify-phase2.mjs`; `--live-store`, `--live-msi` and `--manual-ui` deliberately fail with UNVERIFIED until those checks are actually performed. The Phase 2 verification run is recorded in `GATES.md` G67–G73; the Phase 1 table below remains historical.

## 2b. Phase 3 implementation state — 2026-11-16

Phase 3 adds an evidence-based developer storage path without changing the review-first cleanup engine or the no-rebuild/no-force policy.

- **Project scanner:** `devscan.rs` recognises projects from real manifests, Git roots and package/Cargo/Python framework metadata. It scans explicit folders plus bounded automatic candidates on selected fixed/removable drives, skips reparse points and dependency/build trees, and caps roots, directories, projects and artifacts.
- **Artifacts:** only named regenerable folders become candidates: `node_modules`, `dist`, `build`, Rust/Maven `target`, Gradle `build`/`.gradle`, `.next/cache`, `.turbo`, `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`. Each record has exact path/ownership evidence, confidence (a lockfile raises `node_modules` confidence), logical/allocated bytes, counts, last write, activity, rebuild command and loss copy. Source, `.git`, `.env`, credentials, keys, databases and model weights are reported as protected; they never enter a plan.
- **Id-only preview:** `scan_developer_storage` keeps a 1-hour server-held scan. `preview_developer_cleanup` resolves selected artifact ids, refuses unknown/protected artifacts and any path outside its recorded project, then reuses `guard_path`, snapshot binding, journaled quarantine and the single-use `CleanupPlan`.
- **Verification:** `verify_developer_cleanup` re-measures selected artifacts, reports removed/remaining bytes and files, and lists quarantine batches that still hold matching targets. The Projects tab re-scans after a run and shows the result; it does not claim success from exit status alone.
- **UI:** Cleaner has a **Projects** tab with project/activity filtering, per-artifact evidence and loss/rebuild details, protected components, selective checkboxes, exact-target preview, confirmation, result items and verification. Project roots are persisted in `cyron.settings.v1`. Canonical `\\?\` prefixes are hidden from display only, and opening Quarantine refreshes it so a just-finished run cannot look empty.
- **Catalog safety corrections:** Gemini CLI `tmp` was reclassified `protected` because its chats, shell history and checkpoints live there; Cursor and VS Code `workspaceStorage` were reclassified `protected` because they contain chat/extension state; LM Studio current paths, Hugging Face, Antigravity agent data and Freebuff settings were added as protected. Tool cache rows now carry `installState`; only `not-detected` is shown as orphaned and nothing is auto-selected.

### Phase 3 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 98 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (514.16 kB JS, 101.42 kB CSS; Vite chunk-size warning only) |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 244 passed, 4 ignored, 0 failed |
| Phase 3 disposable project | `cargo test --test phase3_project` | 3 passed: scoped quarantine and restore; `.next/cache` cleaned without touching `.next/server` or `BUILD_ID`; unknown artifact refused |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 76 files |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| RC1 E2E fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK`; all 8 scenarios, after raising the disposable GUI window and polling the hit-test |
| Native Projects workflow | UI Automation against the Tauri debug app (Vite dev server) | PASS: discovered a disposable project, previewed 1 file, quarantined it, verified `1 removed / 0 remaining`, restored it, and found a second disposable project on E:`\Projects`. Source, `.env`, `.git` and `dist` stayed. Screenshots were reviewed locally; raw captures are not committed per [docs/evidence/README.md](docs/evidence/README.md). |
| Native multi-drive discovery | UI Automation against the Tauri debug app (Vite dev server) | PASS: 2 project roots (`Documents\GitHub` on C: and `E:\Projects`) and the E: disposable project appeared in the running window. |

The native run found two UI defects that are now fixed: canonical paths displayed as `\\?\C:\...` (display-only stripping; operations keep the canonical path) and Quarantine showing a stale "empty" after a Projects cleanup (the tab now refreshes on open). The fixtures were removed after the run. UNVERIFIED remains: live `hf cache prune`/`cargo clean`/package-manager commands are guidance only and are never executed by Cyron.

## 2c. Performance diagnostics implementation state — 2026-11-17

Performance diagnostics are read-only sampling plus one existing reversible startup path; no system tuning was added.

- **Metrics:** `perf.rs` measures a 700 ms system-CPU window, memory and commit, PDH physical-disk busy/queue, fixed-volume capacity, and bounded per-process CPU/working-set/read/write counters. Processes whose handles cannot be opened are counted and omitted, not guessed. Cyron's own process and protected/system processes are labeled.
- **Startup and tasks:** HKCU/HKLM Run/RunOnce, WOW6432Node and Startup folders are listed with exact command, location, state, live usage and protected flags. Boot/logon scheduled tasks are read-only and include author, trigger, delay and Microsoft-task provenance. Per-app boot cost is read only from the administrator-only Diagnostics-Performance event log and matched by exact executable path; otherwise it is **Unavailable**.
- **Reversible toggle:** the page calls the existing `set_startup_item_enabled`, which writes `StartupApproved` 0x02/0x03, preserves the Run registration and refuses protected or machine-wide entries when unelevated. A native Tauri dialog now confirms the exact entry and location. This replaced `window.confirm`, which produced no visible dialog in WebView2.
- **Recommendations:** every recommendation requires a measured threshold and carries evidence, expected effect, risk and verification. Per-process I/O is not treated as disk pressure because Windows includes network, pipe and device transfers; the native run exposed that exact false positive and the gate is now physical-disk busy + queue only.
- **Guides, baselines and trace:** five symptom guides filter recommendations; up to 50 labeled local baselines compare CPU, memory, available memory, commit, disk busy and recorded boot time with an association-not-causation note; the optional WPR trace invokes only `%SystemRoot%\System32\wpr.exe` with fixed arguments, requires administrator rights, stays local and never self-elevates.
- **Freshness:** manual **Refresh** calls `capture_performance_snapshot(forceStartup = true)` to bypass the 120-second startup-evidence cache; live sampling keeps the cache.

### Performance diagnostics verification (this tree)

| Check | Command | Observed |
|---|---|---|
| Targeted unit tests | `cargo test --lib perf::tests` | 8 passed, including the idle-disk/high-process-I/O regression and fixed System32 path validation |
| Live snapshot | `cargo test --test performance -- --nocapture` | passed on this machine |
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 98 passed in 12 files |
| Full Rust suite | `cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED` |
| Native Performance workflow | UIA against the native Tauri debug app | PASS on 2026-11-17: 702 ms sample / 179 processes, 15% CPU, 37% memory, 1.1% disk busy; guide switching; 23 startup entries; disposable startup Cancel/Disable/Enable round trip with 0x03→0x02 bytes; two baselines compared; trace correctly disabled unelevated. See [the receipt](docs/evidence/2026-11-17-performance-diagnostics.md). |

The native run found and fixed two defects: the manual Refresh was served stale startup evidence for two minutes, and a disk recommendation fired from process I/O while the physical disk was idle. UNVERIFIED remains: an elevated Diagnostics-Performance boot-impact reading and a real WPR trace recording/opening.

## 2d. Design-system and navigation rebuild — 2026-11-17

Phase 5 replaces the old dashboard surface with a calmer Windows-native utility shell. Backend safety plans, quarantine and IPC payloads are unchanged; this is presentation and information architecture only.

- **Navigation:** exactly five destinations — Home, Applications, Storage & Cleanup, Performance, History & Recovery. Settings sits in the sidebar footer. Hunter Mode is **Find by window** inside Applications; there is no separate Hunter page. Storage & Cleanup contains Cleanup and Disk space; History & Recovery contains Activity and Recovery.
- **Design system:** `src/styles/tokens.css` now defines one neutral family, one blue accent, one type hierarchy, one radius scale and light/dark surfaces. `src/components/ui.tsx` provides the shared primitives (`ViewHeader`, `SectionHeader`, `PageTabs`, `Panel`, `Stat`, `Badge`, `Button`, `SearchField`, `Segmented`, `Toggle`, `Select`, `Field`, `ProgressBar`, `Skeleton`, `EmptyState`, `Banner`, `Drawer`, `Modal`, `KeyValue`). Feature CSS never defines a second palette.
- **New feature shells:** `src/features/storage/StorageCleanup.tsx` hosts Cleanup and Disk space; `src/features/history/HistoryRecovery.tsx` hosts Activity and Recovery. Existing Cleaner, Analyzer, Uninstaller, Overview and Performance views were restyled into those destinations rather than duplicated.
- **States and responsive behaviour:** loading skeletons match final layout; empty/error/stale/disabled states exist for scans, history, quarantine, projects and performance; tables hide secondary columns at 1000 px; actions and headings never collapse below usable widths.
- **Accessibility:** visible `:focus-visible` rings, 32 px controls, dialog focus trap and Escape return, `aria-current` navigation, labelled icon-only controls, reduced-motion support and a forced-colors block. A `prefers-reduced-motion` emulation and native keyboard traversal were exercised; screen-reader software and forced-colors visuals were not.
- **Native audit fixes:** the Projects tab no longer inherits the Caches & leftovers header or cache warnings; the Recovery log badge now fits its grid at 1000 px; the Activity table hides Type at 1000 px instead of scrolling; the Analyzer review strips `\\?\` from display paths; legacy `Cleanup · 1 locations` titles and `1 files permanently removed` copy were corrected.
- **Evidence:** selected native captures and the full command ledger are in [docs/evidence/2026-11-17-design-system.md](docs/evidence/2026-11-17-design-system.md). Design rules are in [docs/DESIGN.md](docs/DESIGN.md).

### Phase 5 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (562.28 kB JS, 127.63 kB CSS; chunk-size warning only) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 79 Markdown files |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; debug executable built |
| Native navigation and keyboard | WebView2 remote debugging against the debug app | PASS: five destinations; Settings footer; Hunter in Applications; focus trap; Escape return; reduced motion; 0 horizontal overflow at 1000x680 |
| Contrast | WCAG token calculation | PASS: sampled text pairs 4.57:1–16.14:1 |

UNVERIFIED in this tree: screen-reader traversal, forced-colors visuals, 200% OS text scaling and a clean-VM installer run. The screenshots show this machine's real 179-application inventory; no real user cache or project was cleaned.

## 2e. Workflow UX completion — 2026-11-17

Phase 6 applied the Phase 5 system to every workflow and fixed the state and layout gaps found in the production desktop build. No Rust safety plan, quarantine rule or IPC permission changed.

- **Home:** real system, volume, application, cleanup and recent-activity measurements; no health score. At 1000 px the five-item system summary uses two columns with Uptime spanning the spare row instead of leaving a blank cell.
- **Cleaner:** the first measurement now says **Measuring known cache folders on C:, D:, E:…**, disables the redundant scan action while it runs, and shows **Not checked yet** before results. Opening a preview clears the previous preview, shows a spinner, explains that nothing moves, and labels dismissal **Cancel preview**.
- **Storage & Cleanup:** the header now says **Review before cleanup** instead of the internal read-only phrasing.
- **History & Recovery:** at 1000 px the cleanup log keeps each outcome under its operation title instead of hiding failed and partial runs. A stop or failure that already moved files is recorded as **partial**, not completed or a bare cancellation; Activity, Home and the Recovery log render the fourth state.
- **Settings:** the startup section explains that disabling an entry preserves it and can be reversed; Copy now confirms **Copied** for two seconds.
- **Shell:** `html` is fixed so a stray focus or programmatic scroll cannot shift the whole window onto blank space. Each view slot remains the only scroll container.
- **Evidence:** native captures at 1320x860 and 1000x680, light/dark, and the full command ledger are in [docs/evidence/2026-11-17-workflow-ux.md](docs/evidence/2026-11-17-workflow-ux.md).

### Phase 6 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (563.72 kB JS, 128.05 kB CSS; chunk-size warning only) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 80 Markdown files |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; debug executable built |
| Native layout probe | WebView2 evaluation at 984 px client width | 0 horizontal overflow on all five destinations and Settings; document scroll height stayed 641 px |
| Native workflow review | WebView2 against the production desktop build | All five destinations, Settings, Hunter dialog, cleanup preview and Recovery log opened; no dead route or placeholder control |

UNVERIFIED in this tree: real cache/project/application mutation, screen-reader traversal, forced-colors visuals, 200% text scaling and a clean-VM installer run. The per-item destructive paths remain covered by fixtures and the Rust suite.

## 2g. Real desktop E2E — 2026-11-17

Phase 7 adds a real Windows desktop acceptance harness. It does not weaken or change any production
safety plan; it drives the compiled Tauri window through WebView2/WebDriver against disposable
HKCU, project and cache fixtures.

- **Harness:** `npm run test:desktop` builds `desktop_e2e_fixture`, starts `tauri-driver` with the
  matching `msedgedriver`, runs WebdriverIO through `@wdio/tauri-service`, restores app data and
  removes fixture keys. Named screenshots, `results.json` and the exact revision are written under
  `docs/evidence/phase7/`; the full run log stays in gitignored `test-results/desktop/run.log`.
- **Journeys:** first launch and every destination; inventory search/filter/sort/select; batch and
  single uninstall; broken-entry registry backup/delete/restore; 1000×680 and 1440×960 light/dark
  overflow checks; Settings theme persistence across reload; Storage disk-space state; Performance
  evidence; startup provenance; History records; Hunter confirmation/targeting/Escape.
- **Real IPC:** no operation result is mocked. Fixture uninstallers are executed, HKCU keys are read
  back independently, and the registry restore is asserted from the Windows registry.
- **Fixes:** display-scaled pointer coordinates, stale HKCU fixture residue, a malformed fixture
  uninstaller command, redirected-profile WebView2 startup and an invalid UIA condition were fixed in
  the harness.

### Phase 7 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| Real desktop suite | `npm run test:desktop` | `DESKTOP_E2E_OK`; 10 journeys passed, 0 failed |
| Desktop harness build | built automatically by the suite | `desktop_e2e_fixture` example compiled with MSVC |
| Tested revision | `git rev-parse HEAD` | `3d9f2daddb527492afc578c43dc70c1a7a7ce6f2` plus Phase 7 working tree |
| Evidence | `docs/evidence/2026-11-17-desktop-e2e.md` | named captures, `results.json`, `revision.json` and explicit limitations |

UNVERIFIED by this harness: 125%/150% OS scaling, the native startup `ask` dialog under WebDriver,
real cache cleanup and quarantine execution, AI/project cleanup, duplicate/treemap interaction,
Explorer verbs and Store/MSI removal. The dedicated UIA helper exists, but those native/external
windows were not completed in this run. See the receipt for the exact list. Installer/relaunch was
closed separately in the Phase 8 packaging pass below.

## 2h. Phase 8 independent audit, packaging and installer verification — 2026-11-17

The final audit compiled and packaged the current tree, then exercised the real per-user installer.
The packaging pass added no application behavior; the audit's separately tested safety fixes are
listed in [the release audit](docs/RELEASE_AUDIT_2026-11-17.md#defects-found-and-fixed-in-this-audit).
Signing and real application removal remain release blockers.

- **Suites:** 100 frontend tests in 12 files, typecheck, `DOCS_CHECK`, 261 Rust tests (4 ignored),
  cleanup and 8-scenario RC1 fixtures, Hunter benchmark, and 10/10 real WebView2 desktop journeys.
- **Bundles:** `build:unsigned` exit 0; `RELEASE_BUNDLES_OK`; `WINDOWS_METADATA_OK`; all three
  artifacts `NotSigned` (`verify:signatures` exits 1 by design for this RC). The local Chryypt
  certificate chain is `UntrustedRoot`. The published `v0.1.0` NSIS hash matches its
  `SHA256SUMS.txt` but the asset is `NotSigned`.
- **Installer:** NSIS install 1.07 s, repair 1.02 s, uninstall 1.24 s; install directory and HKCU
  uninstall entry created and removed correctly; installed app launched, responded and exited 0;
  reinstall after uninstall succeeded; 0 app-data hash changes and quarantine sentinels preserved.
- **MSI:** unelevated `msiexec /qn` returned 1603 / error 1925. The per-machine package needs an
  elevated install test; it did not install anything unelevated.
- **Evidence:** [installer receipt](docs/evidence/2026-11-17-installer-release.md),
  [installed-app capture](docs/evidence/phase8/34-installed-launch.png), and
  [final matrix](docs/RELEASE_AUDIT_2026-11-17.md#final-pass-fail-unverified-feature-matrix).

## 3. Verification — Phase 1 tree, 2026-11-16

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0, 0 diagnostics |
| Frontend tests | `npm test` | 92 passed in 11 files |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 213 passed, 4 ignored (174 library, 26 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress, 2 analyzer-actions) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 74 Markdown files |
| Production frontend | `npm run build` | exit 0 |
| Desktop debug build | `npm run build:desktop` | `src-tauri/target/debug/cyron.exe` built |
| Native Force Delete → restore | UIA-driven desktop session | disposable folder on E: quarantined into `%APPDATA%` on C:, listed in Cleaner > Quarantine, restored to E: through the UI with contents intact |
| Native Force Delete pre-fix run | UIA-driven desktop session | cross-volume folder restore failed at publish (`os error 80`); fixed by the staging path |

Not run for this tree: installer bundles (`build:unsigned`), signing and negative signature checks,
metadata audit, `verify:dist`, `verify:cleanup`, `verify:e2e`, Hunter benchmark. Real vendor
uninstallation, Store removal, real user-cache mutation, screen-reader traversal, and a clean-VM
installer test remain unexercised.

## 4. Release blockers resolved in the Phase 1 audit

1. **CI documentation gate:** links into `.unlazy/`, `tmp/`, and other gitignored directories now fail
   with a clear message instead of passing until a clean clone breaks. Verified with a temporary
   negative link that exited 1.
2. **Explorer Force Delete quarantine:** the batch had no write-ahead journal, stored only a filename
   in its manifest (so restore and purge could not find it), and fell through to permanent deletion
   when a quarantine rename failed. It now journals before the move, stores the full path, refuses
   unknown modes, reports partial results truthfully, and is covered by
   `explorer::tests::quarantine_writes_journal_and_restores_a_folder` and
   `purge_removes_a_quarantined_folder_entry`.
3. **Cross-volume folder restore:** `restore_batch` failed to publish a directory from another volume.
   It now copies into a same-volume staging folder, publishes with a no-overwrite rename, and only
   then removes the quarantined copy. Tested by `cleanup::tests` and the native run above.
4. **Process termination identity check:** a plan whose expected creation time was 0 silently skipped
   PID-recycling verification. It now refuses such plans, and Kill Process plans omit processes whose
   creation time cannot be read.
5. **Dead Hunter widget setting:** `showHunterWidget` was persisted and toggleable but no desktop
   widget exists. The setting was removed; Settings now explains that Hunter Mode is a confirmed
   session action.
6. **Documentation drift:** IPC counts (34/51) and test counts were stale, the Architecture IPC table
   named commands that do not exist, signing was described as trusted, and a limitation claimed
   cross-volume folder restore was unsupported. All corrected against the tree.

## 5. Prioritized backlog

### P0 — release blockers

1. **Resolve release signing trust.** `v0.1.0` is unsigned and the local certificate is self-signed.
   Either obtain a publicly trusted Authenticode certificate or label every artifact explicitly as
   unsigned/self-signed in release notes. Files: `docs/BUILD_AND_SIGNING.md`,
   `docs/CODE_SIGNING_POLICY.md`, `scripts/sign-build.ps1`, `.github/workflows/release.yml`.
2. **Run an elevated MSI and clean-VM/version-upgrade test.** The NSIS current-user install,
   repair and uninstall path passed on this machine, but the WiX MSI is per-machine and returned
   error 1925 without elevation. Test the MSI elevated, then a version-to-version upgrade and a
   clean Windows VM install. Files: `src-tauri/tauri.conf.json`, `docs/RELEASE_CHECKLIST.md`.
3. **Verify native Store and MSI removal end to end.** Use a disposable Windows profile and a
   disposable MSI package. Confirm the Store plan revalidates identity, `RemovePackageAsync`
   reports truthfully, a denied current-user removal is not escalated, and 1605/1614 do not become
   `completed` without inventory evidence. These are UNVERIFIED today.
4. **Verify the elevated performance paths.** Run one session as administrator to read the real
   Diagnostics-Performance boot impact and record/open a short WPR trace. The unelevated UI was
   verified to be honest and disabled; those two elevated paths are UNVERIFIED. Files: `perf.rs`,
   `src/features/performance/Performance.tsx`.

### P1 — correctness and coverage

1. **Reparse-point refusal lacks a fixture.** Cross-volume restore now refuses a quarantined tree that
   contains a junction or symlink and leaves the batch intact. Add a fixture that creates a junction
   and asserts the refusal; creating one may need developer mode, so gate it or use a fixture helper.
2. **Store removal UI is UNVERIFIED.** The native plan/execute commands and the wizard's Store
   mode exist, but the desktop flow has not been exercised against a real package. Treat
   `--live-store` and `--manual-ui` as open release gates.
3. **Store removal remains current-user only.** Machine-wide and provisioned packages are
   intentionally not attempted; the UI directs those users to Windows Settings or an
   administrator-managed tool.
4. **Provider catalog has no UI consumer.** `list_cleanup_providers` is registered and tested but no
   view lists provider definitions. Build the provider browser or stop advertising it.
5. **Native scheduled-task and service remnants are not discovered.** `leftovers.rs` limits
   destructive candidates to files, shortcuts and registry keys, and explicitly does not infer
   service or task ownership from a name. Add read-only discovery and reviewed ownership evidence
   before offering any cleanup there.
6. **Native desktop flows are manual.** Force Delete quarantine/restore, UAC presentation, and
   Explorer verbs have no automated Tauri desktop test. Keep them as recorded manual gates until a
   WebDriver harness exists.
7. **Real-system release validation.** Vendor uninstallers, Store apps, real caches and real project
   folders have not been exercised end to end. Use a disposable Windows VM; the Recycle Bin
   interactive fixture is ignored by default and must be invoked explicitly.
8. **Performance sampling is not a root-cause engine.** A 700 ms sample cannot prove causation, per-process I/O combines network and device traffic, and elevated boot impact/WPR tracing are unverified. Reproduce the lag, use the comparison honestly, and do not treat "old" or "large" as permission to remove anything. Files: `perf.rs`.
9. **Project discovery coverage is bounded.** Automatic roots, 60,000-directory/400-project caps,
   pnpm per-drive stores, custom `UV_CACHE_DIR`/`PIP_CACHE_DIR`/`HF_HOME`/`OLLAMA_MODELS` and Yarn
   PnP/Zero-Installs are not discovered. Add explicit roots or use the owning tool's prune command;
   do not infer ownership from a name.

### P2 — maintainability and small correctness

1. **Dead scan-index IPC.** `query_directory_children`, `search_indexed_files`, and
   `get_scan_index_stats` are registered and populated but unused by any view. Build the tree/search
   consumer or drop the commands, index build, and client wrappers together.
2. **Cleaner is 1580 lines plus the Projects tab component.** Scan setup, selection, quarantine,
   log, and dialogs still share one file; `DeveloperStorage.tsx` is a separate bounded feature.
   Extract `QuarantinePanel`, the scan header, and the log panel into feature files. No behavior
   change is needed.
3. **Settings is 740 lines; `App.tsx` is 428.** Extract the General, Scanning, Startup, Explorer, and
   Safety tabs into components to reduce merge risk and make settings wiring auditable.
4. **Persisted settings residue.** Old profiles still contain `showHunterWidget` in
   `cyron.settings.v1`; it is ignored. No migration is required unless the schema version changes.

## 6. Next step

The combined tree now passes typecheck, 100 frontend tests, 261 Rust tests, documentation checks, the
production frontend build, ten real desktop journeys, and the NSIS install/repair/uninstall/reinstall
path. Remaining release blockers are public signing trust and the unexercised live vendor/MSI/Store
paths; elevated MSI, cross-version upgrade, 125%/150% scaling and elevated performance tracing are
still UNVERIFIED. Commit and push this tree so CI can prove the gates on a clean clone, then obtain a
publicly trusted certificate and run the remaining native release checks. Do not call Store/MSIX
removal, elevated performance tracing or the MSI install path verified until those runs are recorded.
