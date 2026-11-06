# Cyron — Architecture

## Purpose and boundary

Cyron is a Tauri v2 desktop application for Windows. Scanning is read-only: the backend enumerates
applications, measures folders, scans a disk tree, verifies duplicate files and reports storage
categories and volumes. Cleaning is a separate, review-first path: the backend can move
catalog cache files into a recoverable quarantine after a preview and an explicit plan confirmation.
Phase 4 adds standard application uninstallation, critical-process-guarded process management,
leftover detection, automated registry backups, and sequential batch uninstallation.

The product target is a Revo Uninstaller + BleachBit + WinDirStat replacement with better risk
communication for non-technical users.

## Runtime stack

| Part | Implementation |
| --- | --- |
| Window/shell | Tauri v2, one native window (`main`), 1320×860, minimum 1000×680 |
| Frontend | React 19 + TypeScript, Vite 7, no router |
| Backend | Rust, `tauri`, `tauri-plugin-dialog`, `winreg`, `windows-sys`, `sha2` |
| Storage | localStorage (preferences, tree width), `history.json`, `cleanup-log.json`, `recovery_journal.jsonl` and `quarantine/` in the Tauri app-data dir |
| Frontend tests | Vitest, 71 tests in 7 files |
| Backend tests | `cargo test`: 171 passed (134 unit, 25 security, 5 cleanup-stress, 1 real-system, 6 scanner-stress), 3 ignored |

Native APIs used by the backend:

- `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)` for directory enumeration.
- Raw volume reads (`\\.\X:`) plus `DeviceIoControl`/`FSCTL_GET_NTFS_VOLUME_DATA`-style metadata for
  MFT scanning.
- `GetLogicalDrives`, `GetDriveTypeW`, `GetVolumeInformationW`, `GetDiskFreeSpaceExW` for volumes.
- Win32 registry access through `winreg` for installed applications and leftover cleaning.
- `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` for atomic no-overwrite restoration.
- `CreateToolhelp32Snapshot`, `Process32FirstW`, `Process32NextW`, `QueryFullProcessImageNameW`, `OpenProcess`, `TerminateProcess`, `EnumWindows`, `PostMessageW(WM_CLOSE)` for process management.
- `GetTokenInformation` for an elevation check (the app never requests elevation).

## Repository layout

```text
src/
  App.tsx                     shell, sidebar, view switching
  main.tsx                    React root + SettingsProvider
  components/ui.tsx           Button, Badge, Panel, Drawer, Segmented, Toggle, ...
  components/Icons.tsx        inline SVG icon set
  features/overview/          storage, applications, cleanup, recent activity
  features/uninstaller/       multi-source application inventory, uninstaller, batch queue, process manager & residual drawer
  features/analyzer/          scan controls, treemap, tree, filters, duplicates, storage
  features/cleaner/           review-first cache cleaner, preview drawer, execution, quarantine, restore & purge
  features/activity/          local scan history
  features/settings/          theme, scan scope, exclusions, safety copy
  lib/api.ts                  typed invoke wrapper + cached cache-inventory request
  lib/types.ts                shared IPC contract (camelCase)
  lib/analyzerFilters.ts      tree/file filtering, sorting and filtered totals
  lib/treemap.ts              squarified treemap layout
  lib/categories.ts           extension → category → color
  lib/risk.ts                 safe / caution / protected vocabulary
  lib/settings.tsx            persisted settings context
  lib/hooks.ts                useAsync, useDebouncedValue, useEscape
  styles/tokens.css           light/dark design tokens
  styles/app.css              shell, primitives and shared layout
src-tauri/
  src/apps.rs                 multi-source application discovery (Win32, Store/MSIX, Steam, Portable)
  src/fs.rs                   native directory enumeration + std::fs fallback
  src/mft.rs                  NTFS MFT reader and subtree construction
  src/scan.rs                 streaming walker, method selection, aggregation
  src/duplicates.rs           size buckets, partial + full hashing
  src/storage.rs              categories, age buckets, insights
  src/caches.rs               cache catalog, env expansion, measurement
  src/cleanup.rs              cleanup guard, preview, quarantine, recovery journal, restore, purge, log
  src/process.rs              process discovery, critical-process protection, controlled termination
  src/uninstall.rs            safe command parsing, uninstaller execution, exit code tracking
  src/leftovers.rs            leftover scanning, registry backup, residual quarantine
  src/explorer.rs             Windows Explorer right-click shell verbs, CLI routing, path validation
  src/providers.rs            declarative cleanup provider system, schema validator, safety enforcement
  src/volumes.rs              local volume capacity
  src/history.rs              app-data history persistence
  src/commands.rs             validated Tauri command layer (39 commands)
  src/lib.rs                  state, dialog plugin, command registration
  tests/real_system.rs        live registry/cache/scan integration test
  tests/security.rs           permission boundaries, protected paths, links, fixtures
  tests/cleanup_stress.rs     locked files, cancellation, wide cleanup fixtures
  tests/stress_scan.rs        wide/deep/denied/cancellation stress tests
  examples/scan_bench.rs      benchmark binary used by scripts/bench-scan.mjs
  examples/cleanup_fixture.rs isolated end-to-end cleanup verification
  examples/rc1_e2e_fixture.rs comprehensive uninstaller, process, and residual e2e fixture
scripts/                      MSVC env shim, build/bundle/binary/smoke/benchmark checks
```

