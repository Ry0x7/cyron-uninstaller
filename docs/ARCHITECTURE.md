# Cyron — Architecture

## Purpose and boundary

Cyron is a Tauri v2 desktop application for Windows. Scanning is read-only: the backend enumerates
applications, measures folders, scans a disk tree, verifies duplicate files and reports storage
categories and volumes. Cleaning is a separate, review-first path: the backend can move
catalog cache files into a recoverable quarantine after a preview and an explicit plan confirmation.
Phase 4 adds standard application uninstallation, critical-process-guarded process management,
leftover detection, automated registry backups, and sequential batch uninstallation.

The product target is a tool combining ideas from Revo Uninstaller, BleachBit and WinDirStat with better risk
communication for non-technical users.

## Runtime stack

| Part | Implementation |
| --- | --- |
| Window/shell | Tauri v2, one native window (`main`), 1320×860, minimum 1000×680 |
| Frontend | React 19 + TypeScript, Vite 7, no router |
| Backend | Rust, `tauri`, `tauri-plugin-dialog`, `tauri-plugin-updater`, `tauri-plugin-process`, `winreg`, `windows-sys`, `sha2` |
| Storage | localStorage (preferences, tree width), `history.json`, `cleanup-log.json`, `recovery_journal.jsonl` and `quarantine/` in the Tauri app-data dir |
| Frontend tests | Vitest, 120 tests in 15 files |
| Desktop E2E | WebdriverIO + `@wdio/tauri-service` + `tauri-driver` + matching `msedgedriver`, driving the compiled Tauri/WebView2 window; 10 journeys in 2 spec files |
| Backend tests | `cargo test`: 273 passed (230 library, 26 security, 5 cleanup-stress, 5 scanner-stress, 3 Phase 3 project, 2 analyzer-actions, 1 real-system, 1 performance), 4 ignored |

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
  features/overview/          Home summary: system, volumes, applications, cleanup, recent activity
  features/uninstaller/       Applications inventory, uninstaller, batch queue, process manager & residual drawer
  features/analyzer/          Storage & Cleanup → Disk space: scan controls, treemap, tree, filters, duplicates, storage
  features/cleaner/           Storage & Cleanup → Cleanup: caches, Projects, preview drawer, execution
  features/storage/           Storage & Cleanup destination shell and page tabs
  features/activity/          History & Recovery → Activity
  features/history/          History & Recovery destination shell, Recovery and cleanup log
  features/performance/       live metrics, 20-second spike capture, kernel/driver/event evidence, startup checks, baselines, optional trace
  features/settings/          theme, scan scope, exclusions, startup apps, Explorer integration, safety, updates
  lib/api.ts                  typed invoke wrapper + cached cache-inventory request
  lib/updates.ts              updater check, progress, close and relaunch wrapper
  lib/types.ts                shared IPC contract (camelCase)
  lib/analyzerFilters.ts      tree/file filtering, sorting and filtered totals
  lib/treemap.ts              squarified treemap layout
  lib/categories.ts           extension → category → color
  lib/risk.ts                 safe / caution / protected vocabulary
  lib/settings.tsx            persisted settings context
  lib/hooks.ts                useAsync, useDebouncedValue, useEscape
  styles/tokens.css           light/dark design tokens
  styles/app.css              layout, panels, tables, forms and responsive rules
  styles/app.css              shell, primitives and shared layout
src-tauri/
  src/apps.rs                 multi-source application discovery (Win32, Store/MSIX, Steam, Portable)
  src/fs.rs                   native directory enumeration + std::fs fallback
  src/mft.rs                  NTFS MFT reader and subtree construction
  src/scan.rs                 streaming walker, method selection, aggregation
  src/duplicates.rs           size buckets, partial + full hashing
  src/storage.rs              categories, age buckets, insights
  src/caches.rs               cache catalog, env expansion, measurement, conservative tool detection
  src/devscan.rs              read-only project recognition, artifact measurement, protected-data evidence
  src/cleanup.rs              cleanup guard, preview, quarantine, recovery journal, restore, purge, log
  src/process.rs              process discovery, critical-process protection, controlled termination
  src/uninstall.rs            safe command parsing, uninstaller execution, outcome reconciliation
  src/store.rs                native current-user Store/MSIX inventory and package removal
  src/app_history.rs          persistent installed/removed application lifecycle history
  src/leftovers.rs            leftover scanning, registry backup, residual quarantine
  src/explorer.rs             Windows Explorer right-click shell verbs, CLI routing, path validation
  src/perf.rs                 live CPU/memory/disk/process sampling, startup and task evidence, baselines, optional WPR trace
  src/providers.rs            declarative cleanup provider system, schema validator, safety enforcement
  src/volumes.rs              local volume capacity
  src/history.rs              app-data history persistence
  src/commands.rs             validated Tauri command layer
  src/lib.rs                  state, dialog plugin, command registration
  tests/real_system.rs        live registry/cache/scan integration test
  tests/security.rs           permission boundaries, protected paths, links, fixtures
  tests/cleanup_stress.rs     locked files, cancellation, wide cleanup fixtures
  tests/stress_scan.rs        wide/deep/denied/cancellation stress tests
  tests/performance.rs        live read-only performance snapshot consistency
  examples/scan_bench.rs      benchmark binary used by scripts/bench-scan.mjs
  examples/cleanup_fixture.rs isolated end-to-end cleanup verification
  examples/rc1_e2e_fixture.rs comprehensive uninstaller, process, and residual e2e fixture
