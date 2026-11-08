# Cyron — Implementation Status

Last updated: 2026-11-06 (Phase 7 — Code Signing Architecture & Release Packaging)

## Summary

Cyron Uninstaller is a verified Tauri v2 desktop application on Windows 10/11 x64 featuring:
- Multi-source application inventory (Win32 64/32-bit, Microsoft Store / AppX / MSIX, Steam game libraries, and portable heuristic scanning).
- Application uninstallation with exit code 3010 (reboot required) tracking.
- Process discovery, snapshot-verified termination with critical-process protection and PID recycling prevention.
- Leftover scanning with confidence & risk ratings, automatic recursive multi-type `.reg` backup, and quarantine storage.
- Sequential batch uninstallation with failure isolation.
- Interactive disk space analyzer with Squarified treemap and persistent virtualized folder tree (220–520 px).
- Dual scan engine: elevated NTFS MFT volume parsing with automatic streaming directory-walk fallback.
- 3-stage duplicate file verification with hard-link deduplication.
- Storage categorization into 10 categories with known vs inferred confidence ratings.
- Review-first cache cleaner over extensible declarative providers (`schema_version: 1`).
- Crash-consistent recoverable quarantine with durable write-ahead recovery journal (`recovery_journal.jsonl` with `sync_all`), atomic Win32 `MoveFileExW` no-overwrite restoration, and manifest-scoped purging.
- Windows Explorer context menu integration (`HKCU\Software\Classes`) with safe argument routing.
- Standardized Windows PE VERSIONINFO, WiX MSI properties, and NSIS installer headers (`WINDOWS_METADATA_OK`).
- Chryypt Organization code signing architecture (`CHRYYPT_SIGNED`, `SIGNATURES_VERIFIED_OK`).
- Verified release packaging: NSIS setup installer, WiX MSI package, and standalone executable (`RELEASE_BUNDLES_OK`).

Historical narratives and early acceptance logs for Phases 1–3 are preserved in [docs/archive/PHASE_HISTORY.md](archive/PHASE_HISTORY.md).

---

## Current Acceptance Ledger

### 1. Application Inventory & Uninstallation
| Feature | Status | Verification & Evidence |
|---|---|---|
| Win32 Discovery | Done | `apps.rs` enumerates `HKLM`/`HKCU` in 64-bit and 32-bit registry views; deduplicates, normalizes sizes/dates, flags broken entries. |
| Store / MSIX Discovery | Done | Discovers installed modern packages via `AppModel\Repository\Packages`; provides read-only lifecycle indicators and leftover scanning. |
| Steam Game Libraries | Done | Discovers multi-library Steam games via `libraryfolders.vdf` and `appmanifest_*.acf`. |
| Portable Executables | Done | Bounded heuristic detection under `%LOCALAPPDATA%\Programs`. |
| Standard Uninstaller Runner | Done | `uninstall.rs`: validates commands, strips command-chaining injection, rejects shell interpreters (`cmd.exe`, `powershell.exe`), handles reboot exit code `3010`. |
| Server-Held Confirmation Plans | Done | All destructive operations require server-generated, single-use, 10-minute expiring plans (`UninstallPlan`, `BatchUninstallPlan`, `CleanupPlan`, `LeftoverPlan`). |
| Process Management | Done | `process.rs`: matches processes strictly by executable path inside application root; verifies PID creation times (`GetProcessTimes`) against snapshot; protects critical Windows system processes. |
| Leftover Residual Cleaning | Done | `leftovers.rs`: discovers residual files and registry subkeys; creates automatic recursive multi-type `.reg` backup before deletion; moves files to quarantine. |
| Sequential Batch Uninstallation | Done | Sequential queue with failure isolation; residual cleanup requires separate confirmation. |

