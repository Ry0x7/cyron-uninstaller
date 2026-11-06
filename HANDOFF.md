# Cyron — Active Handoff

Updated: 2026-11-06

## Current status

Phase 1 foundation and read-only scanning is implemented across the Rust backend and React
interface, including a synchronized folder tree in the analyzer rail. The app discovers installed
Windows applications, scans folders and renders a treemap, measures 48 developer/AI/Windows cache
targets, records scan history, and persists appearance/scan settings. There is no user-data
cleanup, uninstall, process termination or registry-write path; `clear_history` removes only
Cyron's own history log.

Latest measured checks: typecheck clean; 38 frontend tests in 5 files pass; 38 Rust tests pass with
1 ignored; the real-system integration test passes; the 5-test read-only safety suite passes. The
debug desktop build succeeded (`DESKTOP_BINARY_OK`, 14 MiB `cyron.exe`) and
`DESKTOP_SMOKE_OK` launched a real window titled "Cyron". Release MSI/NSIS packaging, signing and
a native visual pass remain unverified.

## Implemented

- Tauri v2 shell with a strict CSP, dialog capability, one native window and nine commands
  (`list_installed_apps`, `list_volumes`, `list_caches`, `start_scan`, `cancel_scan`,
  `measure_path`, `reveal_path`, `get_history`, `clear_history`). The last only removes Cyron's
  own history file; no scanned/user-data cleanup command exists.
- Registry inventory: four uninstall hives, update/patch/system filtering, dedupe, install-date and
  `EstimatedSize` normalisation, protected-system hint.
- Scanner: native `FileIdBothDirectoryInfo` enumeration with `std::fs` fallback, reparse skipping,
  hard-link dedupe, exclusions, hidden/system filter, depth cap, throttled progress, cancellation,
  top-24 returned extensions, top-100 largest files, pruned IPC tree (200 children / 8 levels /
  up to 40 file nodes per folder plus aggregate groups). Hard-link ids are tracked up to 1,000,000.
  The directory entry/child vectors and extension map can grow with input, so strict memory bounds
  are not guaranteed.
- Volumes: local fixed/removable capacity and free space; network/optical skipped deliberately.
- Cache catalog: AI tools, IDEs, package managers, containers/WSL and Windows/browser locations;
  Safe / Review / Protected risk model; totals per risk; protected data never offered for removal.
- UI: Overview, Uninstaller (inventory + detail drawer with *Measure folder* and *Open location*),
  Disk Analyzer (volume cards, folder picker, treemap, synchronized folder tree and
  selection/largest/types rail), Cleaner (preview), Activity (history + clear), Settings (theme,
  hidden files, exclusions).
- Local persistence: `history.json` in app-data (max 100, atomic write), settings in
  `localStorage["cyron.settings.v1"]`, sidebar state in localStorage.
- Tooling: `scripts/msvc-env.mjs` (finds VS C++ toolchain through `vswhere`), `verify-dist.mjs`,
  `verify-desktop-binary.mjs`, `smoke-desktop.mjs`, Windows CI workflow, Vitest suite, Rust unit
  tests, a live-system integration test and a read-only safety suite (`tests/read_only.rs`).
  Latest measured: 38 Rust tests passed with 1 ignored, real-system passed, read-only 5/5 passed.

## Key files

- `src-tauri/src/scan.rs`, `fs.rs`: scanner and native enumeration.
- `src-tauri/src/apps.rs`, `caches.rs`, `volumes.rs`: Windows inventory.
- `src-tauri/src/commands.rs`, `history.rs`, `lib.rs`: IPC, validation, persistence.
- `src/App.tsx`, `src/features/*`: shell and six views.
- `src/lib/treemap.ts`, `categories.ts`, `risk.ts`, `format.ts`: pure frontend logic.
- `GATES.md` (gitignored): Phase 1 acceptance ledger.

## Verification

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Vitest | `npm test` | Pass, 38/38 across 5 files |
| Frontend bundle | `node scripts/verify-dist.mjs` | Earlier reported `DIST_OK`; not re-run for this handoff |
| Rust tests | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | Pass: 38 passed, 1 ignored |
| Read-only safety | `... --test read_only -- --nocapture` | Pass: 5/5 |
| Real-system test | `... --test real_system -- --nocapture` | Pass |
| Desktop binary | `npm run build:desktop` + `node scripts/verify-desktop-binary.mjs` | Pass: `DESKTOP_BINARY_OK`, 14.03 MiB |
| Launch smoke | `node scripts/smoke-desktop.mjs` | Pass: `DESKTOP_SMOKE_OK`, window "Cyron" |
| Visual review | Both themes, 1320×860, collapsed sidebar | Pending |

Environment note: Visual Studio 2022 Build Tools and a Windows SDK were installed during this
session. Use `node scripts/msvc-env.mjs <command>` rather than bare `cargo` if `link.exe` resolves
to Git's coreutils binary.

## Known limitations

- Registry (Win32) applications only; no Store/MSIX or portable app detection, no app icons.
- Sizes come from installer metadata when present; otherwise the UI shows “—” and the user can
  measure the install folder.
- No MFT scanner and no performance benchmark. Native enumeration is used with a safe fallback.
- Treemap and Tree IPC output is pruned (200 children/folder, 8 levels, 500 rendered tree rows).
  Totals cover the walk; returned extension stats/largest files are top 24/top 100. Scan
  structures are not strictly memory-bounded for huge directories; hard-link tracking caps at one
  million ids and the fallback has no ids.
- Junctions/symlinks are counted and skipped, never followed.
- Fixed cache catalog; no custom paths or unknown tools.
- Cleaner is a preview. No deletion, uninstall, process kill, registry backup or rollback exists.
- Network and optical volumes are skipped when listing drives.
- History records disk scans only.

## Next directive

1. Run `node scripts/verify-dist.mjs` for the production bundle if the frontend changed.
2. Review the running native window in both themes at 1320×860 and collapsed-sidebar width:
   treemap labels, Tree tab sync, table density, drawer keyboard flow and loading/error/empty
   states with real data.
3. Mark only evidenced gates green in `GATES.md`; G9 (visual review) stays open until step 2.
4. Do not start Phase 2 performance claims without MFT investigation and representative
   benchmarks. Release MSI/NSIS packaging and signing belong to Phase 5.

## Open decisions

- Whether Store/MSIX applications are required for the first public release.
- Which signing infrastructure, if any, will be available for Phase 5.
- Whether the first cleaning release needs per-provider plugins or a single audited engine.
