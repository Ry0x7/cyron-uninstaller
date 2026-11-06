# Cyron — Implementation status

Last updated: 2026-11-06 (Phase 3 — safe cleaning)

## Summary

Phases 1–3 are implemented. Cyron is a real Tauri v2 + Rust backend with a React/TypeScript
interface, live Windows registry and filesystem data, an interactive treemap with a persistent
folder tree, an MFT scanner with a directory-walk fallback, duplicate-file verification, storage
categorization, a reproducible benchmark harness, and a review-first cleanup engine with
quarantine and restore.

Cleaning exists, but only over the **fixed 48-location cache catalog**, only after a preview and an
explicit confirmation, and only from a non-read-only backend. Files are **moved into quarantine**,
never deleted outright; the only permanent step is purging a batch the user selects. Protected
entries (histories, models, settings, credentials, Docker/WSL data, Firefox profiles, Windows
Update downloads) are refused in the backend. There is still no uninstall, process termination or
registry-write path.

Latest measured verification: frontend typecheck clean; **70 Vitest tests in 7 files pass**;
**133 Rust tests pass** (105 unit, 17 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress)
with 3 ignored; `CLEANUP_FIXTURE_OK`; `DIST_OK`; `DESKTOP_BINARY_OK`; `DESKTOP_SMOKE_OK`. No real
user cache was cleaned during verification.

## Phase 1 acceptance

| # | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Tauri v2 + React + TypeScript scaffold | Done | `src-tauri/`, `src/`, `package.json`, `vite.config.ts`, `src-tauri/tauri.conf.json` |
| 2 | Design system and six-section navigation | Done | `src/styles/tokens.css`, `src/styles/app.css`, `src/App.tsx`, `src/components/ui.tsx`; light/dark themes, collapsible sidebar |
| 3 | Discover installed Windows applications | Done | `src-tauri/src/apps.rs` reads four uninstall registry views, filters updates/system components, deduplicates, normalizes dates and sizes; unit tests in the module |
| 4 | Publisher, version, size, date and path | Done where published | `Uninstaller.tsx`, `AppDrawer.tsx`; `estimatedSize` is `null` when absent and the UI shows “—” |
| 5 | Filesystem scanning and directory-size analysis | Done | `fs.rs` native enumeration + fallback, `scan.rs` streaming walker; totals/depth/exclusion/hard-link/cancellation/progress tests |
| 6 | Interactive treemap with drill-down | Done | `Treemap.tsx`, `lib/treemap.ts` (squarified), `lib/treemap.test.ts` |
| 7 | Synchronized selection between treemap, tree and details | Done | Persistent tree column, treemap selection, breadcrumbs and details rail all share one selected node |
| 8 | WinDirStat-style folder tree | Done | `TreeView.tsx`: virtualized (no 500-row cap), resizable column, ARIA tree keyboard navigation, selection sync, per-root reset |
| 9 | Real storage usage and scan progress | Done | `volumes.rs`, `GetDiskFreeSpaceExW`; throttled `cyron-scan-progress`; live counters |
| 10 | Developer/AI cache discovery | Done | 48-location catalog in `caches.rs`, measured with `scan::measure_path` |
| 11 | Risk classification and plain-language explanations | Done | `lib/risk.ts`, Cleaner rows, protected-data section |
| 12 | Cancellation, error handling and exclusions | Done | Atomic cancel flag, capped error list, validated exclusion prefixes |
| 13 | Tests | Passing | 51 Vitest; 84 Rust tests passed, 3 ignored (see table below) |
| 14 | No user-data cleanup/uninstall | Superseded by Phase 3, within bounds | Phase 3 added catalog-only cleaning with quarantine. Uninstall, process termination, registry writes and arbitrary-path deletion still do not exist; the boundary is enforced by `tests/security.rs` |
| 15 | Local-only privacy | Done | No network code; settings in localStorage; history in app-data |
| 16 | Windows CI | Workflow added, remote run not observed | `.github/workflows/ci.yml` runs typecheck, Vitest, build and `cargo test` on `windows-latest` |
| 17 | Documentation | Done | `README.md`, this file, `ARCHITECTURE.md`, `ROADMAP.md`, `AGENTS.md`, `HANDOFF.md`, `BENCHMARKS.md` |
| 18 | Native desktop build and launch | Done for the debug build with a running Vite dev server; release packaging unverified | `verify-desktop-binary.mjs` → `DESKTOP_BINARY_OK`; `smoke-desktop.mjs` → `DESKTOP_SMOKE_OK`; all six views captured in both themes |

