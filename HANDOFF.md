# Cyron — Active Handoff

Updated: 2026-11-06 (Phase 4 complete & Priority 0 hardening)

## Current status

Phases 1–4 are implemented across the Rust backend and React interface. Cyron discovers installed
Windows applications, scans folders with an NTFS MFT reader or a streaming directory walk, renders a
treemap with a persistent synchronized folder tree, verifies duplicates, categorizes storage,
measures 48 developer/AI/Windows cache locations, cleans caches safely via recoverable quarantine,
**uninstalls applications with exit-code tracking and reboot alerts, terminates associated processes safely,
scans leftovers with risk ratings, exports registry backups, cleans residuals into quarantine, and executes
sequential batch uninstall queues with failure isolation.**

Priority 0 cleanup recovery risks have been hardened: durable crash-consistent write-ahead recovery
journal (`recovery_journal.jsonl`), atomic Win32 `MoveFileExW` no-overwrite restore, approved preview snapshot
binding, purge preservation for safe retry on locked files, and quarantine storage & low-space limits.

Latest measured checks: typecheck clean; **71 frontend tests in 7 files pass**; **148 Rust tests
pass** (116 unit, 21 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress) with 3 ignored;
`CLEANUP_FIXTURE_OK`; `DIST_OK`; `DESKTOP_BINARY_OK`; `DESKTOP_SMOKE_OK`. No real user application was
uninstalled during automated verification — engines were exercised against isolated fixtures only.
Release MSI/NSIS packaging and signing remain unverified.

## Implemented

### Phase 1

- Tauri v2 shell with strict CSP, dialog capability, one native window and the inventory, scan,
  duplicate, measurement, reveal and history commands.
- Registry inventory: four uninstall hives, update/patch/system filtering, dedupe, install-date and
  `EstimatedSize` normalization.
- Volumes: local fixed/removable capacity and free space; network/optical skipped deliberately.
- Cache catalog: AI tools, IDEs, package managers, containers/WSL and Windows/browser locations;
  Safe / Review / Protected risk model.
- UI: Overview, Uninstaller, Disk Analyzer, Cleaner, Activity, Settings; light/dark themes.
- Local persistence: `history.json` in app-data (max 100, atomic write), settings in
  `localStorage["cyron.settings.v1"]`.
- Tooling: `scripts/msvc-env.mjs`, `verify-dist.mjs`, `verify-desktop-binary.mjs`,
  `smoke-desktop.mjs`, Windows CI, Vitest, Rust unit/integration tests.

### Phase 2