scripts/                      MSVC env shim, build/bundle/binary/smoke/benchmark checks
```

## Backend modules

| Module | Responsibility |
| --- | --- |
| `apps.rs` | Enumerates `HKLM`/`HKCU` uninstall keys (64-bit and 32-bit views), Microsoft Store packages through the Windows Runtime `PackageManager` (with a read-only `AppModel\Repository\Packages` fallback), Steam libraries (`libraryfolders.vdf`), and bounded portable binaries (`%LOCALAPPDATA%\Programs`). Detects MSI product codes in commands or GUID-named uninstall subkeys, reads PE version metadata for portable executables, identifies broken uninstallers, and provides source tags. |
| `fs.rs` | Opens a directory with `FILE_LIST_DIRECTORY` + `FILE_FLAG_BACKUP_SEMANTICS` and reads `FILE_ID_BOTH_DIR_INFO` batches. Returns name, raw Win32 attributes, logical size, `AllocationSize`, file id and created/accessed/last-write times. Falls back to `std::fs` when the native path fails or exceeds an 8 MiB listing buffer. |
| `mft.rs` | Reads the NTFS boot sector and `$MFT` run list, validates record counts against a derived memory budget, applies update-sequence fixups, parses `FILE_NAME`/`$DATA`/`$STANDARD_INFORMATION`, deduplicates hard links with a record bitset, and builds the requested subtree. Cancellation and progress are polled during parsing. Any failure returns a fallback reason. |
| `scan.rs` | Chooses the scan method, then runs the streaming directory walk: depth/error/stat-output/tree caps, exclusions, hidden/system filtering, reparse skipping, totals, extension stats, largest files, hard-link tracking, throttled progress and cancellation. Builds `StorageReport` alongside the tree. `measure_path` reuses the walker without a tree or cancellation. |
| `duplicates.rs` | Walks a root for files at or above `minSize`, groups by size, confirms groups with 4 KiB head/tail SHA-256, then full streaming SHA-256. Tracks hard links by file id, polls cancellation, emits `cyron-duplicate-progress`, caps candidates/groups/files and reports truncation. |
| `storage.rs` | Classifies bytes into ten categories by known root and extension, marks known vs inferred confidence, buckets files by age, and derives up to six explained insights (large, old, developer-heavy). |
| `caches.rs` | Fixed catalog of 66 developer, AI, container and Windows cache locations. Expands `%VAR%` segments, resolves paths, measures each existing folder with `scan::measure_path`, deduplicates paths and computes totals per risk level. Each target also records `install_state` (`detected`, `not-detected`, `unknown`); only an explicit `not-detected` appears as an orphaned cache and nothing is auto-selected. |
| `devscan.rs` | Read-only developer-storage scanner. Discovers projects from package manifests, Git roots and framework metadata, measures only named regenerable artifacts (`node_modules`, `dist`, `build`, `target`, `.next/cache`, `.turbo`, Python caches and equivalents), records confidence and ownership evidence, and reports protected components (`.git`, `.env`, credentials, project databases, model weights) separately. Artifact ids are resolved from a server-held scan; no frontend path reaches cleanup. Also re-measures selected artifacts after a run for verification. |
| `providers.rs` | Declarative versioned (`schema_version: 1`) cleanup provider system. Enforces approved environment roots, validates path templates and patterns, rejects path traversals, wildcards in roots, and attempts to mark protected data safe. |
| `cleanup.rs` | The primary cache cleanup engine. Resolves catalog ids to specs, enforces `guard_path`, plans and measures a preview, executes a server-held plan by moving files into a quarantine batch, and implements list/restore/purge plus the capped cleanup log. |
| `explorer.rs` | Registers and unregisters native per-user Windows shell context menu verbs under `HKEY_CURRENT_USER\Software\Classes` without administrator elevation. Implements standard and advanced destructive verbs (Force Delete, Locking Processes, Kill Process), path validation and system root guards (`is_protected_delete_target`), Windows Restart Manager lock detection (`rstrtmgr.dll`), and server-held single-use execution plans (`StoredForceDeletePlan`, `StoredKillProcessPlan`). |
| `process.rs` | Discovers processes executing from an application root, snapshots process state with 64-bit creation times (`GetProcessTimes`) to defeat PID recycling, denies Windows critical processes, and executes graceful `WM_CLOSE` preceding `TerminateProcess`. |
| `uninstall.rs` | Resolves vendor commands server-side from live registry keys, creates expiring single-use plans (`UninstallPlan`, `BatchUninstallPlan`), tokenizes commands safely, normalizes MSI invocations, and reconciles exit codes: 1605/1614 become removal only when a complete inventory confirms absence, 1602 is cancellation, 3010/1641 require restart, and exit 0 with the product still registered remains `remaining`. A bounded post-exit inventory poll reduces races with vendor cleanup. |
| `store.rs` | Enumerates current-user main packages through `PackageManager::FindPackagesByUserSecurityId`, filters frameworks, resources, bundles and system packages, and removes one reviewed package through `RemovePackageAsync(...).join()`. It never uses `RemoveForAllUsers` or machine-wide deprovisioning, and maps access-denied, in-use and not-found failures to truthful messages. No shell or `winget` wrapper is launched. |
| `app_history.rs` | Persists a bounded (2,000-record) installed/removed lifecycle at `<app_data_dir>/app-history.json` with durable atomic replacement. A complete inventory can retire a missing record; a partial inventory never marks anything removed. Records are descriptive and never authorize cleanup. |
| `perf.rs` | Samples system CPU times, memory and commit charge, PDH physical-disk counters, per-process CPU/working-set/I/O counters, fixed-volume capacity, startup registrations and boot/logon scheduled tasks. Reads the Diagnostics-Performance boot log and the recent System driver/device events only when Windows grants access, lists display/storage/network driver versions and package dates from the read-only class registry, records local baselines at `<app_data>\performance-baselines.json`, and optionally starts/stops a local WPR trace. A kernel-dominant sample is reported as kernel-mode activity without naming a driver it cannot attribute. It never changes services, memory, the page file, Defender, updates, drivers or the registry. |
| `leftovers.rs` | Scans likely residual files and registry keys under AppData, ProgramData, and HKCU/HKLM. Enforces root guards, executes recursive multi-type `.reg` backups, verifies backup integrity, and cleans residual files into quarantine. Name-only file, shortcut and registry matches are `caution` and unselected; only exact install-location or exact registration identity is preselected as `safe`. |
| `health.rs` | Read-only scan for dead startup entries and dead uninstall registrations. It expands `%VAR%` and considers only absolute Run/RunOnce executables that no longer exist (never Startup-folder shortcuts or bare PATH names), and reports an uninstall registration only when its uninstaller and install folder are both missing. Removal reuses the reviewed engines: a single-use `DeadStartupPlan` exports and verifies the exact value, writes a journal and manifest into quarantine, and refuses protected, changed or reappeared entries; dead registrations use the existing fingerprint-bound broken-entry plan. |
| `volumes.rs` | Lists local fixed/removable volumes with label, file system, capacity and free space. Network and optical drives are skipped deliberately. |
| `history.rs` | Reads/writes `history.json` in the app-data directory, newest first, capped at 100, atomic temp-file replace with a direct-write fallback. Corrupt files read as empty. |
| `searchindex.rs` | Native, read-only file index and Everything-compatible query engine. Walks every fixed local volume with `fs::read_dir` on up to four workers, records each accessible file/folder name, parent link, size, created/accessed/modified times and raw attributes, never follows reparse points, and never opens file contents. Entries live in compact name pools plus parallel vectors; full paths are reconstructed on demand. Only directory paths enter the lookup map, and a sorted name order answers name queries without sorting every match. Persists the index to `<app_data>\search-index.bin` with an atomic temp-file publish, loads it in the background at startup, and keeps it current with one recursive `ReadDirectoryChangesW` watcher per volume (add/remove/rename/metadata). The query parser supports AND/OR/NOT, quotes, wildcards, regex, `ext:`, `size:`, `dm:`/`dc:`/`da:`, `type:`/kind macros, `path:`, `attrib:`, `dupe:`, `empty:` and match-case/whole-word/path/regex options; results are sorted and paginated server-side. CSV/TXT export streams the selected or complete result set to a user-chosen destination, capped at 1,000,000 rows. It never executes a result path or writes to an indexed file. |
| `commands.rs` | Path/job/plan validation, background execution, cancellation registries, single-use plan registries, Store removal plans, app lifecycle history, event emission, history and log recording, Explorer reveal, and validated IPC commands. |
| `lib.rs` | Registers the dialog, updater and process plugins, `AppState`, startup route resolution, the background load of the persisted search index, and all Tauri commands. `read_only` starts `true`. |

## IPC surface

All payloads use `#[serde(rename_all = "camelCase")]`; the TypeScript mirror is
`src/lib/types.ts`. Command arguments are camelCase on the JS side and snake_case in Rust.

