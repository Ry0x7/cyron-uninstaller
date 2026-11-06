# Cyron — Active Handoff

Updated: 2026-11-06

## Current status

Phase 1 (foundation, read-only scanning) and Phase 2 (disk intelligence) are implemented across the
Rust backend and React interface. The app discovers installed Windows applications, scans folders
with either an NTFS MFT reader or a streaming directory walk, renders a treemap with a persistent
synchronized folder tree, verifies duplicate files, categorizes storage, measures 48 developer/AI
cache targets, records scan history and persists appearance/scan settings.

There is **no user-data cleanup, uninstall, process termination or registry-write path**.
`clear_history` removes only Cyron's own history log.

Latest measured checks: typecheck clean; **51 frontend tests in 6 files pass**; **84 Rust tests
pass** (73 unit, 5 read-only, 1 real-system, 5 stress) with 3 ignored; `DIST_OK`;
`DESKTOP_BINARY_OK`; `DESKTOP_SMOKE_OK`; both themes captured across all six views with a real scan
loaded. Release MSI/NSIS packaging and signing remain unverified.

## Implemented

### Phase 1

- Tauri v2 shell with strict CSP, dialog capability, one native window and eleven commands
  (`list_installed_apps`, `list_volumes`, `list_caches`, `start_scan`, `cancel_scan`,
  `start_duplicate_scan`, `cancel_duplicate_scan`, `measure_path`, `reveal_path`, `get_history`,
  `clear_history`).
- Registry inventory: four uninstall hives, update/patch/system filtering, dedupe, install-date and
  `EstimatedSize` normalization, protected-system hint.
- Volumes: local fixed/removable capacity and free space; network/optical skipped deliberately.
- Cache catalog: AI tools, IDEs, package managers, containers/WSL and Windows/browser locations;
  Safe / Review / Protected risk model; protected data never offered for removal.
- UI: Overview, Uninstaller (inventory + drawer with *Measure folder* and *Open location*), Disk
  Analyzer, Cleaner (preview), Activity (history + clear), Settings.
- Local persistence: `history.json` in app-data (max 100, atomic write), settings in
  `localStorage["cyron.settings.v1"]`, sidebar state in localStorage.
- Tooling: `scripts/msvc-env.mjs`, `verify-dist.mjs`, `verify-desktop-binary.mjs`,
  `smoke-desktop.mjs`, Windows CI workflow, Vitest suite, Rust unit tests, live-system integration
  test, read-only safety suite.

### Phase 2