- **MFT scanner** (`mft.rs`): boot sector and `$MFT` run-list parsing, fixups,
  `FILE_NAME`/`$DATA`/`$STANDARD_INFORMATION`, subtree construction, per-record hard-link dedupe, a
  768 MiB record budget checked before allocation, cancellation every 4,096 records. Live elevated
  measurement on `C:\`: 2,211,292 files / 604,557 dirs / 332.3 GiB in 14.52 s.
- **Bounded walker** (`scan.rs`): streaming enumeration with node/child/extension/id caps,
  top-24 extensions, top-100 largest files, throttled progress and cancellation.
- **Duplicates** (`duplicates.rs`): size buckets → 4 KiB edge SHA-256 → full streaming SHA-256,
  hard links flagged, caps with a `truncated` flag.
- **Storage** (`storage.rs`): ten categories with known/inferred confidence, age buckets, insights.
- **Analyzer UI**: Map / Duplicates / Storage, persistent resizable virtualized tree, filters,
  synchronized selection, scanner-method badge, partial and stale states.
- **Benchmarks**: `scripts/bench-scan.mjs` + `examples/scan_bench.rs`, results in
  [docs/BENCHMARKS.md](docs/BENCHMARKS.md) (warm cache, one machine).

### Phase 3

- **Cleanup engine** (`src-tauri/src/cleanup.rs`, new): `catalog_specs` → `guard_path` →
  `plan_specs_with_exclusions` → server-held `CleanupPlan` → `execute_plan`. Limits: 64 ids,
  128 exclusions, 100,000 files per target, 400 listed preview files, 500 result items, 100 skips,
  20 errors, depth 64, 10-minute plan TTL, 7-day retention metadata, 100-entry log.
- **Safety model**: target ids only, protected entries and their ancestors refused, reparse points
  and hard links never moved, canonical parent re-checked immediately before each rename, user
  exclusions validated and stored in the plan, read-only switch on by default.
- **Quarantine** (`<app_data>\quarantine\<run-id>\`): manifest written during the run and flushed
  every 256 moves, byte-exact restore that never overwrites, batch retired when fully restored,
  single-batch purge as the only permanent removal.
- **Operation log** (`<app_data>\cleanup-log.json`, max 100) plus an Activity entry per run,
  restore and purge. `clear_history` still removes only Cyron's own history.
- **IPC**: 8 new commands — `get_cleanup_mode`, `set_cleanup_mode`, `preview_cleanup` (accepts
  validated exclusions), `start_cleanup`, `list_quarantine`, `restore_quarantine`,
  `purge_quarantine`, `get_cleanup_log` — for 19 total.
- **Cleaner UI**: groups (Windows / Applications / Developer tools / AI tools), risk and category
  filters, search, expandable per-target detail with impact text, selection bar, preview drawer with
  the planned path list and confirmation checkbox, progress bar, per-item results, Quarantine tab
  (restore/purge with confirmation) and History tab.
- **Security suite**: `tests/security.rs` (17 tests) replaces the blanket `read_only.rs` source
  audit with module boundaries plus protected-path, id, junction, link, restore/purge and read-only
  assertions. `tests/cleanup_stress.rs` (5) and `examples/cleanup_fixture.rs` cover wide fixtures,
  locked files, cancellation and the full round trip. CI runs all three.
- **Catalog adjustment**: the Windows Update download cache moved to `protected` so Cyron never
  manipulates servicing state; final split is 27 safe / 9 caution / 12 protected.

### Phase 4 & Priority 0 Hardening

- **Process manager** (`src-tauri/src/process.rs`, new): discovers active processes executing from the
  application installation directory. Enforces critical system process protection (`csrss.exe`,
  `services.exe`, `explorer.exe`, etc.) and denies termination of system directory binaries. Sends
  graceful `WM_CLOSE` to top-level windows before falling back to `TerminateProcess`.
- **Standard uninstaller** (`src-tauri/src/uninstall.rs`, new): safely tokenizes registry uninstall
  commands, normalizes MSI uninstallers (`msiexec.exe /x`), rejects shell wrappers (`cmd.exe`,
  `powershell.exe`, `.bat`, `.cmd`, `.ps1`), verifies executable presence, runs uninstaller without
  arbitrary shell interpolation, tracks exit codes, and surfaces reboot requirements (code 3010).
- **Leftover scanner & recovery** (`src-tauri/src/leftovers.rs`, new): scans likely leftover directories
  and registry subkeys for uninstalled apps with confidence (`high`/`medium`/`low`) and risk
  (`safe`/`caution`/`protected`) classification. Protects shared vendor folders and system roots.
  Automatically exports registry backup (`.reg`) into the quarantine run batch prior to deleting any
  registry subkey. Quarantines file/directory leftovers using the crash-consistent recovery engine.
- **Batch uninstaller**: sequential execution queue with isolated per-item results and exit codes.
  Prevents automatic follow-up cleanup approval, requiring separate user review before residual deletion.
- **Priority 0 cleanup recovery hardening**:
  - Durable crash-consistent write-ahead recovery journal (`recovery_journal.jsonl`) written and
    flushed before any file move. Corrupted or missing manifests reconstruct automatically.
  - Atomic no-overwrite restoration using Win32 `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`,
    preventing destination overwrite races.
  - Snapshot binding: files created after preview approval are detected and skipped with warnings.
  - Safe purge retry: locked files leave remaining manifest and journal items intact.
  - Quarantine storage caps (`MAX_QUARANTINE_TOTAL_BYTES` = 20 GiB) and minimum free disk space check
    (`MIN_FREE_SPACE_BYTES` = 512 MiB via Win32 `GetDiskFreeSpaceExW`).
- **UI Integration**: `AppDrawer.tsx` extended with Segmented tabs (Overview, Processes, Uninstall, Leftovers),
  live process management, uninstaller execution launcher, and leftover scanner with candidate selection.
  `Uninstaller.tsx` updated with multi-select checkboxes and a bottom batch uninstall drawer.

## Key files

- `src-tauri/src/process.rs`: process discovery, critical process deny list, termination.
- `src-tauri/src/uninstall.rs`: safe uninstaller tokenization, execution, exit code handling.
- `src-tauri/src/leftovers.rs`: leftover detection, .reg backup, residual cleanup.
- `src-tauri/src/cleanup.rs`: cleanup engine, recovery journal, quarantine, guard, log.
- `src-tauri/src/commands.rs`: 26 validated commands, process/uninstall/leftover IPC.
- `src-tauri/tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`: security & stress evidence.
- `src/features/uninstaller/AppDrawer.tsx`, `Uninstaller.tsx`: uninstaller UI, drawer tabs, batch queue.
- `src/lib/api.ts`, `src/lib/types.ts`: IPC contracts.
- `GATES.md`: Phase 4 acceptance ledger with recorded evidence.

## Verification

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend tests | `npm test` | 71/71 pass, 7 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` (448.7 KiB) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 148 pass, 3 ignored |
| Permission boundaries | `... --test security -- --nocapture` | 21/21 pass |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Scanner stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored 20k-file test) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (15.73 MiB) |
| Launch | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron") |
| Cleaner UI pass | Native window, both themes, `scripts/capture-window.ps1` | Pass — 28/48 locations exist, 24.8 GB detected; two defects fixed |
| Windows CI | GitHub Actions | Workflow extended; remote run not observed |