### 2. Disk Analysis & Intelligence
| Feature | Status | Verification & Evidence |
|---|---|---|
| Direct NTFS MFT Reader | Done | `mft.rs`: reads boot sector and `$MFT` run list; validates record count against 768 MiB budget before allocating; parses records and builds subtree. Measured live on `C:\` in 14.52 s. |
| Streaming Directory Walk | Done | `scan.rs`: depth-first streaming traversal; bounds hard links at 1M IDs; caps tree nodes at 100K and folder file nodes at 40; cancellation check latency < 120 ms. |
| Automatic Method Selection | Done | Drive roots select MFT when elevated; subfolders and non-elevated shells select streaming directory walk. |
| Treemap & Persistent Tree | Done | Canvas-based squarified treemap; virtualized resizable folder tree column with ARIA keyboard navigation. |
| Duplicate File Detection | Done | `duplicates.rs`: size grouping -> 4 KiB edge hashing -> full streaming SHA-256; hard links counted once for physical storage. |
| Storage Categorization | Done | `storage.rs`: 10 categories, 5 age buckets, known vs inferred confidence ratings, explainable insights. |
| Analyzer -> Cleaner Direct Action | Done | `inspect_cleanable_directory`: selecting recognized cache directory displays reclaimable space and provides 1-click review in Cleaner. |

### 3. Review-First Cache Cleaning & Quarantine
| Feature | Status | Verification & Evidence |
|---|---|---|
| Extensible Declarative Providers | Done | `providers.rs`: versioned (`schema_version: 1`) declarative definitions; validates environment roots, rejects path traversals and attempts to mark protected data safe. |
| Cache Catalog & Guard | Done | `caches.rs`: 48 fixed locations; `cleanup.rs` guard refuses protected entries (histories, models, configurations, browser profiles, Docker/WSL). |
| Read-Only Mode Default | Done | Backend initializes with `read_only = true`, gating `start_cleanup` and `clean_app_leftovers` until explicitly enabled. |
| Crash-Consistent Quarantine | Done | Files moved into `%APPDATA%\dev.cyron.uninstaller\quarantine\<run-id>\`; durable write-ahead recovery journal (`recovery_journal.jsonl`) physically synced (`sync_all`) before moves. |
| Atomic No-Overwrite Restore | Done | Restores use Win32 `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, preventing destination file overwrites. |
| Manifest-Scoped Purge | Done | `purge_quarantine` permanently deletes only files recorded in that batch manifest whose paths remain inside the batch folder. |
| Storage Caps & Low Space Protection | Done | Quarantine total storage is capped at 20 GB (`MAX_QUARANTINE_TOTAL_BYTES`); cleanup aborts if free space on the quarantine volume is below 512 MB. |

### 4. Windows Integration & Packaging
| Feature | Status | Verification & Evidence |
|---|---|---|
| Explorer Shell Integration | Done | `explorer.rs`: per-user shell verbs registered in `HKCU\Software\Classes` without elevation; safe CLI argument sanitization; toggleable and repairable via Settings. |
| Windows Metadata Standards | Done | Product name, version, publisher (`Chryypt`), UpgradeCode, and description enforced across PE, MSI, and NSIS (`WINDOWS_METADATA_OK`). |
| Chryypt Organization Code Signing | Done | `scripts/sign-build.ps1` signs binaries with Chryypt Organization certificate (`CHRYYPT_SIGNED`); signature integrity and negative tests verified (`SIGNATURES_VERIFIED_OK`). |
| Installer Bundling | Done | NSIS setup (`.exe`) and WiX MSI (`.msi`) generated and verified (`RELEASE_BUNDLES_OK`). |

### 5. Hunter Mode, Startup Manager & Release-Blocking Correctness (Milestones 7.1 & 7.2)
| Feature | Status | Verification & Evidence |
|---|---|---|
| Broken Inventory Entry Cleanup & Fingerprint Binding | Done | `leftovers.rs`: resolves actual subkey name from discovered application metadata (`registry_key_name`); requires 2-step single-use 5-minute plan (`BrokenEntryPlan`) with OS-backed cryptographic random plan IDs; binds plan to SHA-256 fingerprint of live registry metadata; re-reads and re-verifies live metadata and broken status immediately before deletion; rejects protected, working, recovered, altered, replayed, or expired plans; creates verified multi-type `.reg` backup in quarantine with durable recovery journal and manifest; provides discoverable in-app registry restoration via `restore_registry_backup` IPC command. Tested in `rc1_e2e_fixture.rs` [E2E 5/8] and unit tests. |
| Fail-Closed Recovery Integrity & Scope Protection | Done | `leftovers.rs` & `cleanup.rs`: eliminates ignored manifest write errors; writes durable write-ahead recovery journal (`sync_all`) and verified manifest prior to registry subkey deletion; manifest binds backup SHA-256 checksum, exact hive, subkey name, and application ID; enforces fail-closed scope checking on `.reg` file (rejecting any out-of-scope sections outside the approved key); detects live registry key conflicts and refuses silent overwrites; file quarantine restoration and purge never invalidate an outstanding registry recovery. Tested in `rc1_e2e_fixture.rs` [E2E 5/8] and unit tests. |
| Hunter Process Snapshot Isolation & Opaque Tokens | Done | `hunter.rs` & `commands.rs`: segregates Hunter snapshots into dedicated `AppState::hunter_snapshots` and `AppState::hunter_app_snapshots` using unique opaque tokens generated with OS-backed cryptographic randomness (`BCryptGenRandom`); completely isolated from uninstaller process snapshots; termination revalidates PID, path, and creation time immediately before kill; requires explicit operator confirmation; preserves critical system process protection. Tested in `security.rs` (`concurrent_hunter_and_uninstaller_snapshots_stay_isolated`) and `rc1_e2e_fixture.rs` [E2E 3/8]. |
| Hunter Mode High-Performance Targeting | Done | Separates cursor targeting from application discovery; queries cached in-memory apps (`state.cached_apps`) in <1 µs per call (eliminating 60+ ms repeated scan overhead); non-overlapping polling with sequence counter. Honest reproducible benchmark in `hunter_benchmark.rs` and `scripts/verify-hunter-perf.mjs` measures min/mean/p50/p95/p99 targeting latency and verifies bounded memory stability across 1,000 iterations. Tested in `rc1_e2e_fixture.rs` [E2E 7/8]. |
| Real External Window Drag & Lock Targeting | Done | `HunterMode.tsx`: pointer capture drag-and-drop targeting (`setPointerCapture`) across external windows; keyboard locking (Space/Enter to lock, Esc to cancel); strict directory boundary matching (`is_path_in_directory`) preventing prefix collisions. Verified with disposable Win32 GUI fixture with real HWND (`disposable_gui_target.rs`) in `rc1_e2e_fixture.rs` [E2E 1/8, 2/8]. |
| Unregistered Application Forced Removal | Done | `hunter.rs`: registers backend-verified `InstalledApp` snapshot for unregistered executables; protects system and user roots; reviews files and leftovers before removal. Tested in `rc1_e2e_fixture.rs` [E2E 4/8, 8/8]. |
| Startup Manager Toggling, Elevation & Payload Preservation | Done | `system.rs`: enumerates HKCU/HKLM Run/RunOnce, WOW6432Node, and Startup folders; checks write permissions (`is_hklm_startup_writable`), disabling machine toggles without elevation; preserves exact 12-byte raw binary structure, setting byte 0 (`0x02` enabled, `0x03` disabled) and updating FILETIME in bytes 4..12 matching Windows Task Manager; reversible per-user toggling; protects Windows Defender, SecurityHealthSystray, and SmartScreen. Tested in `rc1_e2e_fixture.rs` [E2E 6/8] and unit tests. |
| Hardware Diagnostics Improvements | Done | `system.rs`: native Windows architecture detection via `GetNativeSystemInfo`; discrete GPU preference and virtual adapter filtering; 5-second TTL specs cache. |

