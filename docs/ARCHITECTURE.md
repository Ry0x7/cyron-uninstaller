# Cyron — Architecture

## Purpose and boundary

Cyron is a Tauri v2 desktop application for Windows. Phase 1 is read-only with respect to
scanned/user data: the backend enumerates applications, measures folders, scans a disk tree and
reports volumes. It does not clean scanned files, uninstall software, terminate processes or write
registry values. `clear_history` removes only Cyron's own `history.json`, and history replacement
can remove only a Cyron-created temporary file. The UI reflects that boundary instead of showing
disabled fake cleanup/uninstall actions.

The product target is a Revo Uninstaller + BleachBit + WinDirStat replacement with better risk
communication for non-technical users. Phase 3 adds cleaning, Phase 4 the uninstall engine.

## Runtime stack

| Part | Implementation |
| --- | --- |
| Window/shell | Tauri v2, one native window (`main`), 1320×860, minimum 1000×680 |
| Frontend | React 19 + TypeScript, Vite 7, no router |
| Backend | Rust, `tauri`, `tauri-plugin-dialog`, `winreg`, `windows-sys` |
| Storage | localStorage (preferences), `history.json` in the Tauri app-data dir |
| Frontend tests | Vitest, 32 tests |
| Backend tests | `cargo test`; latest reported run: 38 passed, 1 ignored, plus the real-system integration test passed |

Native WebView2 APIs used by the backend:

- `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)` for directory enumeration.
- `GetLogicalDrives`, `GetDriveTypeW`, `GetVolumeInformationW`, `GetDiskFreeSpaceExW` for volumes.
- Win32 registry access through `winreg` for installed applications.

## Repository layout

```text
src/
  App.tsx                     shell, sidebar, view switching
  main.tsx                    React root + SettingsProvider
  components/ui.tsx           Button, Badge, Panel, Drawer, Segmented, Toggle, ...
  components/Icons.tsx        inline SVG icon set
  features/overview/          storage, applications, cleanup, recent activity
  features/uninstaller/       read-only inventory + application drawer
  features/analyzer/          scan controls, Treemap, selection rail
  features/cleaner/           cache inventory and risk explanation (preview only)
  features/activity/          local scan history
  features/settings/          theme, scan scope, exclusions, safety copy
  lib/api.ts                  typed invoke wrapper + cached cache-inventory request
  lib/types.ts                shared IPC contract (camelCase)
  lib/treemap.ts              squarified treemap layout
  lib/categories.ts           extension → category → color
  lib/risk.ts                 safe / caution / protected vocabulary
  lib/settings.tsx            persisted settings context
  lib/hooks.ts                useAsync, useDebouncedValue, useEscape
  styles/tokens.css           light/dark design tokens
  styles/app.css              shell, primitives and shared layout
src-tauri/
  src/apps.rs                 uninstall-registry discovery
  src/fs.rs                   native directory enumeration + std::fs fallback
  src/scan.rs                 bounded scanner and folder measurement
  src/caches.rs               cache catalog, env expansion, measurement
  src/volumes.rs              local volume capacity
  src/history.rs              app-data history persistence
  src/commands.rs             validated Tauri command layer
  src/lib.rs                  state, dialog plugin, command registration
  tests/real_system.rs        live registry/cache/scan integration test
scripts/                      MSVC env shim, build/bundle/binary/smoke checks
```

## Backend modules