| Command | Arguments | Returns | Notes |
| --- | --- | --- | --- |
| `list_installed_apps` | — | `AppInventory { apps, warnings }` | Blocking thread; cached in AppState. |
| `list_removed_apps` | — | `InstalledApp[]` | Session-scoped retained identities absent from a complete inventory. |
| `list_volumes` | — | `VolumeInfo[]` | Synchronous Win32 queries. |
| `list_caches` | `driveLetters[]?` | `CacheInventory` | Validated mounted local drive scope; blocking thread. |
| `scan_developer_storage` | `roots[]?`, `driveLetters[]?` | `DeveloperStorageScan` | Read-only project/artifact scan; explicit roots are validated and automatic candidates stay on the selected local drives. |
| `preview_developer_cleanup` | `scanId`, `artifactIds[]`, `exclusions[]?` | `CleanupPreview` | Resolves ids against the server-held scan; refuses unknown/protected artifacts; plan is single-use and expiring. |
| `verify_developer_cleanup` | `scanId`, `artifactIds[]` | `DeveloperCleanupVerification` | Re-measures selected artifacts and lists quarantine batches that still hold recoverable copies. |
| `prepare_analyzer_action` | `paths[]`, `operation` | `AnalyzerActionPreview` | Separate recycle/permanent review; exact snapshot binding. |
| `execute_analyzer_action` | `planId` | `AnalyzerActionReport` | Consumes the plan; no client replacement targets. |
| `cancel_analyzer_action` | `planId` | — | Stops between safe item operations. |
| `start_scan` | `jobId`, `path`, `options` | `ScanResult` | Emits `cyron-scan-progress`. |
| `cancel_scan` | `jobId` | — | Sets the registered flag. |
| `start_duplicate_scan` | `jobId`, `path`, `options` | `DuplicateReport` | Emits `cyron-duplicate-progress`. |
| `cancel_duplicate_scan` | `jobId` | — | Sets the registered duplicate flag. |
| `measure_path` | `path` | `Measured` | Directory only; no tree or progress. |
| `reveal_path` | `path` | — | Explorer reveal of an existing path. |
| `get_history` | — | `HistoryEntry[]` | Newest first. |
| `clear_history` | — | — | Removes only Cyron's history file. |
| `get_cleanup_mode` | — | `bool` | `true` means read-only. |
| `set_cleanup_mode` | `readOnly` | `bool` | Backend enforcement switch. |
| `preview_cleanup` | `targetIds[]`, `exclusions[]?` | `CleanupPreview` | Ids only; protected targets refused; exclusions validated. |
| `preview_remnant_cleanup` | `appId`, `planId`, `candidateIds[]`, `exclusions[]`, `driveLetters[]` | `CleanupPreview` | Consumes the residual scan review; app absence rechecked before execution. |
| `start_cleanup` | `planId`, `targetIds[]` | `CleanupReport` | Single-use plan; read-only gated; journaled quarantine. |
| `list_quarantine` | — | `QuarantineBatch[]` | Batches with restorable entries, newest first. |
| `restore_quarantine` | `runId` | `RestoreReport` | No overwrite; retires the batch when everything is back. |
| `purge_quarantine` | `runId` | `PurgeReport` | Manifest-scoped; only paths still inside the batch folder. |
| `get_cleanup_log` | — | `CleanupLogEntry[]` | Newest first, max 100. |
| `prepare_uninstall` | `appId`, `quiet` | `UninstallPlan` | Server-resolved command; 10-minute single-use plan. |
| `prepare_batch_uninstall` | `appIds[]`, `quiet` | `BatchUninstallPlan` | Validates the ordered queue. |
| `discover_app_processes` | `appId` | `AppProcess[]` | Snapshots processes inside the app install roots. |
| `terminate_app_processes` | `appId`, `pids[]`, `force` | `ProcessTerminateResult[]` | Revalidates PID, creation time and path; critical processes refused. |
| `parse_uninstall_command` | `rawCommand` | `ParsedUninstallCommand` | Read-only tokenizer used for review display. |
| `execute_uninstall` | `appId`, `planId`, `timeoutSecs?` | `UninstallExecutionResult` | Consumes the plan; polls and reconciles the live inventory. |
| `list_app_history` | — | `AppHistoryRecord[]` | Persistent installed/removed records, newest activity first, capped at 2,000. |
| `prepare_store_uninstall` | `appId` | `StoreUninstallPlan` | Re-reads the current-user package and creates a 10-minute single-use plan bound to full name, family name and publisher ID. |
| `execute_store_uninstall` | `planId` | `StoreUninstallResult` | Consumes the plan, revalidates package identity, removes the package for the current user, and verifies absence in inventory. |
| `scan_app_leftovers` | `appId` | `LeftoverScanReport` | Filesystem and registry candidates with risk tiers; falls back to persistent history for a removed app. |
| `scan_named_remnants` | `appName`, `publisher?`, `installLocation?` | `LeftoverScanReport` | Explicit review-only scan for apps removed before Cyron; every match stays caution and unselected. |
| `clean_app_leftovers` | `appId`, `planId`, `candidateIds[]` | `LeftoverCleanReport` | Journaled quarantine and multi-type `.reg` backups. |
| `execute_batch_uninstall` | `planId` | `BatchUninstallReport` | Sequential with failure isolation; reconciles each app. |
| `cancel_batch_uninstall` | `planId` | — | Stops after the running vendor uninstaller. |
| `get_startup_route` | — | `StartupRoute \| null` | Consumes the CLI route once. |
| `get_explorer_integration_status` | — | `ExplorerIntegrationStatus` | Reads HKCU registration state. |
| `set_explorer_integration` | `standard`, `advanced` | — | Registers or unregisters per-user shell verbs. |
| `repair_explorer_integration` | — | — | Re-registers with the current binary path. |
| `list_cleanup_providers` | — | `CleanupProviderDef[]` | Declarative provider definitions. |
| `inspect_cleanable_directory` | `path` | `CleanableInspection` | Analyzer-to-Cleaner recognition and expected reclaimable bytes. |
| `query_directory_children` | `query` | `DirectoryPage` | Registered; no current view consumes the scan index. |
| `search_indexed_files` | `query` | `SearchResultPage` | Registered; no current view consumes the scan index. |
| `get_scan_index_stats` | `jobId` | `IndexStats` | Registered; no current view consumes the scan index. |
| `search_index_status` | — | `SearchIndexStatus` | Read-only index state: empty/loading/building/ready/error, entry/file/folder counts, volumes, progress, warnings and live-watch state. Never waits for a full query. |
| `build_search_index` | — | `SearchIndexStatus` | Starts a background index build over fixed local drives. Rejects a second concurrent build; the previous index remains queryable until the new one is published. |
| `cancel_search_index_build` | — | `SearchIndexStatus` | Sets the build cancellation flag and returns the current status. A partial index may be kept with a warning. |
| `query_search_index` | `options` | `SearchIndexPage` | Read-only local query with bounded length/term count, filters, sort and pagination; result paths are display data only. |
| `export_search_results` | `options`, `format`, `destination`, `selected?` | `SearchExportReport` | Writes generated CSV/TXT rows for the selection or the full sorted result set to an absolute `.csv`/`.txt` path inside an existing folder. Caps at 1,000,000 rows; never opens or executes a result. |
| `snapshot_app_identity` | `appId` | `InstalledApp` | Backend identity for review, including Hunter tokens. |
| `prepare_broken_inventory_cleanup` | `appId` | `BrokenEntryPlan` | 5-minute single-use plan; fingerprint-bound. |
| `execute_broken_inventory_cleanup` | `planId`, `confirmedKeyPath` | `LeftoverCleanReport` | Re-verifies the live key before deletion. |
| `prepare_forced_removal_cleanup` | `appId` | `BrokenEntryPlan` | Unregistered application review. |
| `execute_forced_removal_cleanup` | `planId`, `confirmedKeyPath` | `LeftoverCleanReport` | Two-step plan; journaled and backed up. |
| `restore_registry_backup` | `runId`, `overwriteConflict?` | `RegistryRestoreReport` | SHA-256 and scope verified; conflicts require explicit overwrite. |
| `resolve_cursor_target` | — | `HunterTargetInfo \| null` | Uses the current cursor; isolated Hunter snapshot. |
| `get_system_specs` | — | `SystemSpecs` | Five-second cache. |
| `list_startup_items` | — | `StartupItem[]` | HKCU/HKLM Run/RunOnce, WOW6432Node and Startup folders. |
| `set_startup_item_enabled` | `id`, `enabled` | `StartupItem` | Windows `StartupApproved` 0x02/0x03; rejects protected entries. |
| `scan_dead_entries` | — | `DeadEntryScanReport` | Read-only scan of accessible Run/RunOnce hives and Win32 uninstall registrations; each entry carries a reason, protection and elevation flags, and unreadable hives become warnings. |
| `prepare_dead_startup_removal` | `startupId` | `DeadStartupPlan` | 5-minute single-use plan bound to the exact registry value and metadata fingerprint; refuses protected or reappeared entries. |
| `execute_dead_startup_removal` | `planId`, `confirmedLocation` | `DeadStartupRemovalReport` | Read-only gated; revalidates the entry, writes and verifies a single-value `.reg` backup plus journal/manifest in quarantine, then deletes only that value. Restorable through `restore_registry_backup`. |
| `capture_performance_snapshot` | `forceStartup?` | `PerformanceSnapshot` | Read-only 700 ms sample plus cached driver inventory and recent System events; `forceStartup` bypasses the 120 s startup/device-evidence caches for a manual refresh. |
| `list_performance_baselines` | — | `PerformanceBaseline[]` | Local baselines, newest first, capped at 50. |
| `record_performance_baseline` | `label` | `PerformanceBaseline` | Captures a fresh snapshot and stores compareable figures locally. |
| `start_performance_trace` | — | `PerformanceTraceStatus` | Optional WPR trace; refuses when already recording, unavailable or unelevated. |
| `stop_performance_trace` | — | `PerformanceTraceStatus` | Stops and saves the local trace; no upload. |
| `open_performance_settings` | `page` | — | Opens a fixed Windows settings page; no arbitrary command. |
| `prepare_force_delete_plan` | `targetPath` | `ForceDeletePlanPreview` | Inspects target, locks and protections; 10-minute plan. |
| `execute_force_delete_plan` | `planId`, `mode` | `ForceDeleteResult` | `quarantine` or `permanent`; unknown modes refused; journaled quarantine. |
| `terminate_force_delete_locking_process` | `planId`, `pid`, `force` | `ProcessTerminateResult` | PID and creation time must match the plan. |
| `prepare_kill_process_plan` | `exePath` | `KillProcessPlanPreview` | Exact image path; unverifiable processes omitted. |
| `execute_kill_process_plan` | `planId`, `pids[]`, `force` | `ProcessTerminateResult[]` | Consumes the plan; critical processes refused. |
| `find_locking_processes` | `targetPath` | `LockingProcessInfo[]` | Windows Restart Manager (`rstrtmgr.dll`). |