## Backend modules

| Module | Responsibility |
| --- | --- |
| `apps.rs` | Enumerates `HKLM`/`HKCU` uninstall keys (64-bit and 32-bit views), Microsoft Store packages (`AppModel\Repository\Packages`), Steam libraries (`libraryfolders.vdf`), and bounded portable binaries (`%LOCALAPPDATA%\Programs`). Identifies broken uninstallers and provides source tags. |
| `fs.rs` | Opens a directory with `FILE_LIST_DIRECTORY` + `FILE_FLAG_BACKUP_SEMANTICS` and reads `FILE_ID_BOTH_DIR_INFO` batches. Returns name, attributes, logical size, `AllocationSize`, file id and last-write time. Falls back to `std::fs` when the native path fails or exceeds an 8 MiB listing buffer. |
| `mft.rs` | Reads the NTFS boot sector and `$MFT` run list, validates record counts against a derived memory budget, applies update-sequence fixups, parses `FILE_NAME`/`$DATA`/`$STANDARD_INFORMATION`, deduplicates hard links with a record bitset, and builds the requested subtree. Cancellation and progress are polled during parsing. Any failure returns a fallback reason. |
| `scan.rs` | Chooses the scan method, then runs the streaming directory walk: depth/error/stat-output/tree caps, exclusions, hidden/system filtering, reparse skipping, totals, extension stats, largest files, hard-link tracking, throttled progress and cancellation. Builds `StorageReport` alongside the tree. `measure_path` reuses the walker without a tree or cancellation. |
| `duplicates.rs` | Walks a root for files at or above `minSize`, groups by size, confirms groups with 4 KiB head/tail SHA-256, then full streaming SHA-256. Tracks hard links by file id, polls cancellation, emits `cyron-duplicate-progress`, caps candidates/groups/files and reports truncation. |
| `storage.rs` | Classifies bytes into ten categories by known root and extension, marks known vs inferred confidence, buckets files by age, and derives up to six explained insights (large, old, developer-heavy). |
| `caches.rs` | Fixed catalog of 48 developer, AI, container and Windows cache locations. Expands `%VAR%` segments, resolves paths, measures each existing folder with `scan::measure_path`, deduplicates paths and computes totals per risk level. |
| `providers.rs` | Declarative versioned (`schema_version: 1`) cleanup provider system. Enforces approved environment roots, validates path templates and patterns, rejects path traversals, wildcards in roots, and attempts to mark protected data safe. |
| `cleanup.rs` | The primary cache cleanup engine. Resolves catalog ids to specs, enforces `guard_path`, plans and measures a preview, executes a server-held plan by moving files into a quarantine batch, and implements list/restore/purge plus the capped cleanup log. |
| `explorer.rs` | Registers and unregisters native per-user Windows shell context menu verbs under `HKEY_CURRENT_USER\Software\Classes` without administrator elevation. Parses startup CLI arguments (`--analyze`, `--duplicates`, `--storage`, `--uninstall-target`, `--find-related`) and sanitizes paths. |
| `process.rs` | Discovers processes executing from an application root, snapshots process state with 64-bit creation times (`GetProcessTimes`) to defeat PID recycling, denies Windows critical processes, and executes graceful `WM_CLOSE` preceding `TerminateProcess`. |
| `uninstall.rs` | Resolves vendor commands server-side from live registry keys, creates expiring single-use plans (`UninstallPlan`, `BatchUninstallPlan`), tokenizes commands safely, normalizes MSI invocations, and tracks exit codes (including code 3010 reboot requests). |
| `leftovers.rs` | Scans likely residual files and registry keys under AppData, ProgramData, and HKCU/HKLM. Enforces root guards, executes recursive multi-type `.reg` backups, verifies backup integrity, and cleans residual files into quarantine. |
| `volumes.rs` | Lists local fixed/removable volumes with label, file system, capacity and free space. Network and optical drives are skipped deliberately. |
| `history.rs` | Reads/writes `history.json` in the app-data directory, newest first, capped at 100, atomic temp-file replace with a direct-write fallback. Corrupt files read as empty. |
| `commands.rs` | Path/job/plan validation, background execution, cancellation registries, single-use plan registries, event emission, history and log recording, Explorer reveal, and 34 validated IPC commands. |
| `lib.rs` | Registers the dialog plugin, `AppState`, startup route resolution, and all 34 Tauri commands. `read_only` starts `true`. |