| Module | Responsibility |
| --- | --- |
| `apps.rs` | Enumerates `HKLM`/`HKCU` uninstall keys in the 64-bit view and the `WOW6432Node` 32-bit view, parses values, filters non-products, flags protected system locations, deduplicates duplicate registrations and sorts by name. |
| `fs.rs` | Opens a directory with `FILE_LIST_DIRECTORY` + `FILE_FLAG_BACKUP_SEMANTICS` and reads `FILE_ID_BOTH_DIR_INFO` batches. Returns name, attributes, logical size, `AllocationSize`, file id and last-write time. Falls back to `std::fs` when the native path fails or exceeds an 8 MiB listing buffer. |
| `scan.rs` | Depth-first walk with depth/error/stat-output/tree limits. Applies exclusions, hidden/system filtering and reparse skipping; aggregates totals, extension stats and largest files; tracks hard-link ids up to one million; emits throttled progress; honours a cancellation flag; prunes the tree that crosses IPC. `measure_path` reuses the walker without a tree or cancellation. Some per-directory entries/subdirectories and the extension map still scale with input, so memory is not strictly bounded for arbitrarily large directories/scans. |
| `caches.rs` | Fixed catalog of 48 developer, AI, container and Windows cache locations. Expands `%VAR%` segments, resolves paths, measures each existing folder with `scan::measure_path`, deduplicates paths and computes totals per risk level. |
| `volumes.rs` | Lists local fixed/removable volumes with label, file system, capacity and free space. Network and optical drives are skipped deliberately. |
| `history.rs` | Reads/writes `history.json` in the app-data directory, newest first, capped at 100, atomic temp-file replace with a direct-write fallback. Corrupt files read as empty. |
| `commands.rs` | Path/job validation, background execution, scan cancel registry, event emission, history recording, Explorer reveal and measurement commands. |
| `lib.rs` | Registers the dialog plugin, `AppState { scans: Mutex<HashMap<job_id, Arc<AtomicBool>>> }` and the nine commands. |

## IPC surface

All payloads use `#[serde(rename_all = "camelCase")]`; the TypeScript mirror is
`src/lib/types.ts`. Command arguments are camelCase on the JS side and snake_case in Rust.

| Command | Arguments | Returns | Notes |
| --- | --- | --- | --- |
| `list_installed_apps` | — | `AppInventory { apps, warnings }` | Runs on a blocking thread. |
| `list_volumes` | — | `VolumeInfo[]` | Cheap synchronous Win32 queries. |
| `list_caches` | — | `CacheInventory` | Blocking thread. Not written to history. |
| `start_scan` | `jobId`, `path`, `options` | `ScanResult` | Runs on a blocking thread; emits `cyron-scan-progress`. |
| `cancel_scan` | `jobId` | — | Sets the registered flag; unknown ids are ignored. |
| `measure_path` | `path` | `Measured` | Directory only; no tree and no progress events. |
| `reveal_path` | `path` | — | Requires an existing path; opens `explorer.exe` with `/select,` for files. |
| `get_history` | — | `HistoryEntry[]` | Newest first. |
| `clear_history` | — | — | Removes `history.json`; a missing file is not an error. This is the only user-visible deletion in the
current command surface, and it is limited to Cyron's own history file. |

Progress event payload (`cyron-scan-progress`):

```text
{ jobId, files, dirs, bytes, currentDir }
```

It is emitted at most every 120 ms and once more when the scan ends. The UI filters events by the
active job id so a cancelled or superseded scan cannot overwrite the current view.

## Data model

```text
InstalledApp       id, name, version, publisher, installLocation, installDate,
                   uninstallString, quietUninstallString, displayIcon, estimatedSize,
                   hive, scope, architecture, isProtected

ScanOptions        includeHidden, exclusions[], collectTree, maxDepth
ScanNode           name, path, kind(dir|file|group), size, allocated, fileCount,
                   dirCount, modified, children[]
ScanResult         jobId, rootPath, root, totalSize, totalAllocated, fileCount,
                   dirCount, dedupedFiles, skippedReparse, errors[], extensionStats[],
                   largestFiles[], elapsedMs, cancelled, truncated

CacheTarget        id, tool, label, path, exists, size, allocated, fileCount, dirCount,
                   risk(safe|caution|protected), category(ai|ide|packages|containers|windows),
                   note, lastModified
CacheInventory     targets[], totalSize, totalSafe, totalCaution, totalProtected,
                   scannedAt, warnings[]

HistoryEntry       id, kind, title, detail, at, bytes, items,
                   status(completed|cancelled|failed)
```

