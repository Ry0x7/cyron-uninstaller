# Cyron — Historical Phase Archive (Phases 1–3)

This document preserves the detailed development notes, early acceptance logs, and historical milestone narratives for Cyron Phases 1, 2, and 3. For the active, maintained feature matrix, see [docs/IMPLEMENTATION_STATUS.md](../IMPLEMENTATION_STATUS.md). For current and upcoming roadmap items, see [ROADMAP.md](../../ROADMAP.md).

---

## Historical Phase 1 Log — Foundation and Read-Only Scanning

### Scope Delivered:
- **Tauri v2 + React 19 + TypeScript** scaffold with modular Rust backend.
- Hand-built design system with light/dark themes, collapsible sidebar, and 6-section navigation (`src/styles/tokens.css`, `src/styles/app.css`, `src/components/ui.tsx`).
- Installed application discovery reading four Windows uninstall registry views (64-bit and 32-bit under `HKLM` and `HKCU`), filtering system components and updates.
- Detail drawer reporting publisher, version, install date, architecture, estimated size, and install location.
- Native Win32 directory enumeration (`GetFileInformationByHandleEx`) with logical size and allocation size reporting, plus `std::fs` fallback.
- Depth- and output-capped cancellable filesystem scan with throttled progress events and junction skipping.
- Squarified treemap with interactive drill-down and details synchronization.
- Synchronized WinDirStat-style folder tree view.
- Largest files (top 100) and extension breakdown (top 24).
- 48-location developer, AI, and Windows cache catalog with Safe / Caution / Protected risk tiers.
- System overview dashboard with volume capacities, application count, and cache measurements.
- Local scan history persisted in `%APPDATA%\dev.cyron.uninstaller\history.json` (max 100 entries).
- Local settings in `localStorage` for theme, hidden/system files, and exclusion paths.
- Read-only boundary: no file modifications, process terminations, or registry writes existed.

---

## Historical Phase 2 Log — Disk Intelligence & Hybrid Engine

### Scope Delivered:
- **Direct NTFS MFT Parser (`src-tauri/src/mft.rs`)**: Reads volume boot sector, parses `$MFT` run list, applies update-sequence fixups, parses `FILE_NAME`, `$DATA`, and `$STANDARD_INFORMATION` attributes into an in-memory tree. Measured live on `C:\`: 2,211,292 files / 604,557 directories in 14.52 s. Requires administrator elevation; falls back cleanly to directory walk if unelevated or on non-NTFS volumes.
- **Directory Walk Streaming Rewrite (`src-tauri/src/scan.rs`)**: Single-threaded streaming directory traversal bounding hard-link tracking at 1,000,000 IDs, capping tree nodes at 100,000, children per folder at 200, and file nodes per folder at 40.
- **Automatic Method Selection**: Drive roots (e.g. `C:\`) automatically select MFT when elevated; subfolders select streaming directory walks.
- **Persistent Virtualized Tree Column (`TreeView.tsx`)**: Resizable 220–520 px split column beside the treemap, virtualized with no row cap and full ARIA tree keyboard navigation.
- **Advanced Filters**: Name search, minimum size thresholds, file age filters, extension filtering, and multi-criteria sorting.
- **3-Stage Duplicate Verification (`duplicates.rs`)**: File size grouping -> 4 KiB edge hashing -> full streaming SHA-256. Hard links identified and excluded from recoverable waste calculation.
- **Storage Categorization Intelligence (`storage.rs`)**: 10 storage categories with known vs inferred confidence ratings, 5 age distribution buckets, and explainable storage insights.
- **Reproducible Scan Benchmarks (`scripts/bench-scan.mjs`)**: Verified warm-cache timing and peak working set across multiple directory scales.

---

## Historical Phase 3 Log — Review-First Safe Cleaner

### Scope Delivered:
- **Review-First Cache Cleaning Engine (`src-tauri/src/cleanup.rs`)**: Resolves catalog IDs against validated specifications, enforces path guards, executes only server-held expiring plans (`CleanupPlan`, 10m TTL), and requires explicit user confirmation.
- **Crash-Consistent Quarantine**: Files are moved via Win32 renames into `%APPDATA%\dev.cyron.uninstaller\quarantine\<run-id>\` rather than deleted in place. Every move is recorded in `manifest.json`.
- **Durable Write-Ahead Recovery Journal**: `recovery_journal.jsonl` physically flushed and synced (`sync_all`) before each file move.
- **Atomic No-Overwrite Restoration**: Quarantined files restored using `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, preventing destination overwrites.
- **Catalog Risk Model**: 48 catalog entries categorized as `safe` (regenerates), `caution` (re-download cost), or `protected` (history, credentials, models, virtual disks). Protected entries and their ancestors are strictly refused by the backend.
- **Manifest-Scoped Purging**: `purge_quarantine` is permanent but strictly scoped to files recorded in that specific batch manifest whose paths remain inside the batch directory.
- **Read-Only Mode Default**: Backend starts with `read_only = true`, requiring user toggle before cleaning operations are permitted.
- **Audit Logging**: `cleanup-log.json` maintains the newest 100 cleanup, restore, and purge operations.