### 6. Explorer Destructive Actions & Hunter Navigation (Milestone 7.4)
| Feature | Status | Verification & Evidence |
|---|---|---|
| Hunter Mode Sidebar Navigation & Activation Dialog | Done | Moved to primary sidebar below Uninstaller as a global action; clicking triggers a compact confirmation dialog explaining window targeting, actions, and exit; cursor tracking only begins upon confirmation. Active sidebar indicator and `Esc` cancellation ensure no lingering intervals or stale targets. |
| Session-Bound Hunter HUD & Floating Triggers | Done | `HunterMode.tsx`: targeting HUD banner and crosshair trigger render strictly during active sessions (`isActive === true`); uninstaller header button eliminated. Tested across window resize and collapsed sidebar. |
| Windows Explorer Force Delete & Locking Processes | Done | `explorer.rs`: registers `Cyron.ForceDelete` and `Cyron.LockingProcesses` in HKCU; validates paths, blocks system/boot roots (`is_protected_delete_target`), prepares 10-minute expiring single-use execution plan (`StoredForceDeletePlan`); detects active file locks via Windows Restart Manager (`rstrtmgr.dll`); defaults to 7-day recoverable quarantine; verified permanent deletion with warning checks. Tested in unit tests and security suite. |
| Windows Explorer Kill Process | Done | `explorer.rs` & `process.rs`: registers `Cyron.KillProcess` for `.exe` files; matches running processes strictly by executable path; captures PID, window title, and creation time; terminates via core engine (`process::terminate_single_process`) with graceful closure and verified force termination. |
| Standard vs Advanced Explorer Settings | Done | `Settings.tsx`: provides independent toggles for standard and advanced/destructive context-menu verbs; displays HKCU registration status badges and repair actions. |

---

## Known Limitations & Design Boundaries

1. **Store / MSIX Lifecycle:** Windows Store packages are discovered from `AppModel\Repository\Packages` for inventory, file inspection, and residual scanning. Because Store removal requires Windows Runtime COM package management interfaces, traditional Win32 uninstallation is disabled with clear user guidance directing to Windows Settings or `winget`.
2. **Portable Applications:** Standalone portable executables in `%LOCALAPPDATA%\Programs` are discovered through explainable heuristics. They lack registry uninstaller commands; Cyron provides residual scanning, folder measurement, and Explorer navigation.
3. **MFT Scanner Elevation:** Accessing raw volume handles (`\\.\X:`) requires administrator elevation. When unelevated, Cyron reports the reason and falls back seamlessly to the streaming directory walk.
4. **MFT vs Walk Counting Difference:** The MFT reader counts physical NTFS records; the streaming directory walk counts accessible filesystem paths (deduplicating up to 1,000,000 hard-link IDs). Volume totals from the two methods differ by design (see [docs/BENCHMARKS.md](BENCHMARKS.md)).
5. **Debug Desktop Build:** The debug build executable (`src-tauri/target/debug/cyron.exe`) targets Tauri's `devUrl` and requires the Vite dev server (`npm run dev`). Production release builds embed frontend assets directly and run standalone.
6. **Platform Scope:** Cyron targets Windows 10/11 x64 exclusively. Win32 APIs are invoked unconditionally.
