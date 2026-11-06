# Cyron — Implementation status

Last updated: 2026-11-06 (Phase 1 implementation handoff)

## Summary

Phase 1 (foundation and read-only scanning) is implemented. The application has a real Tauri v2 +
Rust backend, a complete React/TypeScript interface, live Windows registry and filesystem data, an
interactive treemap and a measured developer/AI cache inventory. There are **no cleanup or uninstall operations against user data** in this build; Activity can
remove only Cyron's own history log.

Latest measured verification: frontend typecheck and 38 Vitest tests pass; 38 Rust tests pass with
1 ignored; the real-system integration test passes; the 5-test read-only safety suite passes. The
debug desktop build succeeded, `DESKTOP_BINARY_OK` reported the 14 MiB executable, and
`DESKTOP_SMOKE_OK` launched it with a real "Cyron" window. Release installer packaging and visual
review remain unverified.

## Phase 1 acceptance

| # | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Scaffold a Tauri v2 + React + TypeScript application | Done | `src-tauri/`, `src/`, `package.json`, `vite.config.ts`, `src-tauri/tauri.conf.json` |
| 2 | Design system and six-section navigation | Done | `src/styles/tokens.css`, `src/styles/app.css`, `src/App.tsx`, `src/components/ui.tsx`; light/dark themes, collapsible sidebar |
| 3 | Discover installed Windows applications | Done | `src-tauri/src/apps.rs` reads four uninstall registry views, filters updates/system components, deduplicates, normalises dates and sizes; unit tests in the module |
| 4 | Show publisher, version, size, date and path | Done (where published) | `Uninstaller.tsx`, `AppDrawer.tsx`; `estimatedSize` is `null` when the registry has none and the UI shows “—” |
| 5 | Filesystem scanning and directory-size analysis | Done | `src-tauri/src/fs.rs` native enumeration + fallback, `scan.rs` bounded walker; tests for totals, depth, exclusions, hard links, cancellation and stats |
| 6 | Interactive treemap with drill-down | Done | `src/features/analyzer/Treemap.tsx`, `src/lib/treemap.ts` (squarified), `src/lib/treemap.test.ts` |
| 7 | Synchronized selection between treemap, folder tree and details | Done | Treemap↔Tree↔Selection tabs in `Analyzer.tsx`, `TreeView.tsx`; breadcrumb drill-down; largest files and extension statistics use real scan data |
| 8 | WinDirStat-style folder tree | Done (rail tab, not a column) | `src/features/analyzer/TreeView.tsx`: real scan nodes, expand/collapse, ARIA tree keyboard navigation, selection sync, scroll-to-selection, 500-row cap; a persistent column is deferred |
| 9 | Real storage usage and scan progress | Done | `volumes.rs`; `GetDiskFreeSpaceExW`; throttled `cyron-scan-progress` events; live counters in `Analyzer.tsx` |
| 10 | Developer/AI cache discovery | Done | 48-location catalog in `src-tauri/src/caches.rs`; measured via `scan::measure_path`; catalog validity and protection tests |
| 11 | Risk classification and plain-language explanations | Done | `lib/risk.ts`, Cleaner rows, protected-data section; three levels (Safe / Review / Protected) |
| 12 | Cancellation, error handling and exclusions | Done | Atomic cancel flag, capped error list, exclusion prefixes validated in Settings and applied by the scanner |
| 13 | Tests | Passing | 38 Vitest tests in 5 files; Rust suite 38 passed, 1 ignored; real-system integration test passed; 5 read-only safety tests passed |
| 14 | No user-data cleanup/uninstall operations | Done by construction | No command removes scanned/cache files, uninstalls apps, terminates processes or writes registry values; `clear_history` removes only Cyron's own log |
| 15 | Local-only privacy | Done | No network calls; settings in localStorage; history in app-data |
| 16 | Windows CI | Added, not yet observed green | `.github/workflows/ci.yml` runs typecheck, Vitest, build and `cargo test` on `windows-latest` |
| 17 | Documentation | Done | `README.md`, this file, `docs/ARCHITECTURE.md`, `ROADMAP.md`, `AGENTS.md`, `HANDOFF.md` |
| 18 | Native desktop build and launch | Done for debug; visual review pending | `npm run build:desktop` built `cyron.exe`, `verify-desktop-binary.mjs` returned `DESKTOP_BINARY_OK`, `smoke-desktop.mjs` returned `DESKTOP_SMOKE_OK` with window title "Cyron". Release MSI/NSIS and a native visual pass are unverified |