## IPC surface

All payloads use `#[serde(rename_all = "camelCase")]`; the TypeScript mirror is
`src/lib/types.ts`. Command arguments are camelCase on the JS side and snake_case in Rust.

| Command | Arguments | Returns | Notes |
| --- | --- | --- | --- |
| `list_installed_apps` | — | `AppInventory { apps, warnings }` | Runs on a blocking thread. |
| `list_volumes` | — | `VolumeInfo[]` | Cheap synchronous Win32 queries. |
| `list_caches` | — | `CacheInventory` | Blocking thread. Not written to history. |
| `start_scan` | `jobId`, `path`, `options` | `ScanResult` | Blocking thread; emits `cyron-scan-progress`. `options.method` is `auto`, `mft` or `directory`. |
| `cancel_scan` | `jobId` | — | Sets the registered flag; unknown ids are ignored. |
| `start_duplicate_scan` | `jobId`, `path`, `options` | `DuplicateReport` | Blocking thread; emits `cyron-duplicate-progress`. Options: `minSize`, `includeHidden`, `exclusions[]`. |
| `cancel_duplicate_scan` | `jobId` | — | Sets the registered duplicate flag. |
| `measure_path` | `path` | `Measured` | Directory only; no tree and no progress events. |
| `reveal_path` | `path` | — | Requires an existing path; opens `explorer.exe` with `/select,` for files. |
| `get_history` | — | `HistoryEntry[]` | Newest first. |
| `clear_history` | — | — | Removes `history.json`; a missing file is not an error. Limited to Cyron's own history file. |
| `get_cleanup_mode` | — | `bool` | `true` means read-only. |
| `set_cleanup_mode` | `readOnly` | `bool` | Flips the backend switch; the UI persists the same value in settings. |
| `preview_cleanup` | `targetIds[]`, `exclusions[]?` | `CleanupPreview` | Blocking thread. Ids only; protected/unknown ids are refused; exclusions are validated as absolute non-root paths and stored in the plan. |
| `start_cleanup` | `planId`, `targetIds[]` | `CleanupReport` | Consumes the plan once, re-checks the selection, the 10-minute TTL and read-only mode; emits `cyron-cleanup-progress`; logs the run and appends an Activity entry. |
| `list_quarantine` | — | `QuarantineBatch[]` | Batches that still hold restorable files, newest first. |
| `restore_quarantine` | `runId` | `RestoreReport` | Byte-exact restore, never overwrites; retires the batch when everything is back; logged. |
| `purge_quarantine` | `runId` | `PurgeReport` | Permanently deletes only the files in that batch manifest that are still inside the batch folder; logged. |
| `get_cleanup_log` | — | `CleanupLogEntry[]` | Newest first, max 100. |

All three registries reject a job id that is already running instead of replacing its cancellation
flag, so a cancel call cannot target a flag that no longer controls the running task. Cleanup plan
ids are opaque, single-use and swept when expired.

Progress event payloads:

```text
cyron-scan-progress       { jobId, files, dirs, bytes, currentDir }
cyron-duplicate-progress   { jobId, phase(index|partial|full), files, bytesHashed, currentPath }
```

Scan progress is emitted at most every 120 ms and once more when the scan ends; duplicate progress
is throttled to 150 ms. The UI filters events by the active job id so a cancelled or superseded run
cannot overwrite the current view.

## Data model