`size` is logical file length. `allocated` is the allocation from the native directory record;
under the `std::fs` fallback it is approximated using logical size. Native file ids deduplicate
allocated bytes up to the one-million-id tracking cap; fallback entries and later ids beyond that
cap cannot guarantee hard-link deduplication.

## Installed-application discovery

Registry roots read through the 64-bit view (`KEY_WOW64_64KEY`):

- `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`

Each subkey becomes a candidate with `id = "<hive>|<key name>"`. An entry is hidden when it has no
display name, is a system component, has a parent key, is an update/hotfix/security update, starts
with “Update for”, or is only a `KB` number. Duplicate registrations are merged by lowercase name
+ version + normalized install location, preferring `machine-64`, then `machine-32`, then user
entries. `isProtected` is a UI hint for entries under `%WINDIR%` or Windows-branded components; it
does not grant any operation (Phase 1 has none). `EstimatedSize` is read as a DWORD or a string of
kilobytes and reported as bytes; a missing value stays `null` rather than being guessed.

Windows Store/MSIX applications are not enumerated in Phase 1. Application icons are not extracted.

## Filesystem scanner

### Enumeration

`fs::read_dir` opens the directory with `FILE_LIST_DIRECTORY` and shared read/write/delete access,
then calls `GetFileInformationByHandleEx` with `FileIdBothDirectoryInfo`. The first call of each
handle uses `FileIdBothDirectoryRestartInfo`; subsequent calls continue the listing. The 64 KiB
buffer is 8-byte aligned (`Vec<u64>`), grows on `ERROR_MORE_DATA`/`ERROR_BUFFER_OVERFLOW` up to
8 MiB and fails into the `std::fs` fallback beyond that. Each record yields:

- `EndOfFile` as logical size;
- `AllocationSize` as disk allocation (cluster-aware, sparse/compressed aware);
- `FileId` as a stable per-volume identity for hard-link deduplication;
- attributes for directory, hidden, system and reparse checks;
- `LastWriteTime` converted from FILETIME to Unix seconds.

The fallback reports logical size as allocated and provides no file id; it is marked implicitly by
`file_id = None`, so hard links can only be deduplicated on the native path.

### Walk rules

1. Refuse to walk a non-directory and record one error.
2. Skip hidden/system entries when `includeHidden` is false.
3. Never follow a reparse point (junction, symlink, mount point); increment `skippedReparse` and
   record at most the first three paths.