The Search destination is backed by Cyron's own index, not the analyzer's in-memory scan index
and not a third-party process. A build enumerates every fixed local volume with `fs.rs`, skips
reparse points and records metadata only. One `FileIndex` holds a UTF-8 name pool, a lowercase
matching pool, fixed-size entry records (parent link, size, timestamps, attributes, flags) and
child/sibling links; the full path is reconstructed by walking parents, so the index does not
duplicate path strings for every file.

The index is published with a magic/version header to `<app_data>\search-index.bin` through a
temp-file write, `sync_all`, and an atomic replace. Startup loads it in a background thread, then
starts one recursive `ReadDirectoryChangesW` watcher per indexed volume; watcher events update the
in-memory index in place and re-seed descendant path keys after a folder rename. A rebuild builds a
new index off to the side, saves it, then swaps it in, so queries keep using the previous index
until the new one is complete. Directory enumeration runs on up to four workers while insertion
holds one short index lock per folder; a directory-only path map and a sorted name order keep
memory and name-sorted queries bounded. Watcher mutations clear that order and fall back to the
general comparator until the next rebuild. The app data cache is the only file the index module
writes during scanning; indexed files are opened for metadata only and are never modified, executed
or deleted. The explicit Search export writes its generated CSV/TXT only to the destination chosen
in the save dialog.