- **MFT scanner** (`src-tauri/src/mft.rs`): boot sector and `$MFT` run-list parsing,
  update-sequence fixups, `FILE_NAME`/`$DATA`/`$STANDARD_INFORMATION`, subtree construction,
  per-record hard-link dedupe, a 768 MiB record budget checked before allocation, cancellation every
  4,096 records, and one fixed 1,024-byte parse buffer. Live elevated measurement on `C:\`:
  2,211,292 files / 604,557 dirs / 332.3 GiB in 14.52 s, 655.5 MiB peak.
- **Method selection**: `auto` uses MFT only for drive roots; subfolders use the walk (System32:
  0.28 s walk vs 3.91 s MFT). Any requested method that falls back reports the reason in
  `methodNote`, shown in the analyzer.
- **Bounded walker**: streaming directory enumeration, ≤200 retained children per folder, ≤100,000
  tree nodes, ≤40 file nodes per folder, ≤4,096 extension keys, ≤1,000,000 tracked hard-link ids,
  top-24 extension stats, top-100 largest files, throttled progress and cancellation.
- **Duplicates** (`duplicates.rs`): size buckets → 4 KiB head/tail SHA-256 → full streaming
  SHA-256; hard links flagged and excluded from recoverable bytes; progress and cancellation;
  caps on candidates (1M), groups (200) and files per group (50) with a `truncated` flag.
- **Storage** (`storage.rs`): ten categories with known/inferred confidence, five age buckets and
  up to six explained insights; root-relative detection for Program Files, Downloads, Steam,
  `.cargo`, `.ollama`, Docker/WSL and other known locations.
- **Analyzer UI**: Map / Duplicates / Storage modes, persistent resizable virtualized tree column
  (220–520 px, persisted), filters (name, size, age, extension, type, sort), synchronized treemap
  selection, breadcrumbs and details, scanner-method badge, partial-result and stale-result states.
- **Benchmarks**: `scripts/bench-scan.mjs` + `examples/scan_bench.rs`; results in
  [docs/BENCHMARKS.md](docs/BENCHMARKS.md). Warm cache, one machine; not comparable to WizTree
  without matched measurement.
- **Stress tests** (`tests/stress_scan.rs`): 20,000-file wide directory with exact totals and
  bounded children, deep nesting, access-denied folders, exclusion subtree and cancellation.

### Phase 1 remediation

- `.chryypt/` runtime sessions and checkpoints are untracked and gitignored; no credentials or
  secrets were found in them. The initial commit still contains those files, so removing them from
  history requires a rewrite that has **not** been performed.
- `GATES.md` is now a tracked Phase 2 acceptance ledger (previously a gitignored Phase 1 file).
- Scanner memory bounds replaced the previously unbounded per-directory entry vectors and
  extension map; stress tests cover the wide/deep cases.
- UI fixes: text-contrast token correction (light-theme arithmetic), uninstaller table minimum
  width, Activity refresh plus error-vs-empty states and Escape-to-cancel confirmation, Cleaner
  incomplete-measurement state, Settings/Uninstaller copy that no longer calls the app "Phase 1",
  per-root tree reset, focus return after expand/collapse, duplicate-panel stale-scope cancellation.

## Key files

- `src-tauri/src/mft.rs`, `scan.rs`, `fs.rs`: scanners and native enumeration.
- `src-tauri/src/duplicates.rs`, `storage.rs`: Phase 2 analysis.
- `src-tauri/src/apps.rs`, `caches.rs`, `volumes.rs`: Windows inventory.
- `src-tauri/src/commands.rs`, `history.rs`, `lib.rs`: IPC, validation, persistence.
- `src/features/analyzer/*`: analyzer, tree, filters, duplicates, storage.
- `src/App.tsx`, `src/features/*`: shell and six views.
- `src/lib/analyzerFilters.ts`, `treemap.ts`, `categories.ts`, `risk.ts`, `format.ts`: pure logic.
- `GATES.md`: Phase 2 acceptance ledger.
- `docs/BENCHMARKS.md`: measured scanner results.

## Verification

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend tests | `npm test` | 51/51 pass, 6 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 84 pass, 3 ignored |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Read-only safety | `... --test read_only -- --nocapture` | 5/5 pass |
| Stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored 20k-file test) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` |
| Launch | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` |
| Visual pass | Six views × two themes + real scan, native window | Rendered correctly |
| MFT live scan | Elevated `examples/scan_bench` | `C:\` in 14.52 s, 2.21M files |

## Known limitations

- The debug desktop executable loads the Vite dev server; launching it without `npm run dev` shows a
  connection error. Release packaging/signing is unverified.
- MFT needs administrator rights and NTFS, and reads the whole volume even for a subfolder root.
- Whole-volume MFT and walk totals differ by counting rule (record vs path; hard links).
- Benchmarks are warm-cache, single-machine, one run; no WizTree comparison and no cold-cache
  methodology.
- Store/MSIX inventory is still missing; the IPC tree is pruned; duplicates are candidate-based
  (≥1 MiB by default); storage categories are heuristics.
- Git history still contains the original `.chryypt` files. Rewriting history needs explicit
  approval.
- Windows CI has been added but its remote run has not been observed from this machine.

## Next directive

Phase 3 (safe cleaning) may start from this baseline. It must add preview + explicit confirmation,
protected-data enforcement (models, histories, repositories, Docker/WSL volumes), operation logs and
rollback where supported, and extend the read-only audit tests to the new destructive boundary.
Do not add cleanup or uninstall paths without those gates.