4. Skip paths matching an exclusion. Matching is case-insensitive, separator-normalized, and
   applies to the exact path or any descendant (boundary on `\`).
5. Recurse to `max_depth` (clamped 1–64; default 48). The IPC serializer applies a separate
   display-depth cap of 8.
6. Count every directory visited in `dirCount`; a directory's own `dirCount` excludes itself.
7. Poll the cancellation flag before each directory and each entry. A cancelled scan returns the
   partial tree and `cancelled: true` instead of an error.

### Sizes and hard links

Files increment `fileCount` and logical `size` every time a path is seen. The file id is inserted
into a scan-wide `HashSet<u64>` up to a cap of 1,000,000 ids; only the first occurrence adds
`AllocationSize` to `allocated`. Repeats increment `dedupedFiles`. Once the cap is reached, existing
ids are still recognized but new ids are not retained, one warning is recorded, and later links
with new ids may be counted more than once. Extension size totals use the unique flag while ids are
tracked; extension file counts include every path. Junction targets are never walked. The `std::fs`
fallback has no file id, so hard-link deduplication cannot be guaranteed for those entries.

### Output caps and IPC tree

- At most 40 individual file nodes are retained per directory, plus an aggregate group for the
  smaller file nodes. Directory children are collected separately and are not capped during the
  walk.
- `prune_tree` keeps at most 200 children per directory in the IPC result, keeps the largest,
  aggregates the rest into a `group` node, clears children below depth 8 and sets `truncated` when
  anything is dropped.
- Returned extension statistics are limited to the top 24 and largest files to the top 100 by
  logical size. Totals are computed across the full walk before pruning. The underlying extension
  map and per-directory entry vector are not capped, so the scanner does not guarantee a strict
  upper memory bound for an arbitrarily large scan.

### Progress

`maybe_progress` runs at directory entry and every 512 files within a large single directory; it
is throttled to 120 ms. A final forced event closes the scan. Counters are real walker counters,
not estimates. `elapsedMs` is measured with `Instant`.

### Measurement

`measure_path` uses the same walker without tree construction, progress or cancellation. It backs
the Uninstaller's *Measure folder* action and every cache target measurement.

## Cache catalog

`caches.rs` contains 48 `TargetDef` entries. Each has a stable id, tool, label, env-templated path,
risk, category and a consequence note. Paths use `%USERPROFILE%`, `%LOCALAPPDATA%`, `%APPDATA%`,
`%TEMP%`, `%WINDIR%`. An unset variable skips the entry with a warning (max 12); a resolved but
missing directory is returned with `exists: false` and zero sizes so coverage is visible.

Risk model:

- **Safe** — a cache the owning tool regenerates (renderer caches, logs, package caches).
- **Caution** — removal costs time or a re-download (stores, model-independent caches, workspace
  window state, Windows Update payloads).
- **Protected** — conversation history, model weights, editor local history, settings/prompt state,
  Firefox profiles, Docker volumes and WSL distros. Protected targets are measured and displayed,
  never proposed for cleanup.

Resolved paths are deduplicated case-insensitively and the catalog is written to avoid overlapping
folders, so inventory totals are not double-counted. Totals include existing targets only and are
split by risk. `scanned_at` is Unix seconds.

## Frontend architecture

`App` owns the active view and a `visited` list. Visited views stay mounted but `hidden`, so an
in-progress scan or a populated table survives navigation; there is no router dependency. The
sidebar is collapsible (persisted under `cyron.sidebar.collapsed`), exposes `aria-current`, and
offers a system/light/dark theme switch.

`SettingsProvider` reads `cyron.settings.v1`, writes changes back to localStorage, resolves
`system` through `prefers-color-scheme`, and sets `data-theme` on `<html>`. Settings are:
`theme`, `includeHidden`, `exclusions[]`. They are consumed by the analyzer when it builds
`ScanOptions` and by the Settings page.

`lib/api.ts` is the single IPC boundary. It rejects with a clear message when the app runs outside
Tauri, normalizes errors, and memoizes `list_caches` so Overview and Cleaner share one measurement
pass; `invalidateCaches()` forces the next request to run again.

### Views

- **Overview** joins volumes, application inventory, cache inventory and history. All figures come
  from those responses; loading, error and empty states are explicit.
- **Uninstaller** filters/sorts the registry inventory, shows missing sizes as “—” and opens a
  drawer with metadata, real folder measurement and Explorer reveal.
- **Analyzer** starts from volumes or the native folder picker (`@tauri-apps/plugin-dialog`),
  subscribes to progress events filtered by a generated UUID, supports cancel and cancels on
  unmount. Results are nodes held in a stack for breadcrumbs.
- **Treemap** measures its container with `ResizeObserver`, lays out children with
  `lib/treemap.ts`, and renders absolutely positioned buttons on a fixed dark canvas. File tiles
  are colored by extension category; folder tiles are neutral, aggregate nodes amber. Labels are
  shown only when the tile fits them; click selects, double-click/Enter drills, hover shows the
  share of the parent.
- **Cleaner** groups existing cache targets by category, filters by risk/search, expands rows for
  the full path and consequence note, and separates protected data. There is no removal control.
- **Activity** lists persisted disk scans, with a two-step clear confirmation.
- **Settings** validates exclusions (drive-letter or UNC paths, no wildcards), debounces saving by
  450 ms, and resets to defaults.

The treemap layout is pure TypeScript (`squarify`) with no chart dependency; extension families,
colors and labels live in `lib/categories.ts`. The analyzer rail's **Tree** tab renders the pruned
scan tree (`features/analyzer/TreeView.tsx`) with ARIA tree keyboard navigation, expands the
ancestors of the treemap selection, scrolls it into view and caps visible rows at 500. It is the
summarized IPC tree, not a complete file listing, and a persistent tree column is not implemented.

## Persistence

| Data | Location | Lifetime |
| --- | --- | --- |
| Theme, scan scope, exclusions | `localStorage["cyron.settings.v1"]` | Until cleared by the user/profile reset |
| Sidebar collapsed | `localStorage["cyron.sidebar.collapsed"]` | Same |
| Scan history | `<app_data_dir>/history.json` | Until cleared from Activity or the profile is removed |
| Scan results | Rust memory / React state | Session only |

Only disk scans write history. Cache discovery is a measurement used by Overview and Cleaner and
is deliberately not logged, to avoid an entry every time the app starts.

## Security and safety constraints

- No command deletes scanned/cache/user files, uninstalls applications, terminates processes or
  writes registry values. `clear_history` deletes only Cyron's own history log; history temp-file
  cleanup is likewise limited to a Cyron-created file.
- Command inputs are validated before use: job ids are 1–64 characters of `[A-Za-z0-9_-]`; paths
  must be non-empty, absolute, existing, canonicalized and (for scans) directories. Explorer
  arguments are passed as separate arguments, not interpolated into a shell string.
- Reparse points are skipped during traversal, preventing cycle and mount-point escapes.
- Scan depth, returned child counts, retained per-directory file nodes, largest-file output,
  returned extension stats, errors, tracked file ids and native directory buffer have caps. The
  underlying per-directory entry/subdirectory vectors and extension aggregation map can still grow
  with the input; strict memory bounds for arbitrary directory sizes are not guaranteed.
- The WebView runs with a strict CSP (`default-src 'self'`, IPC allowed, no remote scripts) and
  only the dialog-open capability is granted beyond Tauri core defaults.
- No network API, telemetry, analytics or auto-update code is present.
- The app does not request elevation. Folders the user cannot read are reported as scan errors.

## Performance characteristics and limits

Scanning is single-threaded per request and runs off the UI thread via `spawn_blocking`; the UI only
receives throttled counters and the final pruned tree. Memory includes the pruned-result candidate tree and capped hard-link set, but also an unbounded
per-directory entry vector, directory-child vector and extension map; very large individual
directories or unusually many extensions can therefore consume substantial memory. The native enumeration is designed to avoid a
`stat`/`open` per file, but **no benchmark exists** and no WizTree-level comparison is claimed.
MFT parsing is not implemented; Phase 2 is where performance work and benchmarks belong.

## Testing and CI

- `src/lib/*.test.ts` — treemap geometry (area, bounds, overlap), formatting, category mapping and
  risk vocabulary: 32 tests (latest reported `npm test` run).
- Rust unit tests — registry filtering/deduplication/date and size parsing, native enumeration,
  hard-link dedupe, exclusions, depth, cancellation, progress, stats, cache catalog validity and
  protection rules, volume sanity, history ordering, command validation. Latest reported cargo run:
  38 passed, 1 ignored; the real-system integration test also passed.
- `src-tauri/tests/real_system.rs` — live registry count, live cache targets, a temporary fixture
  tree with exact byte assertions.
- `.github/workflows/ci.yml` — Windows CI for typecheck, Vitest, frontend build and `cargo test`.
- `scripts/verify-dist.mjs`, `verify-desktop-binary.mjs`, `smoke-desktop.mjs` — artefact checks used
  by the Phase 1 acceptance ledger.

## Deferred design work

- MFT-based NTFS scanning with a safe fallback, plus measured benchmarks.
- Folder tree view synchronized with the treemap (current UI synchronizes selection and
  breadcrumbs, not a full column tree).
- Duplicate detection with staged hash verification.
- Cleanup previews, deletion, rollback and operation logs (Phase 3).
- Standard/advanced/force uninstall, scheduled uninstall queue, leftover registry/file detection,
  process scanning and controlled termination (Phase 4).
- MSIX/Store inventory, portable app detection, code signing and packaged installers (later).
