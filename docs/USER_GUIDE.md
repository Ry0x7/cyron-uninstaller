# Cyron Uninstaller — User Guide

Cyron is a review-first Windows application inventory, disk space analyzer, and developer cache cleaner designed to give you complete visibility and control over your storage without unexpected data loss.

---

## 1. Overview Dashboard

The Overview view provides an instant snapshot of your system storage and developer caches:
- **System Drives**: Shows total and free capacity across all connected NTFS and FAT drives.
- **Developer Caches**: Inspects 48 known development and AI cache locations (Node, Python, Rust, Gradle, Docker, HuggingFace, Ollama, etc.).
- **Risk Indicators**: Clearly labels targets as **Safe** (regenerated automatically), **Caution** (re-download or configuration cost), or **Protected** (history, models, or essential system state).

---

## 2. Applications & Uninstallation

The Uninstaller view provides transparent management of all desktop software registered in the 64-bit and 32-bit Windows registry.

### Features
- **Application Search & Filtering**: Filter by publisher, name, or installation scope (machine-wide vs per-user).
- **Size Measurement**: Because many installers record inaccurate or missing sizes, use the **Measure folder** action to calculate exact on-disk byte totals.
- **Process Manager**:
  - Automatically identifies active processes whose executable files reside inside the application's installation folder.
  - Strictly protects critical Windows operating system processes (`csrss.exe`, `explorer.exe`, `services.exe`, `dwm.exe`, etc.).
  - Controlled termination: attempts graceful window closing (`WM_CLOSE`) first. Force termination requires explicit confirmation.
- **Standard Uninstaller**:
  - Validates registered uninstaller commands and MSI packages.
  - Shell interpreters (`cmd.exe`, `powershell.exe`) and arbitrary script wrappers are rejected for your safety.
  - Tracks process exit codes, including Windows restart requirement code `3010`.
- **Leftover & Residual Scanner**:
  - Discovers remaining directories and registry entries under `AppData`, `ProgramData`, and `HKCU`/`HKLM`.
  - Classifies items by risk and confidence.
  - Automatically creates a durable `.reg` registry backup before deleting any subkey.
  - Moves residual files into a recoverable quarantine batch rather than permanently deleting them.
- **Sequential Batch Uninstallation**:
  - Queue multiple applications for unattended sequential uninstallation.
  - Provides failure isolation: if one uninstaller fails or cancels, remaining applications continue.
  - Residual cleanup always requires a separate, explicit review.

---

## 3. Disk Analyzer & Treemap

Analyze where disk space went across any directory or drive:
- **Visual Treemap**: High-performance Squarified treemap rendering directory sizes proportionally.
- **Synchronized Hierarchy Tree**: Resizable virtualized folder tree synchronized with the treemap selection.
- **Scanning Methods**:
  - **Directory Walk**: Depth-first streaming traversal that tracks hard links and respects memory bounds.
  - **NTFS MFT Reader**: High-speed Master File Table parser for full volume scans when running with administrator privileges.
- **Safety**: Reparse points, symlinks, and directory junctions are never traversed.

---

## 4. Duplicate Finder

Locate redundant files safely:
- **Three-Stage Verification**:
  1. File size grouping to eliminate unique sizes immediately.
  2. 4 KiB edge hashing (start and end of candidate files).
  3. Full streaming SHA-256 hash comparison.
- **Hard-Link Awareness**: Hard links sharing the same physical cluster allocation on NTFS are identified and do not count toward wasted recoverable space.

---

## 5. Review-First Cache Cleaner

Clean accumulated build artifacts, package manager caches, and disposable temporary data:
- **Preview First**: Calculates exact file counts and byte totals before asking for approval.
- **Snapshot Binding**: Cleanup execution verifies that on-disk files match the approved preview snapshot. Any unexpected file created during the preview is preserved.
- **Durable Quarantine**: Files are renamed into an app-data quarantine batch alongside a crash-consistent write-ahead recovery journal (`recovery_journal.jsonl`).
- **Zero In-Place Deletions**: The cleaner moves files into quarantine; permanent deletion only occurs if you explicitly purge a batch.
- **Restoration**: Restore any quarantine batch at any time. Existing destination files are never overwritten.

---

## 6. Activity & Recovery Management

The Activity view maintains an audit log of past scans and quarantine batches:
- **Inspect Batches**: View quarantined files, original paths, file sizes, and creation timestamps.
- **Restore Batch**: Safely returns quarantined files to their original directories.
- **Purge Batch**: Permanently deletes files inside the batch folder and removes the quarantine record once confirmed.

---

## 7. Settings & Safety Controls

- **Read-Only Mode**: Enabled by default upon initial installation. When active, all destructive operations (cleanup, leftover quarantine, process termination) are completely blocked by the backend.
- **Exclusion Folders**: Define specific absolute directory paths that the scanner and cleaner must never touch.
- **Theme**: Choose between System, Light, and Dark themes with zero theme flicker on startup.
