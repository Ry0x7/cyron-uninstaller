# Cyron Uninstaller — User Guide

Cyron is a review-first Windows application inventory, disk space analyzer, and developer cache cleaner designed to give you complete visibility and control over your storage without unexpected data loss.

---

## 1. Overview Dashboard

The Overview view provides an instant snapshot of your system storage and developer caches:
- **System Drives**: Shows total and free capacity across all connected NTFS and FAT drives.
- **Developer Caches**: Inspects 51 known development and AI cache locations (Node, Python, Rust, Gradle, Docker, HuggingFace, Ollama, etc.).
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
  - This Uninstaller residual-file path uses same-volume renames and reports failures for files on another volume. For a removal recorded in this session, Cleaner’s **Known removed applications** review supports cross-volume filesystem quarantine through its standard cleanup engine.
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
- **Cleaner quarantine**: The cleaner moves files into quarantine; permanent deletion only occurs if you explicitly purge a batch.
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
  - **Standard Verbs**: "Analyze with Cyron", "Find Duplicates with Cyron", "View Storage Breakdown", "Uninstall with Cyron", and "Find Related Files".
  - **Advanced & Destructive Verbs**:
    - **Files & Folders**: "Force Delete with Cyron" and "Find Locking Processes with Cyron".
    - **Executables (`.exe`)**: "Kill Running Process with Cyron".
  - **Review Dialogs**: All actions open a dedicated Cyron review modal; operations are never executed directly from shell arguments.
  - **Force Delete**: Analyzes target attributes, validates canonical path safety against Windows/boot/system roots, queries active file locks via Windows Restart Manager (`rstrtmgr.dll`), and routes deleted items through a 7-day recoverable quarantine batch by default. Permanent deletion requires explicit multi-step confirmation.
  - **Find Locking Processes**: Queries Restart Manager for all processes currently holding open handles on the selected file or directory. Offers graceful window closure (`WM_CLOSE`) and verified force termination with PID creation-time checks.
  - **Kill Running Process**: Matches running processes against the clicked executable's canonical path, displaying PID, creation time, and window title. Never terminates by filename alone, and unconditionally protects critical Windows system processes.
  - **Independent Settings Toggles**: Separate controls for Standard and Advanced verbs with live HKCU status indicators and one-click Repair actions.

---

## 8. Hunter Mode & Window Targeting

Hunter Mode allows you to identify, inspect, terminate, or uninstall any visible application running on your desktop:
- **Global Sidebar Access**: Hunter Mode is positioned in the primary sidebar navigation directly below Uninstaller as a global session action, without routing away from your current workspace.
- **Activation Confirmation**: Clicking Hunter Mode opens a compact confirmation dialog explaining window targeting, available actions, and exit controls. Cursor tracking and target polling only begin after confirmation.
- **Session Lifecycle & Active Indicators**:
  - The sidebar displays an animated active status badge with an inline exit button.
  - A top-center targeting HUD and crosshair trigger are visible strictly during an active Hunter session and disappear immediately upon exit.
  - Press `Esc` or click "Exit Hunter Mode" at any time to immediately cancel the session without leaving lingering timers or stale background targets.
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

## Reviewed Analyzer actions and Cleaner scope

In Disk Analyzer, select a file or folder in the listing or treemap, then use **Add selected item**. Add more items as needed, or exclude paths from the selection. **Move to Recycle Bin…** and **Permanently delete…** open separate reviews showing the exact paths, content counts and estimated logical bytes. Confirm only after reviewing the operation. Permanent deletion bypasses quarantine; Cyron cannot restore it. Recycling does not free space until Windows empties its bin. Files are recycled separately; empty selected folders are removed only when empty. Cancel stops the remaining work between items. Review any failures; the Analyzer refreshes after an attempt.

Cleaner can scan selected mounted local drives for its supported locations. It does not search every file on a drive for things to delete. Path, size, modification time, risk and consequences remain visible in the review; exclusions remain available through Settings. Unsupported generic temporary folders and installer servicing stores are left alone. Retained identities for applications removed during this session can be reviewed separately under known removed applications; unknown historical leftovers are not inferred from folder names. Quarantined space is still on disk until an explicit purge.

Uninstaller distinguishes a verified missing inventory entry from an uninstaller that only exited. **Remaining**, **Restart required**, **Cancelled**, and failed items need follow-up. Batch **Stop after current app** lets the vendor wizard finish and skips the rest of the queue. Store packages show Windows Settings and winget guidance.