```text
InstalledApp       id, name, version, publisher, installLocation, installDate,
                   uninstallString, quietUninstallString, displayIcon, estimatedSize,
                   hive, scope, architecture, isProtected

ScanOptions        includeHidden, exclusions[], collectTree, maxDepth, method
ScanNode           name, path, kind(dir|file|group), size, allocated, fileCount,
                   dirCount, modified, children[]
ScanResult         jobId, rootPath, root, totalSize, totalAllocated, fileCount,
                   dirCount, dedupedFiles, skippedReparse, errors[], extensionStats[],
                   largestFiles[], outcome, method, methodNote, storage,
                   elapsedMs, cancelled, truncated
StorageReport      categories[]{key,label,detail,bytes,fileCount,known}, ageBuckets[],
                   insights[]{kind,title,detail,bytes,fileCount,samplePath}
ScanMethod         auto | mft | directory

DuplicateOptions   minSize, includeHidden, exclusions[]
DuplicateGroup     id, size, wasted, files[]{path,size,allocated,modified,hardLink}
DuplicateReport    jobId, rootPath, groups[], totalGroups, totalFiles, totalWasted,
                   candidateFiles, hashedFiles, scannedFiles, errors[], elapsedMs,
                   cancelled, truncated

CacheTarget        id, tool, label, path, exists, size, allocated, fileCount, dirCount,
                   risk(safe|caution|protected), category(ai|ide|packages|containers|windows),
                   note, lastModified
CacheInventory     targets[], totalSize, totalSafe, totalCaution, totalProtected,
                   scannedAt, warnings[]

HistoryEntry       id, kind, title, detail, at, bytes, items,
                   status(completed|cancelled|failed)

CleanupSpec        id, tool, label, risk, category, note, impact, path      (backend only)
CleanupPlan        planId, specs[], expiresAt, readOnly, exclusions[], expectedFiles,
                   expectedBytes                                          (backend only)
CleanupPreview     planId, generatedAt, expiresAt, targets[], files[], skipped[],
                   fileCount, bytes, totalFiles, readOnly, hiddenCount, warnings[],
                   truncated
CleanupTarget      id, tool, label, path, note, impact, risk, category, exists,
                   size, fileCount, dirCount, includable
CleanupReport      runId, planId, bytes, files, dirsRemoved, quarantined, skipped,
                   failed, items[], errors[], elapsedMs, cancelled, quarantineDir,
                   purgeAt
CleanupProgress    runId, files, totalFiles, bytes, currentPath
QuarantineBatch    runId, createdAt, expiresAt, bytes, fileCount, dir, targetIds[]
RestoreReport      runId, restored, skipped, failed, errors[]
PurgeReport        runId, removedFiles, freedBytes
CleanupLogEntry    id, at, title, targets[], bytes, files, quarantined, skipped,
                   failed, status(completed|cancelled|failed), detail
```

`size` is logical file length. `allocated` is the allocation from the native directory record or
MFT `$DATA`; under the `std::fs` fallback it is approximated using logical size. Native file ids
deduplicate allocated bytes up to the one-million-id tracking cap; the MFT reader counts each
record once.

## Scan methods

### Selection