## Phase 2 acceptance

| # | Requirement | Status | Evidence and limits |
| --- | --- | --- | --- |
| 1 | NTFS MFT scanning | Done | `src-tauri/src/mft.rs` reads the boot sector, `$MFT` run list, applies update-sequence fixups and parses `FILE_NAME`, `$DATA` and `$STANDARD_INFORMATION` into a subtree. Measured live on `C:`: 2,211,292 files / 604,557 dirs / 332.3 GiB in 14.52 s. Requires an elevated process; unverified below administrator rights by design (it returns a clear reason and falls back) |
| 2 | Directory-walk fallback | Done | `scan.rs` unchanged in role; `mft::scan` returns `fallback_reason` for non-NTFS, inaccessible volume handles, parse failures, volume record budget exceeded and cancellation. `method`/`methodNote` are surfaced in `ScanResult` and the analyzer |
| 3 | Automatic method selection | Done | `method: auto` selects MFT only for a drive root (e.g. `C:\`); subfolders use the walk because MFT loads the whole volume (measured: System32 3.91 s MFT vs 0.28 s walk). An explicit `mft` request on a subfolder is allowed |
| 4 | Junctions, hard links, permissions | Done | Reparse points never followed in either scanner; MFT uses a per-record bitset so one file is counted once; the walk bounds hard-link ids at 1,000,000; unreadable folders become capped errors |
| 5 | Bounded memory, streaming, cancellation, progress | Done | The walk streams directory entries (no per-directory entry vector), retains ≤200 children per folder, ≤100,000 tree nodes, ≤40 file nodes per folder and ≤4,096 extension keys; MFT enforces a 768 MiB parse budget (4,910,404 records at the current record size) before allocating, polls cancellation every 4,096 records and uses one fixed 1,024-byte stack buffer |
| 6 | Persistent, resizable tree column | Done | `Analyzer.tsx` split view with a drag handle (220–520 px, persisted), `TreeView.tsx` virtualized rows |
| 7 | Synchronized expansion/selection/breadcrumbs/details | Done | Treemap click updates the tree and details; tree selection updates the treemap outline and details; drill-down updates breadcrumbs; changing the root resets tree state |
| 8 | Large trees without a row cap | Done | Virtualized rendering with `ResizeObserver`; the tree renders only the visible window |
| 9 | Sorting and filtering | Done | `lib/analyzerFilters.ts`: name, minimum size, age, extension, category/type, sort by size/name/files/modified; filters apply to the tree, largest files and treemap with filtered totals |
| 10 | Logical vs allocated size and percentages | Done | Both reported in totals and details; share always computed from logical size |
| 11 | Duplicate detection | Done | Size buckets → 4 KiB head/tail SHA-256 → full streaming SHA-256; progress and cancellation; hard links flagged and excluded from recoverable bytes; files that change during hashing are reported, not trusted |
| 12 | Duplicate groups UI | Done (read-only) | `DuplicatesPanel.tsx`: groups, locations, per-file size, wasted bytes, pagination, cancel; no delete action |
| 13 | Storage categorization | Done | `storage.rs`: ten categories (applications, games, downloads, documents, media, system, developer, AI, caches, other) with `known` vs `inferred` confidence, five age buckets and up to six explained insights; `StoragePanel.tsx` shows categories, sizes and confidence |
| 14 | Developer/AI storage visibility without disposal claims | Done | Docker/WSL/package caches/model locations are classified and explained; copy states they are observations, not removal guidance |
| 15 | Benchmark harness | Done | `scripts/bench-scan.mjs` + `examples/scan_bench.rs`; results in [BENCHMARKS.md](BENCHMARKS.md). Warm-cache, single machine, one run per MFT target |
| 16 | Reliability under large inputs | Done | `tests/stress_scan.rs`: 20,000-file wide directory with exact totals and bounded children, deep nesting, access-denied directories, exclusion and cancellation; `scan.rs` unit tests cover the node/extension caps |
| 17 | Read-only preservation | Superseded by Phase 3 | Phase 2 added no destructive command and its read-only guarantees still hold for scanning, duplicates and storage. The blanket source audit was replaced in Phase 3 by the narrower `tests/security.rs` boundary suite |
| 18 | Documentation | Done | README, this file, ARCHITECTURE, ROADMAP, HANDOFF, BENCHMARKS, GATES |

## Phase 3 acceptance

| # | Requirement | Status | Evidence and limits |
| --- | --- | --- | --- |
| 1 | Modular cleanup engine with audited providers | Done | `src-tauri/src/cleanup.rs` (guard, planner, executor, quarantine, log) over the fixed catalog in `caches.rs`. Providers are catalog entries, not plugins: 27 safe, 9 caution, 12 protected |
| 2 | Scan → preview → selection → confirmation → execution → verification | Done | `preview_cleanup` returns a server-held plan plus a preview; `start_cleanup` consumes the plan id once, re-checks the selection, the 10-minute TTL and read-only mode; the report carries per-item status and the quarantine batch is listed immediately |
| 3 | Backend enforces eligibility, never trusts the frontend | Done | Commands accept target **ids** only; `catalog_specs` refuses unknown and protected ids; `guard_path` runs before planning and again before each target; `check_plan_selection` rejects ids the plan does not contain |
| 4 | Dry run, progress, cancellation, failure recovery, detailed results | Done | Preview is the dry run; `cyron-cleanup-progress` events every 120 ms; cancellation is an `AtomicBool` polled per file; a locked file is skipped and the run continues; results list up to 500 items with reasons |
| 5 | Revalidate identity/location/eligibility before modification | Done | Each file is re-`symlink_metadata`d, rejected if it is not a regular file or carries a reparse attribute, and its canonicalized parent must still be inside the target root immediately before the rename |
| 6 | No symlink/junction traversal, path escapes or protected deletion | Done | Reparse points are skipped in the walk and refused per file; `normalize_key`/`is_within_key` compare canonical prefixes; protected catalog paths and their ancestors are refused; `tests/security.rs` covers junction, link and escape cases |
| 7 | Windows and application providers | Done | User temp, Windows temp, crash dumps, INetCache, Chrome/Edge caches, VS Code/Cursor/Antigravity cache+GPU+log entries, Docker logs. Windows Update downloads are measured but protected; no servicing or update-service manipulation |
| 8 | Developer and AI providers | Done | Claude Code, Codex CLI, Gemini CLI, Cursor, Antigravity, VS Code, npm, pnpm, Yarn, pip, Cargo, NuGet, Ollama/LM Studio logs. Each entry carries an impact note saying whether it regenerates, is re-downloaded, or costs a reset |
| 9 | Strict protection of repositories, models, histories, credentials, Docker/WSL, browser profiles | Done | 12 protected entries plus a guard that refuses ancestors of protected data; hard links and reparse points are never moved; no command accepts a free-form path |
| 10 | Review-first workflow, never automatic | Done | Backend starts read-only; preview works in that mode because it only reads, while `start_cleanup` refuses until cleaning is enabled; the drawer requires a confirmation checkbox; nothing runs on a timer or at startup |
| 11 | Safe / Review / Protected classification | Done | `risk` on every catalog entry, surfaced as `RiskBadge`, filters and the protected-data section; protected rows have no checkbox and the backend refuses them anyway |
| 12 | Confirmation showing paths, impact and recovery | Done | Preview drawer lists target paths, per-target impact, the planned file list (capped at 400 with an explicit note), skipped items, warnings, expiry time and the quarantine promise |
| 13 | Recoverable quarantine with retention and restore | Done | `<app_data>\quarantine\<run-id>\manifest.json`; restore is byte-exact, never overwrites, and retires the batch when everything is back; `expiresAt` records 7-day retention. No automatic expiry sweep or size cap |
| 14 | Identify operations that cannot be rolled back | Done | Purge is labelled permanent in the UI and the confirmation text; restore failures and skipped files are reported per item |
| 15 | Backend-controlled exclusions, audit log, global read-only mode | Done | Settings exclusions are validated as absolute non-root paths by `preview_cleanup`, stored in the plan and applied to both preview and execution; `cleanup-log.json` keeps 100 entries; `read_only` is an `AtomicBool` in `AppState` |
| 16 | No administrator rights requested | Done | No elevation call exists; locked or system-owned files are reported as skipped |
| 17 | Functional Cleaner UI in the existing page | Done | `Cleaner.tsx`: Windows/Applications/Developer/AI groups, risk and category filters, search, per-target detail, selection bar, preview drawer, progress, results, Quarantine and History tabs, read-only banner, empty/error/stale states |
| 18 | Destructive-operation security suite replacing the blanket audit | Done | `tests/read_only.rs` removed; `tests/security.rs` (17 tests) asserts which modules may call destructive APIs, protected-path refusal, hostile ids, junction/link handling, restore/purge boundaries, read-only refusal and two plan-to-execution junction swaps |
| 19 | Cleanup tested only on isolated fixtures | Done | `examples/cleanup_fixture.rs` + `scripts/verify-cleanup.mjs` use `%TEMP%\cyron-cleanup-<pid>`; `tests/cleanup_stress.rs` uses temp fixtures. No personal cache was cleaned |
| 20 | Path traversal, junction, TOCTOU, locked, partial, cancellation, restore-failure tests | Done | `tests/security.rs` covers hostile run ids, a junction target, and two TOCTOU swaps (subfolder and whole target replaced by a junction between preview and execution — nothing outside moves, the swap is reported); `tests/cleanup_stress.rs` and the fixture cover locked files, cancelled runs, restore onto an existing file and purge accounting. Moving files consumes no extra disk space, so the insufficient-space case reduces to the manifest write, which is written incrementally |
| 21 | Reconcile Phase 2 counting differences before quoting recoverable space | Done | Cleaner sizes and preview totals both come from the same directory walk (`walk_target`), so the preview matches the measured target; MFT totals are never used for cleanup figures. The UI labels sizes as measured, not as guaranteed savings |
| 22 | Windows CI, native build, all suites, real UI behaviour | Partly verified | CI workflow extended with the security, cleanup-stress and fixture steps (remote run not observed); native build, binary check and launch smoke pass; the Cleaner was driven live in both themes on 2026-11-06 (G13), which found and fixed the preview/read-only defect and an Overview warning layout bug |
| 23 | Milestone documentation | Done | README, ROADMAP, HANDOFF, this file, SECURITY, ARCHITECTURE, AGENTS and GATES updated in the same commit |

## Verification commands and current results

| Check | Command | Result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend unit tests | `npm test` | Pass, 70/70 across 7 files |
| Frontend production bundle | `node scripts/verify-dist.mjs` | Pass (`DIST_OK`) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | Pass: 105 unit (2 ignored), 17 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress (1 ignored) |
| Real registry/cache/scan | `... --test real_system -- --nocapture` | Pass: live registry, live cache targets, exact byte fixture |
| Permission boundaries | `... --test security -- --nocapture` | Pass 17/17: destructive-API module boundary, protected paths, hostile ids, junction, TOCTOU swaps, links, restore/purge, read-only |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | Pass 5/5: wide fixture, locked file, cancellation, consecutive batches, empty targets |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | Pass: `CLEANUP_FIXTURE_OK` |
| Scanner stress | `... --test stress_scan -- --nocapture` | Pass 5/5 (one 20k-file test) |
| Desktop binary | `npm run build:desktop` + `verify-desktop-binary.mjs` | Pass: `DESKTOP_BINARY_OK` |
| Launch | `node scripts/smoke-desktop.mjs` | Pass: `DESKTOP_SMOKE_OK` |
| Visual pass | Six views × two themes plus analyzer Map/Storage/Duplicates and a real `C:\Windows\System32` scan | Captured with the Tauri window attached to a running Vite dev server; no misrendered or blank view |
| MFT live scan | Elevated benchmark binary | Pass: `method: mft` on `C:` (and intentionally error-free fallback when unelevated) |
| Windows CI | GitHub Actions `ci.yml` | Workflow present and locally equivalent commands pass; the remote run has not been observed in this session |

Notes:

- `scripts/msvc-env.mjs` discovers the Visual Studio C++ toolchain through `vswhere`. Without it a
  plain shell can pick up Git's unrelated `link.exe`.
- The debug desktop executable loads the Vite dev server (`devUrl`) rather than embedded assets, so
  launching it without `npm run dev` shows a connection error. This is a known limitation of the
  debug build, not a scanner defect; `smoke-desktop.mjs` checks window presence only.

## Known limitations

1. **Registry inventory only.** Store/MSIX apps, portable apps and application icons are not
   enumerated.
2. **MFT needs elevation and NTFS.** Without administrator rights the handle to `\\.\C:` cannot be
   opened; the scan reports the reason and uses the directory walk. Volumes whose record count
   exceeds the 768 MiB budget (4,910,404 records at the current record size) are rejected and fall
   back.
3. **MFT counts records, not paths.** A file with several hard links appears once; the directory
   walk counts each path and dedupes allocated bytes only while its id set has room. Whole-volume
   totals from the two methods therefore differ (see BENCHMARKS.md).
4. **Subfolder MFT scans are slower than the walk.** MFT reads the whole volume's record set; the
   UI only auto-selects MFT for drive roots.
5. **Warm-cache measurements only.** No cold-cache methodology, and no comparison with WizTree or
   any commercial tool.
6. **The IPC tree is pruned.** At most 200 children per folder, 8 levels deep and up to 40 file
   nodes per folder plus aggregate groups; totals cover the whole walk, returned extension stats
   are the top 24 and largest files the top 100.
7. **Duplicate detection is candidate-based.** Default minimum size is 1 MiB, at most 1,000,000
   candidates, 200 returned groups and 50 files per group; the report sets `truncated` when a cap
   is hit. Files modified during hashing are reported rather than guessed.
8. **Storage categories are heuristics.** Known locations (Program Files, Downloads, Steam,
   `.cargo`, Docker/WSL, model folders) are marked `known`; extension-based classification is
   marked `inferred`. A category is never fully `known` if any of its bytes were inferred.
9. **No duplicate management.** Duplicates are reported only; only the Cleaner acts, and only on
   catalog caches.
9a. **Cleaning is catalog-scoped.** Custom install paths, individual projects and files outside the
   48 known locations are never touched. A tool that relocates its cache breaks its entry until the
   catalog is updated.
9b. **Locked and inaccessible files are left behind.** No restart-manager trick, VSS snapshot,
   service stop or process termination is used to force a cleanup; skipped items are reported.
9c. **Quarantine has no size cap or expiry sweep.** The 7-day `expiresAt` is recorded and shown but
   nothing is deleted automatically; a batch stays restorable until the user purges it.
9d. **Preview listings are capped.** Up to 400 files are listed and up to 100,000 files per target
   are considered; the totals and moves cover every considered file, and the preview says when it
   stopped early.
9e. **The Windows Update download cache is protected.** Cyron does not manipulate servicing state;
   Windows Storage Sense or Disk Cleanup remain the supported route.
10. **Debug build requires the dev server.** Release packaging, signing and a standalone installer
    remain unverified.
11. **Windows only.** Win32 APIs are called unconditionally.

## Phase 4 readiness

Phase 3 delivers the reusable halves of an uninstall engine: a validated command boundary, a
single-use plan/confirmation model, a quarantine with manifests and restore, a local operation log,
a permission-boundary test suite that can be extended to new destructive modules, and a UI pattern
for preview → confirm → progress → results.

Phase 4 still needs, before it may ship:

1. Process discovery and a critical-process deny list, since uninstallers need to detect and stop
   holding processes. Phase 3 deliberately has no termination path.
2. Registry export/backup plus restore, because leftover detection will read and eventually remove
   registry keys.
3. A plan model that covers non-file operations (registry values, services) rather than only file
   moves.
4. Explicit handling for shared dependencies and system components, with a reviewed allow list and
   a refusal path like `guard_path`.
5. Extending `tests/security.rs` module boundaries and adding fixtures for shell execution of
   vendor uninstallers, which Phase 3 does not run at all.