## Verification commands and current results

| Check | Command | Result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass (no errors) |
| Frontend unit tests | `npm test` | Pass, 38/38 tests across 5 files |
| Frontend production bundle | `node scripts/verify-dist.mjs` | Earlier reported `DIST_OK`; not re-run in this handoff |
| Rust unit/integration tests | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | Pass: 38 passed, 1 ignored |
| Read-only safety tests | `... --test read_only -- --nocapture` | Pass: 5/5 (source audit, junction skip, hard-link dedupe, missing path, protected catalog) |
| Real registry/cache/scan smoke | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test real_system -- --nocapture` | Pass; live registry/cache and temporary scan fixture exercised |
| Desktop binary | `npm run build:desktop` then `node scripts/verify-desktop-binary.mjs` | Pass: `DESKTOP_BINARY_OK`, 14 MiB debug executable |
| Launch check | `node scripts/smoke-desktop.mjs` | Pass: `DESKTOP_SMOKE_OK`, window title "Cyron" |
| Visual review | Manual, 1320×860 and collapsed sidebar, both themes | Pending |

Notes:

- `scripts/msvc-env.mjs` discovers the Visual Studio C++ toolchain through `vswhere` and adds the
  linker and Windows SDK to `PATH`, `LIB` and `INCLUDE`. Without it, a plain shell can pick up
  Git's unrelated `link.exe` and fail.
- The real-system test report records installed-app discovery, cache target discovery and a
  temporary-folder scan with exact byte/file assertions. Latest reported run passed; no live
  count is treated as a stable benchmark.

## Known limitations

1. **Registry inventory only.** Store/MSIX apps, portable apps and application icons are not
   enumerated yet.
2. **Sizes are metadata when available.** `EstimatedSize` is installer-provided. Real install
   folder size is available on demand via *Measure folder*.
3. **No MFT scan.** Native `FileIdBothDirectoryInfo` enumeration is used; no benchmark exists and
   no WizTree-level speed is claimed. Each scan's directory walk is single-threaded. The hard-link
   file-id set stops retaining new ids after 1,000,000; subsequent distinct ids may later be
   counted more than once. The `std::fs` fallback has no ids and cannot dedupe hard links.
4. **Pruned visual tree.** IPC output has at most 200 children per folder, depth 8, and keeps up
   to 40 individual file nodes per folder plus aggregate groups. Directory entries and directory
   child nodes are collected before pruning; large folders can consume substantial memory. Totals
   cover the full walk, returned extension statistics are top 24 and largest files top 100.
5. **Reparse points are skipped.** Junctions, symlinks and mount points are counted and reported,
   never followed. This is a safety property, not a bug.
6. **Fixed cache catalog.** 48 known locations. Risk is `safe` (regenerable), `caution` (re-download/reset cost), or `protected` (history, model weights, settings, Docker/WSL data, browser profiles); all are measured where resolvable, but protected targets are never offered for cleanup. Custom project paths and unknown tools are not discovered.
7. **Network and optical volumes are skipped** to avoid blocking on disconnected mapped drives.
8. **No duplicate-file analysis** yet (Phase 2).
9. **No user-data cleaning, uninstall or process control** (Phases 3–4). Cleaner is a preview; clearing Activity history removes only Cyron's own log.
10. **History covers disk scans only.** Cache and application inventory passes are not logged.
11. **Windows only.** Win32 APIs are called unconditionally.

## Phase 2 readiness

The scanner already collects extension statistics, largest files, allocated size, file ids and
reparse/error counts. Resource limits are partial: tree/stat outputs, errors and tracked file ids
are capped, while per-directory entry/subdirectory vectors and the extension map can grow with
the input. A strict memory bound is not established. Phase 2 work should add:

- MFT enumeration for NTFS roots with a verified fallback and a benchmark harness (no claims before
  measurement).
- A persistent tree column beside the treemap (the synchronized Tree rail tab exists today).
- Duplicate detection using size buckets first, then partial hashes, then full hashes.
- Richer storage categories (applications, games, downloads, documents, developer tools, AI models).
- Reliability/performance passes on large roots (Windows, Program Files, project directories) with
  memory and duration measurements.

Phase 3 and later must not shortcut the risk model: protected categories (histories, models,
credentials, Docker volumes/WSL disks, repositories) stay outside any bulk removal.