`method: auto` selects MFT only when the root is a drive root (`C:\`, not `C:\Windows`). MFT reads
the volume's whole record set, so subfolder scans are measurably slower with it (System32: 0.28 s
walk vs 3.91 s MFT). An explicit `mft` request is honoured and falls back with a reason when the
volume cannot be read. `method`, `methodNote` and `outcome` in `ScanResult` always describe what
actually ran.

### MFT reader

1. Open `\\.\X:` with read access. Failure (typically not elevated) returns a fallback reason.
2. Read the NTFS boot sector for bytes/sector, sectors/cluster and the `$MFT` cluster.
3. Read record 0, parse its `$DATA` run list, and read the `$MFT` record count from the volume.
4. Reject a record count above the derived budget (768 MiB / per-record cost = 4,910,404 records)
   **before** allocating, then allocate once.
5. Stream records through one fixed 1,024-byte buffer, applying the update-sequence fixup with a
   fixed 16-slot stack array, and parse file name, data and standard-information attributes.
6. Mark seen records in a bitset so hard links do not inflate totals; retain at most 200 children
   per directory and 100,000 tree nodes.
7. Accumulate the requested subtree depth-first, then emit the same `ScanResult` shape as the walk.

Cancellation is polled every 4,096 records and during subtree accumulation. A cancelled MFT scan
returns a cancelled result rather than an error.

### Directory walk

`fs::read_dir` opens the directory with `FILE_LIST_DIRECTORY` and shared read/write/delete access,
then calls `GetFileInformationByHandleEx` with `FileIdBothDirectoryInfo`. The first call of each
handle uses `FileIdBothDirectoryRestartInfo`; subsequent calls continue the listing. The 64 KiB
buffer is 8-byte aligned (`Vec<u64>`), grows on `ERROR_MORE_DATA`/`ERROR_BUFFER_OVERFLOW` up to
8 MiB and fails into the `std::fs` fallback beyond that.

Walk rules:

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
8. Stream entries: each directory's entries are processed as they are read, not collected into a
   vector first.

### Sizes, hard links and bounds

Files increment `fileCount` and logical `size` every time a path is seen. The file id is inserted
into a scan-wide set up to 1,000,000 ids; only the first occurrence adds allocation to
`allocated`, and repeats increment `dedupedFiles`. Once the cap is reached, existing ids are still
recognized, one warning is recorded, and later links with new ids may count more than once. The
`std::fs` fallback has no file id and cannot dedupe.

The walk retains at most 40 individual file nodes per folder plus an aggregate group, 200 children
per folder in the IPC tree, 100,000 nodes in total, 4,096 extension keys (with an overflow bucket
for the rest), the top 24 extension stats and the top 100 largest files. Aggregate totals cover the
whole walk.

### Progress and measurement

`maybe_progress` runs at directory entry and every 512 files within a large single directory; it
is throttled to 120 ms. A final forced event closes the scan. Counters are real walker counters,
not estimates. `elapsedMs` is measured with `Instant`. `measure_path` uses the walker without tree
construction, progress or cancellation and backs *Measure folder* plus every cache measurement.

## Duplicate verification

1. Walk the root (same exclusion, hidden and reparse rules), collecting files at or above the
   minimum size (default 1 MiB), capped at 1,000,000 candidates.
2. Group candidates by exact byte size; size-unique files are dropped without a read.
3. For each size group with more than one file, hash the first and last 4 KiB (plus a full hash for
   files smaller than 8 KiB). This separates most same-size files cheaply.
4. Full-content streaming SHA-256 (1 MiB buffer) confirms each remaining subgroup.
5. Emit groups with `wasted = size × (non-hard-link copies − 1)`. Hard links are listed but do not
   add recoverable bytes. A file that changes size or timestamp during hashing is reported rather
   than trusted.
6. Caps: 200 groups and 50 files per group in the report, with `truncated: true`. Cancellation
   returns what was proven so far and `cancelled: true`.

The duplicate report is read-only; duplicate management is deferred to a later phase.

## Cleanup engine

The Cleaner is the only feature that modifies user files, and it is deliberately narrow.

### Eligibility

1. `preview_cleanup` accepts catalog **ids**. `catalog_specs` looks each one up in
   `caches::CATALOG`, de-duplicates, refuses unknown ids, refuses `protected` ids and resolves the
   templated path. Frontend-provided paths are never accepted.
2. `plan_specs_with_exclusions` refuses an empty selection, runs `guard_path` on every spec, and
   validates the caller's exclusions (absolute, non-root, at most 128, de-duplicated).
3. `guard_path` refuses a relative path, anything with fewer than three path components (drive roots
   and top-level folders), any of the environment roots themselves (`USERPROFILE`, `LOCALAPPDATA`,
   `APPDATA`, `WINDIR`, `SystemDrive`, `ProgramFiles`, `ProgramFiles(x86)`) and any path that
   **contains** a protected catalog location. It is a floor, not the whole model: the real allow
   list is the catalog itself, and the guard runs again for each target at execution time.

### Preview

`walk_target(path, 100_000, remaining, None, exclusions)` measures the location with the same
streaming walker the analyzer uses, so preview totals and the measured inventory agree. It returns
exact file counts and bytes, up to 400 listed paths, up to 100 skip reasons (link, junction, excluded
in Settings, unreadable metadata), bounded errors, and a warning when statistics are partial. The
preview is stored alongside a `CleanupPlan` in `AppState::plans` under an opaque id and expires after
600 s.

### Execution

1. `start_cleanup` validates the plan id, pops the plan exactly once (expired plans are swept),
   re-checks that the selection is a subset of the plan, and re-checks read-only mode.
2. For each spec: re-run `guard_path`, walk the location again, and create the batch folder only if
   there is something to move. Anything the second walk refuses — a target that became a junction, a
   subfolder swapped for a link after the preview — is added to the report as a warning, so the run
   never silently cleans nothing.
3. For each file: `symlink_metadata` (must be a regular file, no reparse attribute), canonicalize the
   parent and require it to be inside the target root, then `fs::rename` into
   `<quarantine>\<run-id>\<index>-<name>`. Rename means a file on another volume or held without
   delete sharing fails instead of being copied and truncated.
4. The manifest is appended per move and flushed every 256 items, so an interrupted run is still
   restorable. Progress is emitted at most every 120 ms and once at the end.
5. Empty directories are removed one at a time with `remove_dir` (never recursive), deepest first.
6. Cancellation is polled per target and per file; the report says `cancelled: true` and accounts
   only for what actually moved.

### Quarantine

A batch is `<app_data_dir>\quarantine\<run-id>\` with `manifest.json` (run id, timestamps, target
ids, items with original path, stored path and size). `restore_batch` moves each item back, refuses
to overwrite an existing file, recreates missing parents, keeps failed items in the manifest and
deletes the batch when everything is restored. `purge_batch` deletes only manifest items whose
canonical path is still inside the batch, then removes the manifest and the (now empty) folder. The
7-day `expiresAt` is informational: nothing expires automatically.

### Log

`cleanup-log.json` keeps the newest 100 run/restore/purge entries (id, time, title, target ids,
bytes, files, counts, status, one-line detail). Each cleanup run also appends an Activity history
entry. File contents and paths of individual files are not logged.

## Storage intelligence

`storage.rs` classifies every scanned byte into one of ten fixed keys (applications, games,
downloads, documents, media, system, developer, ai, caches, other). Classification uses the path
relative to the scan root and known folder names (`Program Files`, `SteamLibrary`, `Downloads`,
`Documents`, `.cargo`, `.ollama`, `AppData\Local\Docker`, etc.) plus extension families. Each
category carries `known: bool`: false when any bytes were inferred rather than matched to a known
location. Age buckets are 0–30, 31–90, 91–365, 365+ and unknown. Insights highlight large files,
old files, developer-heavy folders and AI/model storage with a size and a sample path. Category
descriptions come from `category_meta`, so the UI never shows a category without an explanation of
what it contains.

## Installed-application discovery

Registry roots read through the 64-bit view (`KEY_WOW64_64KEY`):

- `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`

Each subkey becomes a candidate with `id = "<hive>|<key name>"`. An entry is hidden when it has no
display name, is a system component, has a parent key, is an update/hotfix/security update, starts
with "Update for", or is only a `KB` number. Duplicate registrations are merged by lowercase name
+ version + normalized install location, preferring `machine-64`, then `machine-32`, then user
entries. `isProtected` is a UI hint for entries under `%WINDIR%` or Windows-branded components; it
does not grant any operation. `EstimatedSize` is read as a DWORD or a string of kilobytes and
reported as bytes; a missing value stays `null` rather than being guessed.

Windows Store/MSIX applications are enumerated from `AppModel\Repository\Packages` as a distinct inventory source with read-only lifecycle indicators. Application icons are not extracted.

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
`theme`, `includeHidden`, `exclusions[]`, `readOnly`. Exclusions feed both `ScanOptions` and the
cleanup preview; `readOnly` is pushed to the backend with `set_cleanup_mode` so the UI and the
backend switch always agree (the backend also defaults to read-only on its own).

`lib/api.ts` is the single IPC boundary. It rejects with a clear message when the app runs outside
Tauri, normalizes errors, and memoizes `list_caches` so Overview and Cleaner share one measurement
pass; `invalidateCaches()` forces the next request to run again after a cleanup changes files.

### Views

- **Overview** joins volumes, application inventory, cache inventory and history. All figures come
  from those responses; loading, error and empty states are explicit.
- **Uninstaller** filters/sorts the registry inventory, shows missing sizes as "—" and opens a
  drawer with metadata, real folder measurement and Explorer reveal.
- **Analyzer** starts from volumes or the native folder picker, subscribes to progress events
  filtered by a generated job id, supports cancel and cancels on unmount. Results are nodes held in
  a stack for breadcrumbs. Three modes: **Map**, **Duplicates**, **Storage**.
- **Treemap** measures its container with `ResizeObserver`, lays out children with
  `lib/treemap.ts`, and renders absolutely positioned buttons on a fixed dark canvas. File tiles
  are colored by extension category; folder tiles are neutral, aggregate nodes amber. Click
  selects, double-click/Enter drills, hover shows the share of the parent. Selection is shared with
  the tree and details.
- **Tree column** (`TreeView.tsx`) is a persistent, resizable (220–520 px, persisted) column with
  virtualized rows, ARIA tree keyboard navigation, expand/collapse, scroll-to-selection and reset
  when the scan root changes. It renders the pruned IPC tree, not a complete file listing.
- **Filters** (`AnalyzerFilters.tsx`, `lib/analyzerFilters.ts`) filter by name, minimum size, age,
  extension and type, and sort by size, name, files or modified date. Filtered totals are labelled
  as such.
- **Duplicates panel** cancels its job and resets when the scan scope changes, so an old folder's
  report cannot be shown as the new folder's result.
- **Storage panel** presents categories, confidence, age buckets and insights, and states that
  observations are not removal guidance.
- **Cleaner** groups cache targets into Windows / Applications / Developer tools / AI tools,
  filters by risk, category and search, expands rows for path, impact and dates, and separates
  protected data into a read-only section. Selecting targets opens a preview drawer that lists the
  planned files (capped at 400 with an explicit note), skipped items, warnings and the quarantine
  promise; a confirmation checkbox enables the run. Progress comes from `cyron-cleanup-progress`,
  results are per item, and the Quarantine and History tabs drive restore, purge and the run log.
- **Activity** lists persisted disk scans, with a two-step clear confirmation that Escape cancels.
- **Settings** validates exclusions (drive-letter or UNC paths, no wildcards), debounces saving by
  450 ms, and resets to defaults.

The treemap layout is pure TypeScript (`squarify`) with no chart dependency; extension families,
colors and labels live in `lib/categories.ts`.

## Store/MSIX inventory investigation and compatibility notes

Windows Store, UWP, and modern packaged applications (MSIX) do not write standard Win32 uninstall keys under `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`. Instead, their metadata is stored in:
- `HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages`
- `HKLM\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages`

### Technical Findings:
1. **Discovery**: Read-only inventory discovery of installed packages can enumerate these `AppModel\Repository\Packages` subkeys to extract package full names, display names, and install paths.
2. **Uninstallation Boundary**: Unlike standard Win32 applications, MSIX packages have no `UninstallString` or MSI product code. Uninstallation requires invoking Windows Runtime COM interfaces: `Windows.Management.Deployment.PackageManager::RemovePackageAsync`.
3. **Security Constraints**: Invoking `powershell.exe -Command Remove-AppxPackage` is strictly refused because Cyron forbids spawning shell interpreters or running arbitrary scripts. A native WinRT COM projection (using the `windows` crate) is required to remove MSIX packages safely without shell wrappers.
4. **Implementation**: Store/MSIX packages are enumerated in `apps.rs` with source metadata, install paths, and residual detection. Because safe removal requires Windows Runtime COM package management interfaces, traditional Win32 uninstallation is disabled with clear guidance directing users to Windows Settings or `winget`.

## Persistence

| Data | Location | Lifetime |
| --- | --- | --- |
| Theme, scan scope, exclusions | `localStorage["cyron.settings.v1"]` | Until cleared by the user/profile reset |
| Sidebar collapsed | `localStorage["cyron.sidebar.collapsed"]` | Same |
| Tree column width | `localStorage["cyron.analyzer.treeWidth"]` | Same |
| Scan history | `<app_data_dir>/history.json` | Until cleared from Activity or the profile is removed |
| Quarantine batches | `<app_data_dir>/quarantine/<run-id>/` | Until restored and retired or purged by the user |
| Recovery journal | `<quarantine>/<run-id>/recovery_journal.jsonl` | Append-only per-move write-ahead log |
| Cleanup log | `<app_data_dir>/cleanup-log.json` | Newest 100 entries |
| Scan and duplicate results | Rust memory / React state | Session only |
| Pending cleanup plans | Rust memory (`AppState::plans`) | 10 minutes or until executed |
| Pending uninstall plans | Rust memory (`AppState::uninstall_plans`) | 10 minutes or until executed |
| Process snapshots | Rust memory (`AppState::process_snapshots`) | 5 minutes or until refreshed |
| Pending leftover plans | Rust memory (`AppState::leftover_plans`) | 10 minutes or until executed |

Disk scans, cleanup runs, restores and purges write history; cache discovery and duplicate runs are
measurements and are not logged.

## Security and safety constraints

- **File modifications** are strictly confined to `cleanup.rs` and `leftovers.rs` through review-first plans, quarantine batches, write-ahead recovery journals, and atomic no-overwrite restoration.
- **Process management** in `process.rs` verifies executable ownership, creation time (`GetProcessTimes`), and PID identity against a recent snapshot. Critical Windows system processes and Windows directory binaries can never be terminated.
- **Uninstaller execution** in `uninstall.rs` validates vendor command lines, normalizes MSI product codes, and rejects shell wrappers (`cmd.exe`, `powershell.exe`, `.bat`, etc.). Executables and arguments are resolved server-side from registered keys; caller-supplied executables are never launched.
- **Registry modifications** in `leftovers.rs` are guarded against shared vendor keys and system roots, and require an automatic multi-type recursive `.reg` backup before deleting any subkey.
- Command inputs are validated before use: job ids are 1–64 characters of `[A-Za-z0-9_-]`; paths must be non-empty, absolute, existing, canonicalized and directories.
- Reparse points are skipped during traversal, preventing cycle and mount-point escapes.
- Scan depth, returned child counts, retained nodes, file nodes, largest-file output, extension stats and keys, errors, tracked file ids, the native directory buffer and the MFT record budget are all capped.
- The WebView runs with a strict CSP (`default-src 'self'`, IPC allowed, no remote scripts) and only the dialog-open capability is granted beyond Tauri core defaults.
- No network API, telemetry, analytics or auto-update code is present.
- The app does not request elevation. Without it, MFT is unavailable, the walk reports unreadable folders, and locked cache files are skipped instead of forced.
- Cleanup starts in read-only mode.

## Performance characteristics and limits

Scans run off the UI thread via `spawn_blocking`; the UI receives throttled counters and the final
pruned result. Measured on the development machine (warm cache, one run):

| Target | Method | Time | Peak working set |
| --- | --- | ---: | ---: |
| `C:\` | MFT (elevated) | 14.52 s | 655.5 MiB |
| `C:\` | directory walk | 155.24 s | 50.8 MiB |
| `C:\Windows\System32` | MFT (elevated, whole-volume filter) | 3.91 s | 571.9 MiB |
| `C:\Windows\System32` | directory walk | 0.28 s | 7.8 MiB |

MFT trades memory for speed and only pays off at a drive root. The walk is streaming and bounded
but slower; neither is a WizTree comparison. Details and caveats are in
[docs/BENCHMARKS.md](BENCHMARKS.md).

## Testing and CI

- `src/lib/*.test.ts` — Frontend unit test suites covering formatting, categories, risk ratings, analyzer filters, cleanup selection rules, treemap geometry, and Tauri IPC contracts.
- Rust unit tests — Tests covering registry discovery, directory traversal, MFT parsing, duplicate detection, storage categorization, cache catalog validity, quarantine roundtrip, uninstaller tokenization, process filters, and recursive registry backup/restore.
- `tests/real_system.rs` — live registry count, live cache targets, a temporary fixture tree with exact byte assertions.
- `tests/security.rs` — Architectural boundary and security tests: module boundaries for destructive APIs, protected-path refusal, hostile IDs, junction/symlink swaps, shell rejection, process termination boundaries, and registry backup verification.
- `tests/cleanup_stress.rs` — wide cleanup fixture with a bounded preview, locked file, cancelled run, consecutive batches and empty/missing targets.
- `examples/cleanup_fixture.rs` + `scripts/verify-cleanup.mjs` — end-to-end plan → quarantine → restore → purge against `%TEMP%\cyron-cleanup-<pid>`, printing `CLEANUP_FIXTURE_OK`.
- `tests/stress_scan.rs` — 20,000-file wide directory with exact totals and bounded children, deep nesting, access-denied folders, exclusion subtree and cancellation.
- `.github/workflows/ci.yml` & `release.yml` — Windows CI for typecheck, Vitest, frontend build, `cargo test`, security and cleanup-stress suites, release packaging, checksum generation, and GitHub release automation.
- `scripts/verify-dist.mjs`, `verify-desktop-binary.mjs`, `smoke-desktop.mjs`, `verify-release-bundles.mjs`, `bench-scan.mjs` — artefact verification, desktop launch smoke checks, and release bundle verification.

## Deferred design work

- MFT scanning for volumes above the record budget (e.g. streaming aggregation without retaining all records).
- Native WinRT COM bindings for Store/MSIX package removal without shell scripts.
- Cold-cache benchmark methodology and multi-run statistics published from CI hardware.
