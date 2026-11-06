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
- Two-track code signing architecture: local development self-signing (`DEV_TEST_SIGNED`, `SIGNATURES_VERIFIED_OK`) and prepared SignPath Foundation public release automation (`SIGNPATH_SIGNED`).
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
| Windows Metadata Standards | Done | Product name, version, publisher (`Ry0x7`), UpgradeCode, and description enforced across PE, MSI, and NSIS (`WINDOWS_METADATA_OK`). |
| Local Dev Code Signing | Done | `scripts/sign-dev-build.ps1` signs binaries with local self-signed certificate (`DEV_TEST_SIGNED`); signature integrity and negative tests verified (`SIGNATURES_VERIFIED_OK`). |
| SignPath Foundation Integration | Prepared | Prepared `.github/workflows/signpath-release.yml` and `docs/CODE_SIGNING_POLICY.md` pending open-source onboarding. |
| Installer Bundling | Done | NSIS setup (`.exe`) and WiX MSI (`.msi`) generated and verified (`RELEASE_BUNDLES_OK`). |

---

## Known Limitations & Design Boundaries

1. **Store / MSIX Lifecycle:** Windows Store packages are discovered from `AppModel\Repository\Packages` for inventory, file inspection, and residual scanning. Because Store removal requires Windows Runtime COM package management interfaces, traditional Win32 uninstallation is disabled with clear user guidance directing to Windows Settings or `winget`.
2. **Portable Applications:** Standalone portable executables in `%LOCALAPPDATA%\Programs` are discovered through explainable heuristics. They lack registry uninstaller commands; Cyron provides residual scanning, folder measurement, and Explorer navigation.
3. **MFT Scanner Elevation:** Accessing raw volume handles (`\\.\X:`) requires administrator elevation. When unelevated, Cyron reports the reason and falls back seamlessly to the streaming directory walk.
4. **MFT vs Walk Counting Difference:** The MFT reader counts physical NTFS records; the streaming directory walk counts accessible filesystem paths (deduplicating up to 1,000,000 hard-link IDs). Volume totals from the two methods differ by design (see [docs/BENCHMARKS.md](BENCHMARKS.md)).
5. **Debug Desktop Build:** The debug build executable (`src-tauri/target/debug/cyron.exe`) targets Tauri's `devUrl` and requires the Vite dev server (`npm run dev`). Production release builds embed frontend assets directly and run standalone.
6. **Platform Scope:** Cyron targets Windows 10/11 x64 exclusively. Win32 APIs are invoked unconditionally.
