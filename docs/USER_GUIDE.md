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

The Uninstaller view provides transparent management of all desktop software, modern store packages, and gaming installations across your system.

### Features
- **Multi-Source Inventory**:
  - **Win32 Applications**: Standard 64-bit and 32-bit software registered in the Windows uninstall registry.
  - **Microsoft Store / AppX / MSIX**: Modern packaged apps enumerated via `AppModel\Repository\Packages`.
  - **Steam Games**: Multi-library game discovery reading `libraryfolders.vdf` and `appmanifest_*.acf`.
  - **Portable Applications**: Discovers standalone executables installed under `%LOCALAPPDATA%\Programs`.
  - **Broken Records & Review-First Cleanup**:
  - Flags uninstall entries whose executable or uninstall string targets missing binaries.
  - Safe 2-step cleanup: prepares an expiring plan (`BrokenEntryPlan`) bound to a SHA-256 fingerprint of live registry metadata.
  - Re-reads and re-verifies live metadata and broken qualification immediately before deletion. Altered, tampered, or recovered entries are refused.
  - Generates an automatic multi-type `.reg` backup in quarantine with a crash-consistent recovery journal and manifest.
  - Provides instant in-app registry restoration ("Restore Registry Now" button in the wizard) and discoverable quarantine recovery. Protected Windows components are strictly refused.
- **Source Filtering & Badges**: Filter easily between `All`, `Win32`, `Store`, `Steam`, `Portable`, and `Broken` records.
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
- **Direct Cleaner Integration**: Selecting recognized developer or cache folders (e.g., `.gradle`, `.cargo`, `node_modules`, pip cache, NuGet) displays a cleanable callout in Selection Details, showing expected reclaimable space and providing a 1-click **Review for Cleanup** shortcut directly to the Cleaner drawer.
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
- **Declarative Providers**: Built upon versioned (`schema_version: 1`) declarative provider definitions covering developer tooling, package managers, and temporary caches.
- **Preview First**: Calculates exact file counts and byte totals before asking for approval.
- **Snapshot Binding**: Cleanup execution verifies that on-disk files match the approved preview snapshot. Any unexpected file created during the preview is preserved.
- **Durable Quarantine**: Files are renamed into an app-data quarantine batch alongside a crash-consistent write-ahead recovery journal (`recovery_journal.jsonl`).
- **Zero In-Place Deletions**: The cleaner moves files into quarantine; permanent deletion only occurs if you explicitly purge a batch.
- **Restoration**: Restore any quarantine batch at any time. Existing destination files are never overwritten.

---

## 6. Activity & Recovery Management

The Activity view maintains an audit log of past scans and quarantine batches:
- **Inspect Batches**: View quarantined files, original paths, file sizes, creation timestamps, and associated `.reg` registry backups.
- **Restore Batch**: Safely returns quarantined files to their original directories and supports in-app `.reg` registry restoration via verified import.
- **Purge Batch**: Permanently deletes files inside the batch folder and removes the quarantine record once confirmed.

---

## 7. Settings & Safety Controls

- **Read-Only Mode**: Enabled by default upon initial installation. When active, cache cleanup (`start_cleanup`) and uninstaller leftover quarantine (`clean_app_leftovers`) are completely blocked by the backend until deliberately toggled off. Process management and uninstaller execution are independently governed by single-use server confirmation plans, process snapshot verification, and critical Windows process allowlists.
- **Windows Explorer Integration**:
  - Optional native right-click integration registered cleanly under `HKCU\Software\Classes` without needing administrator rights.
  - **Folders & Drives**: "Analyze with Cyron", "Find Duplicates with Cyron", and "View Storage Breakdown".
  - **Executables (`.exe`)**: "Uninstall with Cyron" (opens registered uninstaller review) and "Find Related Files" (opens residual file review).
  - Single-click **Enable**, **Disable**, and **Repair** controls in Settings. Clean uninstallation completely removes all registered shell verbs.
---

## 8. Hunter Mode & Window Targeting

Hunter Mode allows you to identify, inspect, terminate, or uninstall any visible application running on your desktop:
- **Targeting**:
  - Drag the targeting crosshair onto any application window on your desktop using pointer capture.
  - Alternatively, use the keyboard (`Space` or `Enter` to lock target under cursor, `Esc` to cancel).
- **Fast Process & App Matching**:
  - Resolves target window and process identities against Cyron's cached inventory in under 1 microsecond without repeating system-wide scans.
  - Strict directory boundary checks ensure correct matching (e.g., `C:\Apps\Foo` never matches `C:\Apps\FooBar`).
- **Verified Actions**:
  - **Process Termination**: Terminates targeted processes with verified PID, image path, and creation time checks immediately before termination, backed by an explicit confirmation dialog.
  - **Uninstall**: For registered software, opens the standard Uninstaller wizard with process closure and leftover detection.
  - **Forced Removal**: For unregistered executables, registers a backend-verified app snapshot and opens review-first residual inspection while protecting Windows system roots.

---

## 9. Startup Manager

Manage programs configured to start automatically with Windows:
- **Comprehensive Coverage**: Discovers startup registrations across HKCU/HKLM `Run` and `RunOnce`, WOW6432Node keys, and user/all-users Startup folders.
- **Reversible Toggling**: Disables or enables startup items per-user using Windows standard `StartupApproved` binary values (`0x02` enabled, `0x03` disabled) while preserving exact 12-byte payloads and updating FILETIME timestamps matching Windows Task Manager.
- **Elevation Awareness**: Machine-wide entries (HKLM / all users) require administrator privileges; Cyron verifies write permissions and cleanly disables toggles when unelevated rather than indicating misleading success.
- **System Protection**: Critical Windows security components (Windows Defender, SecurityHealthSystray, SmartScreen) are clearly badged as **System Protected** and cannot be disabled.
- **Operator Review**: Modifying any startup entry requires explicit confirmation.
