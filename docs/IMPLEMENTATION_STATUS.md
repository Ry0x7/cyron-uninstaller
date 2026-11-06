# Cyron — Implementation status

Last updated: 2026-11-06 (Phase 2 + Phase 1 remediation)

## Summary

Phase 1 (foundation, read-only scanning) and Phase 2 (disk intelligence) are implemented. The
application is a real Tauri v2 + Rust backend with a React/TypeScript interface, live Windows
registry and filesystem data, an interactive treemap with a persistent folder tree, an MFT scanner
with a directory-walk fallback, duplicate-file verification, storage categorization and a
reproducible benchmark harness.

There are **no cleanup, uninstall, process-termination or registry-write operations against user
data** in this build. Activity can remove only Cyron's own history log.

Latest measured verification: frontend typecheck clean; **51 Vitest tests in 6 files pass**;
**84 Rust tests pass** (73 unit, 5 read-only safety, 1 real-system, 5 stress) with 3 ignored. The
desktop window launched and all six views were captured in both themes with a real scan loaded.

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
| 14 | No user-data cleanup/uninstall | Done by construction | No command deletes scanned/cache files, uninstalls apps, terminates processes or writes registry values; `read_only` suite audits the backend source |
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
| 17 | Read-only preservation | Done | No new command deletes anything; duplicate and storage features are read-only; the `read_only` suite still passes |
| 18 | Documentation | Done | README, this file, ARCHITECTURE, ROADMAP, HANDOFF, BENCHMARKS, GATES |

## Verification commands and current results

| Check | Command | Result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend unit tests | `npm test` | Pass, 51/51 across 6 files |
| Frontend production bundle | `node scripts/verify-dist.mjs` | Pass (`DIST_OK`) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | Pass: 73 unit (2 ignored), 5 read-only, 1 real-system, 5 stress (1 ignored) |
| Real registry/cache/scan | `... --test real_system -- --nocapture` | Pass: live registry, live cache targets, exact byte fixture |
| Read-only safety | `... --test read_only -- --nocapture` | Pass 5/5: source audit for destructive APIs, junction skip, hard-link dedupe, missing path, protected catalog |
| Stress | `... --test stress_scan -- --nocapture` | Pass 5/5 (one 20k-file test) |
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
9. **No duplicate management.** Phase 2 is read-only; selecting or deleting duplicates belongs to a
   later phase.
10. **Debug build requires the dev server.** Release packaging, signing and a standalone installer
    remain unverified.
11. **Windows only.** Win32 APIs are called unconditionally.

## Phase 3 readiness

Phase 2 delivers the evidence Phase 3 needs: a verified scanner with a measured cost profile, a
duplicate report with recoverable-byte accounting, storage categories with confidence labels, and
read-only safety tests that still pass. Before any deletion path is added, Phase 3 must introduce
preview + confirmation, protected-data enforcement (models, histories, repositories, Docker/WSL
volumes), operation logs and rollback where supported, and move the read-only audit tests to cover
the new boundary.