## Known limitations

- The debug desktop executable loads the Vite dev server; launching it without `npm run dev` shows a
  connection error. Release packaging/signing is unverified.
- Preview reads sizes, so read-only mode gates execution only; the Cleaner still requires the plan
  id and the confirmation checkbox, and refuses to plan Protected entries.
- Cleaning is catalog-scoped: custom install paths, projects and files outside the 48 known
  locations are never touched.
- Locked, in-use or permission-denied files are skipped and reported; no restart, service stop or
  process termination is used to force them.
- Quarantine has no size cap or automatic expiry sweep; the 7-day `expiresAt` is informational.
- Restore can fail if the original path now exists (the quarantined copy is kept and reported) or if
  the original folder was removed and cannot be recreated.
- The Windows Update download cache is measured as protected; Storage Sense/Disk Cleanup remain the
  supported route.
- MFT needs administrator rights and NTFS; whole-volume MFT and walk totals differ by counting rule.
- Benchmarks are warm-cache, single-machine; no WizTree comparison.
- Store/MSIX inventory is still missing; the IPC tree is pruned; duplicates are candidate-based.
- Git history still contains the original `.chryypt` files. Rewriting history needs explicit
  approval.
- Windows CI has been added but its remote run has not been observed from this machine.

## Next directive

Phase 4 (uninstall engine) may start from this baseline. It needs process discovery with a
critical-process deny list, registry export/backup and restore, a plan model for non-file
operations, shared-dependency and system-component protection, and fixtures for vendor uninstaller
execution — plus extending `tests/security.rs` module boundaries to every new destructive module.
Do not add a hidden deletion, termination or registry-write path outside that plan/confirm/quarantine
pattern.