Queries are parsed server-side. Whitespace is AND, `|` is OR, `!` is NOT, parentheses group,
quotes form a phrase, and `*`/`?` are wildcards. Field terms include `ext:`, `file:`/`folder:`,
`size:`, `dm:`/`dc:`/`da:` (relative keywords such as `today`, `thisweek`, `lastmonth`, absolute
dates, comparisons and `..` ranges), `attrib:`, `type:` and the audio/video/image/document/
executable/compressed macros, `path:`, `dupe:` and `empty:`. The global match-case, whole-word,
match-path and regex switches are applied by the parser, and the UI's filter chips add a typed
predicate. Queries are capped at 512 characters and 32 terms; regex uses Rust's linear-time engine
with bounded compiled size. Results carry only display metadata; the renderer reveals a path
through the existing validated `reveal_path` command.

All three registries reject a job id that is already running instead of replacing its cancellation
flag, so a cancel call cannot target a flag that no longer controls the running task. Cleanup plan
ids are opaque, single-use and swept when expired.

## In-app updates

**Settings → About → Software updates** is the only application network path. The Tauri updater
plugin fetches the fixed `https://github.com/Ry0x7/cyron-uninstaller/releases/latest/download/latest.json`
manifest only after the user clicks **Check for updates**. The manifest names the version, notes, a
signed NSIS setup URL and the minisign signature checked against the public key in
`tauri.conf.json`. Its only platform entry is `windows-x86_64-nsis`; an MSI installation is not
offered the per-user NSIS package. Download progress is reported to the renderer; installation
starts only after a native confirmation, and the Windows installer restarts Cyron when it finishes.
There is no background polling, telemetry, caller-supplied URL or update private key in the
application.
Release builds use `src-tauri/tauri.release.conf.json` to create the updater signatures, and
`scripts/generate-updater-manifest.mjs` writes `latest.json`. The updater private key is a local or
CI secret; the public key is committed in `tauri.conf.json`.

Progress event payloads:

```text
cyron-scan-progress       { jobId, files, dirs, bytes, currentDir }
cyron-duplicate-progress   { jobId, phase(index|partial|full), files, bytesHashed, currentPath }
cyron-analyzer-action-progress { planId, completed, total, currentPath }
cyron-batch-uninstall-progress { planId, completed, total, currentApp }
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
                   status(completed|partial|cancelled|failed)

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
                   failed, status(completed|partial|cancelled|failed), detail
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
   The Phase 3 `preview_developer_cleanup` path is equally id-only: a 1-hour server-held
   `DeveloperStorageScan` supplies the artifact id, project ownership path, kind and evidence;
   `devscan::selected_specs` refuses unknown ids, protected artifacts and any path that escaped its
   recorded project. The resulting artifacts still pass the same `guard_path`, preview, plan and
   execution checks as catalog targets.
2. `plan_specs_with_exclusions` refuses an empty selection, runs `guard_path` on every spec, and
   validates the caller's exclusions (absolute, non-root, at most 128, de-duplicated).
3. `guard_path` refuses a relative path, anything with fewer than three path components (drive roots
   and top-level folders), any of the environment roots themselves (`USERPROFILE`, `LOCALAPPDATA`,
   `APPDATA`, `WINDIR`, `SystemDrive`, `ProgramFiles`, `ProgramFiles(x86)`) and any path that
   **contains** a protected catalog location. It is a floor, not the whole model: the real allow
   list is the reviewed catalog plus server-held project artifacts, and the guard runs again for
   each target at execution time.

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

Windows Store/MSIX applications are enumerated for the current user through the Windows Runtime `PackageManager`; the `AppModel\Repository\Packages` registry view remains a read-only fallback when the WinRT API is unavailable. Framework, resource, bundle and system packages are excluded from the removable inventory. Application icons are not extracted.

## Cache catalog

`caches.rs` contains 66 `TargetDef` entries. Each has a stable id, tool, label, env-templated path,
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

## Developer storage discovery

`devscan.rs` is read-only. It walks explicit project folders plus a bounded automatic candidate set
(`%USERPROFILE%\source`, `repos`, `projects`, `dev`, `code`, `workspace`, `git`, `src`, selected
Documents/Desktop/Downloads project folders, and common project folders at the root of each selected
local drive) to depth 3, bounded by 60,000 directories and 400 projects. It never follows reparse
points and never enters `node_modules`, `.git`, `target`, `dist`, `build`, `.next`, `.turbo`,
`.gradle`, `__pycache__`, virtual environments or vendor trees while looking for projects.

Recognition uses direct manifests (`package.json`, `Cargo.toml`, `pyproject.toml`,
`requirements.txt`, `poetry.lock`, `uv.lock`, `go.mod`, Gradle/Maven files, `.csproj`/`.sln`,
`CMakeLists.txt`, `composer.json`, `Gemfile`) and Git roots. Package dependencies supply framework
evidence (`next`, `vite`, `react`, `vue`, `svelte`, `astro`, `nuxt`, `remix`, `tauri`, and similar).
Artifacts are limited to named regenerable folders: `node_modules`, `dist`, `build`, Rust/Maven
`target`, Gradle `build`/`.gradle`, `.next/cache` (not all of `.next`), `.turbo`, `__pycache__`,
`.pytest_cache`, `.mypy_cache`, and `.ruff_cache`. Each record carries size, allocated size, file and
folder counts, latest modification time, evidence and confidence. Hard-linked files are counted once
by file id; links and junctions are skipped.

Project and artifact activity is informational: active (writes within 30 days), recent (180 days),
old, or unknown. Nothing is preselected because a project is old or an artifact is large. `.git`,
`.env`-style files, credentials, keys and model weights are measured into a separate protected list.
The frontend never supplies a path; `preview_developer_cleanup` resolves artifact ids from the
server-held scan, and `verify_developer_cleanup` re-measures them after a run.

## Frontend architecture

`App` owns the active view and a `visited` list. Visited views stay mounted but `hidden`, so an
in-progress scan or a populated table survives navigation; there is no router dependency. The
sidebar has five primary destinations — Home, Applications, Storage & Cleanup, Performance, and
History & Recovery — with Settings in the footer and a system/light/dark theme switch inside
Settings. It is collapsible (persisted under `cyron.sidebar.collapsed`) and exposes `aria-current`.
Hunter Mode is launched from **Find by window** inside Applications, not from navigation.

The visual system is centralized in `src/styles/tokens.css`, shared primitives in
`src/components/ui.tsx`, and the navigation and component rules in [DESIGN.md](DESIGN.md). Feature
styles consume tokens instead of defining their own palette, radius or shadow scales.

`SettingsProvider` reads `cyron.settings.v1`, writes changes back to localStorage, resolves
`system` through `prefers-color-scheme`, and sets `data-theme` on `<html>`. Settings are:
`theme`, `includeHidden`, `exclusions[]`, `projectRoots[]`, `readOnly`. Exclusions feed both
`ScanOptions` and the cleanup preview; `projectRoots` extends the developer project scan without
automatically selecting anything; `readOnly` is pushed to the backend with `set_cleanup_mode` so the
UI and the backend switch always agree (the backend also defaults to read-only on its own).

`lib/api.ts` is the single IPC boundary. It rejects with a clear message when the app runs outside
Tauri, normalizes errors, and memoizes `list_caches` so Home and Storage & Cleanup share one
measurement pass; `invalidateCaches()` forces the next request to run again after a cleanup changes
files.

### Views

- **Home** joins volumes, application inventory, cache inventory and history. All figures come from
  those responses; loading, error and empty states are explicit.
- **Applications** filters/sorts the registry inventory, shows missing sizes as "—", opens a
  drawer with metadata, real folder measurement and Explorer reveal, and launches Hunter Mode from
  **Find by window**.
- **Storage & Cleanup → Disk space** starts from volumes or the native folder picker, subscribes to
  progress events filtered by a generated job id, supports cancel and cancels on unmount. Results
  are nodes held in a stack for breadcrumbs. Tabs: **Map**, **Files**, **Duplicates**, **By type**.
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
- **Storage & Cleanup → Cleanup → Caches & leftovers** groups cache targets into Windows /
  Applications / Developer tools / AI tools, filters by risk, category and search, expands rows for
  path, impact and dates, and separates protected data into a read-only section. Selecting targets
  opens a preview drawer that lists the planned files (capped at 400 with an explicit note),
  skipped items, warnings and the quarantine promise; a confirmation checkbox enables the run.
  Progress comes from `cyron-cleanup-progress` and results are per item.
  The **Projects** tab (`DeveloperStorage.tsx`) scans recognised repositories on the selected
  drives plus user-added folders, shows project activity, exact artifact evidence, rebuild/loss
  copy and protected components, and sends only selected server-held artifact ids into the same
  preview/cleanup engine. A successful run re-measures the selected artifacts and surfaces
  remaining files and restorable quarantine batches.
- **History & Recovery** combines **Activity** (persisted scans and cleanup runs with a two-step
  clear confirmation that Escape cancels) and **Recovery** (quarantine restore/purge plus the
  cleanup log). Opening Recovery refreshes quarantine so a just-finished run cannot look empty.
- **Performance** (`features/performance/Performance.tsx`) renders measured system, disk, process,
  startup and scheduled-task evidence, symptom guides, local baselines and the optional WPR trace.
  All sampling is read-only; the only mutation is the existing reversible startup toggle.
- **Settings** validates exclusions (drive-letter or UNC paths, no wildcards), debounces saving by
  450 ms, and resets to defaults. Appearance, scanning, startup, Explorer and safety each have a
  dedicated tab.

The treemap layout is pure TypeScript (`squarify`) with no chart dependency; extension families,
colors and labels live in `lib/categories.ts`.

## Store/MSIX inventory and removal

Windows Store, UWP, and modern packaged applications (MSIX) do not write standard Win32 uninstall keys under `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`. Instead, their metadata is stored in:
- `HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages`
- `HKLM\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages`

### Technical findings
1. **Discovery:** Cyron reads current-user main packages through `Windows.Management.Deployment.PackageManager::FindPackagesByUserSecurityId`, including full name, family name, display name, publisher, publisher ID, install path, version, framework/resource/bundle flags and signature kind. The registry repository is a read-only fallback.
2. **Removal boundary:** MSIX packages have no `UninstallString` or MSI product code. Cyron removes a reviewed package through `PackageManager::RemovePackageAsync` and waits on the operation. It never calls `RemoveForAllUsers`, never deprovisions machine-wide, and never spawns `powershell.exe`, `winget` or another shell.
3. **Permissions:** Removal is current-user only. Access denied, package in use and package not found are reported distinctly; a permission failure is never retried with elevation. Machine-wide or provisioned packages must be removed through Windows Settings or an administrator-managed tool.
4. **Plan binding:** `prepare_store_uninstall` re-reads the live package, rejects dependencies and system packages, and stores a 10-minute single-use plan bound to the full package name, family name and publisher ID. `execute_store_uninstall` revalidates all three before removal and then verifies the package is absent from the live inventory before reporting `completed`.
5. **Verification status:** The native inventory path is implemented and unit-tested for package filtering. The removal plan/execute path is implemented but its live behavior is unverified. A real Store package removal through the desktop UI is **UNVERIFIED**; it must be exercised on a disposable Windows profile before release.

## Persistence

| Data | Location | Lifetime |
| --- | --- | --- |
| Theme, scan scope, exclusions | `localStorage["cyron.settings.v1"]` | Until cleared by the user/profile reset |
| Sidebar collapsed | `localStorage["cyron.sidebar.collapsed"]` | Same |
| Tree column width | `localStorage["cyron.analyzer.treeWidth"]` | Same |
| Scan history | `<app_data_dir>/history.json` | Until cleared from Activity or the profile is removed |
| Quarantine batches | `<app_data_dir>/quarantine/<run-id>/` | Until restored and retired or purged by the user |
| Recovery journal | `<quarantine>/<run-id>/recovery_journal.jsonl` | Append-only per-move write-ahead log |
| Application lifecycle history | `<app_data_dir>/app-history.json` | Newest 2,000 installed/removed records |
| Cleanup log | `<app_data_dir>/cleanup-log.json` | Newest 100 entries |
| Scan and duplicate results | Rust memory / React state | Session only |
| Pending cleanup plans | Rust memory (`AppState::plans`) | 10 minutes or until executed |
| Pending uninstall plans | Rust memory (`AppState::uninstall_plans`) | 10 minutes or until executed |
| Process snapshots | Rust memory (`AppState::process_snapshots`) | 5 minutes or until refreshed |
| Hunter process snapshots | Rust memory (`AppState::hunter_snapshots`) | 5 minutes; completely isolated from uninstaller process snapshots |
| Hunter app snapshots | Rust memory (`AppState::hunter_app_snapshots`) | 5 minutes; isolated from uninstaller app snapshots |
| Pending leftover plans | Rust memory (`AppState::leftover_plans`) | 10 minutes or until executed |
| Pending broken entry plans | Rust memory (`AppState::broken_entry_plans`) | 5 minutes or until executed; bound to SHA-256 live metadata fingerprint |
| Cached installed apps | Rust memory (`AppState::cached_apps`) | Session / until invalidated |

Disk scans, cleanup runs, restores and purges write history; cache discovery and duplicate runs are
measurements and are not logged.

## Security and safety constraints

- **Cleaner and leftover modifications** use `cleanup.rs` and `leftovers.rs` with review-first plans, quarantine batches, recovery journals and no-overwrite restoration. Separate Analyzer actions live in `analyzer_actions.rs` under the explicit exception documented below.
- **Process management** in `process.rs` verifies executable ownership, creation time (`GetProcessTimes`), and PID identity against a recent snapshot. Critical Windows system processes and Windows directory binaries can never be terminated.
- **Uninstaller execution** in `uninstall.rs` validates vendor command lines, normalizes MSI product codes, and rejects shell wrappers (`cmd.exe`, `powershell.exe`, `.bat`, etc.). Executables and arguments are resolved server-side from registered keys; caller-supplied executables are never launched.
- **Registry modifications** in `leftovers.rs` are guarded against shared vendor keys and system roots, and require an automatic multi-type recursive `.reg` backup before deleting any subkey.
- Command inputs are validated before use: job ids are 1–64 characters of `[A-Za-z0-9_-]`; paths must be non-empty, absolute, existing, canonicalized and directories.
- Reparse points are skipped during traversal, preventing cycle and mount-point escapes.
- Scan depth, returned child counts, retained nodes, file nodes, largest-file output, extension stats and keys, errors, tracked file ids, the native directory buffer and the MFT record budget are all capped.
- The WebView runs with a strict CSP (`default-src 'self'`, IPC allowed, no remote scripts) and only the dialog-open and dialog-ask capabilities are granted beyond Tauri core defaults. Native confirmations cannot be spoofed by renderer content.
- No telemetry, analytics or background update polling is present. The only user-initiated network path is the signed updater checked from Settings → About. The renderer cannot supply an update URL, and the update private key is never bundled.
- The app does not request elevation. Without it, MFT is unavailable, the walk reports unreadable folders, and locked cache files are skipped instead of forced.
- Cleanup starts in read-only mode.

## Performance characteristics and limits

Scans run off the UI thread via `spawn_blocking`; the UI receives throttled counters and the final
pruned result. The most recent three-run, unelevated measurements are in
[docs/BENCHMARKS.md](BENCHMARKS.md). This table reproduces them:

| Target | Method | Median | Peak working set |
| --- | --- | ---: | ---: |
| `C:\Windows\System32` | directory walk | 0.21 s | 8.4 MiB |
| `node_modules` (repo) | directory walk | 0.36 s | 11.1 MiB |
| `src` (repo) | directory walk | 0.00 s | 5.5 MiB |

MFT requires an elevated process and was not measured in that run, so no MFT timing is claimed
here. MFT trades memory for speed and only pays off at a drive root. The walk is streaming and
bounded but slower; neither is a WizTree comparison.

## Testing and CI

- `src/lib/*.test.ts` — Frontend unit test suites covering formatting, categories, risk ratings, analyzer filters, cleanup selection rules, treemap geometry, and Tauri IPC contracts.
- Rust unit tests — Tests covering registry discovery, directory traversal, MFT parsing, duplicate detection, storage categorization, cache catalog validity, quarantine roundtrip, uninstaller tokenization, process filters, and recursive registry backup/restore.
- `tests/real_system.rs` — live registry count, live cache targets, a temporary fixture tree with exact byte assertions.
- `tests/security.rs` — Architectural boundary and security tests: module boundaries for destructive APIs, protected-path refusal, hostile IDs, junction/symlink swaps, shell rejection, process termination boundaries, and registry backup verification.
- `tests/cleanup_stress.rs` — wide cleanup fixture with a bounded preview, locked file, cancelled run, consecutive batches and empty/missing targets.
- `examples/cleanup_fixture.rs` + `scripts/verify-cleanup.mjs` — end-to-end plan → quarantine → restore → purge against `%TEMP%\cyron-cleanup-<pid>`, printing `CLEANUP_FIXTURE_OK`.
- `tests/stress_scan.rs` — 20,000-file wide directory with exact totals and bounded children, deep nesting, access-denied folders, exclusion subtree and cancellation.
- `tests/performance.rs` and `perf::tests` — live read-only snapshot consistency, CPU accounting, command parsing, exact executable-to-boot-event matching, scheduled-task provenance, driver-date/event classification, the kernel-share threshold, evidence on every recommendation, an idle-disk/high-process-I/O non-recommendation, and baseline persistence.
- `tests/desktop/*.spec.mjs` + `scripts/desktop-e2e.mjs` — real Windows desktop E2E. Builds the disposable fixture, starts `tauri-driver`/`msedgedriver`, drives the compiled Tauri WebView2 window, exercises real IPC and destructive plans, and records named screenshots, JSON results and the tested revision. `npm run test:desktop` runs the suite; `node scripts/desktop-uia.ps1` exposes Windows UI Automation for native dialogs.
- `.github/workflows/ci.yml` & `release.yml` — Windows CI for typecheck, Vitest, frontend build, `cargo test`, security and cleanup-stress suites, release packaging, checksum generation, and GitHub release automation.
- `scripts/verify-dist.mjs`, `verify-desktop-binary.mjs`, `smoke-desktop.mjs`, `verify-release-bundles.mjs`, `bench-scan.mjs` — artefact verification, desktop launch smoke checks, and release bundle verification.

## Deferred design work

- MFT scanning for volumes above the record budget (e.g. streaming aggregation without retaining all records).
- Native scheduled-task and Windows service remnant discovery. Current scans limit destructive candidates to files, shortcuts and registry keys; service and task ownership is not inferred from names.
- Cold-cache benchmark methodology and multi-run statistics published from CI hardware.

## Analyzer actions and scoped Cleaner review (2026-11-09)

Scanning remains strictly read-only. User-initiated Analyzer mutations are a narrow exception to the quarantine rule: `prepare_analyzer_action(paths, operation)` creates an opaque server-held ten-minute plan; `execute_analyzer_action(planId)` consumes it once and accepts no replacement paths or operation. `cancel_analyzer_action(planId)` stops between items. The UI collects paths from listing/treemap selection, reviews operation, counts and bytes, then shows per-item outcomes. Every execution attempt invalidates cached scans and selections and starts a fresh scan.

Permanent deletion uses no-follow handles, Windows file identities and empty-only directory disposition. Recycling uses Windows IFileOperation, recycle-only/early-failure flags, no recursion and no connected-file expansion. It handles approved files individually and removes approved directories only when empty. Empty folder entries are not restored by Cyron; files are restored through Windows Recycle Bin. The Windows shell exposes a final pathname operation: parent handles and immediate identity checks reduce substitution risk but do not make it atomic against another process in the same user session. Cleaner/leftover removal retain quarantine and journal recovery.

`list_caches(driveLetters?)` validates mounted local drive letters and measures supported locations only on those drives. Unknown directories named Temp, Cache or node_modules are not made disposable. Recycle Bin contents are inventory-only. Non-system-drive npm and Steam candidates require recognized layout evidence, and preview resolves the evidence again. Removed applications come from persistent lifecycle history plus retained snapshots; an incomplete inventory prevents any missing app from being classified as removed. Remnant candidates whose only evidence is a name match are `caution` and unselected; exact install-location or exact registration identity is the only `safe` evidence. Explicitly named scans for applications removed before Cyron was installed are review-only and never prove ownership.

Uninstaller completion requires a bounded post-exit inventory poll. Remaining registration or incomplete inventory produces `remaining`; 1605/1614 reconcile to completed only when a complete inventory confirms absence; restart, vendor cancellation and failure remain explicit. Batch stop requests apply between vendor uninstallers and never kill the running vendor process. Each batch item is rechecked against live inventory; missing and duplicate selections are rejected. Store packages use the separate native current-user plan path; unsupported machine-wide operations fall back to Windows Settings guidance.
