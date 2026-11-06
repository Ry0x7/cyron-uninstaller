# Cyron — Active Handoff

Updated: 2026-11-06 (Phase 3 complete)

## Current status

Phases 1–3 are implemented across the Rust backend and React interface. Cyron discovers installed
Windows applications, scans folders with an NTFS MFT reader or a streaming directory walk, renders a
treemap with a persistent synchronized folder tree, verifies duplicates, categorizes storage,
measures 48 developer/AI/Windows cache locations, and **cleans 36 of them safely**: preview, explicit
confirmation, move into a recoverable quarantine, per-item results, restore or purge, and a local
operation log.

There is still **no uninstall, process termination or registry-write path**. Cleaning accepts target
*ids* from the fixed catalog only, never arbitrary paths, and protected data is refused by the
backend rather than hidden by the UI.

Latest measured checks: typecheck clean; **70 frontend tests in 7 files pass**; **133 Rust tests
pass** (105 unit, 17 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress) with 3 ignored;
`CLEANUP_FIXTURE_OK`; `DIST_OK`; `DESKTOP_BINARY_OK`; `DESKTOP_SMOKE_OK`. No real user cache was
cleaned during verification — the engine was exercised against isolated `%TEMP%` fixtures only.
Release MSI/NSIS packaging and signing remain unverified. The Phase 3 Cleaner was captured
visually in both themes (G13), which found and fixed two defects: preview was blocked by read-only
mode, and Overview joined its cache warnings into one run-on sentence.

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

## Key files

- `src-tauri/src/cleanup.rs`: cleanup engine, guard, quarantine, log.
- `src-tauri/src/commands.rs`: 19 validated commands, plan registry, read-only switch.
- `src-tauri/tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`: Phase 3
  safety evidence.
- `src/features/cleaner/Cleaner.tsx`, `src/lib/cleanup.ts`, `src/lib/risk.ts`: Cleaner UI and
  selection rules.
- `src-tauri/src/mft.rs`, `scan.rs`, `fs.rs`, `duplicates.rs`, `storage.rs`: scanners and Phase 2
  analysis.
- `src-tauri/src/apps.rs`, `caches.rs`, `volumes.rs`, `history.rs`, `lib.rs`: Windows inventory,
  catalog, persistence and app state.
- `src/features/analyzer/*`, `src/App.tsx`, `src/lib/settings.tsx`: shell, analyzer and settings.
- `GATES.md`: Phase 3 acceptance ledger with recorded evidence.
- `docs/BENCHMARKS.md`: measured scanner results.

## Verification

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend tests | `npm test` | 70/70 pass, 7 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 133 pass, 3 ignored |
| Permission boundaries | `... --test security -- --nocapture` | 17/17 pass |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Scanner stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored 20k-file test) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` |
| Launch | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` |
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
