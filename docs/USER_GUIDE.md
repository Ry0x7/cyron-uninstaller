# Cyron Uninstaller — User Guide

Cyron is a review-first Windows application inventory, disk space analyzer, and developer cache cleaner designed to give you complete visibility and control over your storage without unexpected data loss.

---

## 1. Home

Home is the summary destination for this PC:
- **System summary**: Windows version, processor, memory, graphics and uptime, read from the local system.
- **Capacity**: Free space and used percentage across every ready local volume, with an **Open disk space** shortcut.
- **Installed applications**: Counts from the Windows inventory, split by all users and this account.
- **Cleanup potential**: Measured developer, AI and Windows cache bytes, with safe, review and protected categories.
- **Recent activity**: The latest scan or cleanup from History & Recovery.

Every figure comes from a real local measurement. A loading placeholder means the measurement is still running; Cyron does not fill the space with an invented score.

---

## 2. Applications

The Applications destination provides transparent management of all desktop software, modern store packages, and gaming installations across your system. **Find by window** in the page header launches Hunter Mode for the application behind a desktop window.

### Features
- **Multi-Source Inventory**:
  - **Win32 Applications**: Standard 64-bit and 32-bit software registered in the Windows uninstall registry.
  - **Microsoft Store / AppX / MSIX**: Modern packaged apps enumerated for the current user through the Windows Runtime package manager, with a read-only registry fallback.
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
  - Residual files use Windows cross-volume moves into quarantine and never overwrite an existing quarantine destination. A failed move removes its journal entry and leaves the original in place.
  - Name-only matches are shown as caution and are not preselected. Only the exact recorded install folder or the exact registration entry is treated as owned evidence.
  - Cleaner’s **Known removed applications** also lists persistent removal history and offers an explicit name/path scan for apps removed before Cyron was installed; those matches always require review.
- **Sequential Batch Uninstallation**:
  - Queue multiple applications for unattended sequential uninstallation. Only applications with a registered Win32 vendor uninstaller can be selected; Store packages, Steam games, portable applications and broken registrations stay out of the queue.
  - The batch is refused as a whole if any selected application is not eligible, instead of silently running a subset.
  - Provides failure isolation: if one uninstaller fails or cancels, remaining applications continue.
  - Residual cleanup always requires a separate, explicit review.

---

## 3. Storage & Cleanup — Disk space

Open **Storage & Cleanup**, then **Disk space** to analyze where space went across any directory or drive:
- **Visual Treemap**: High-performance Squarified treemap rendering directory sizes proportionally.
- **Synchronized Hierarchy Tree**: Resizable virtualized folder tree synchronized with the treemap selection.
- **Direct Cleaner Integration**: Selecting recognized developer or cache folders (e.g., `.gradle`, `.cargo`, `node_modules`, pip cache, NuGet) displays a cleanable callout in Selection Details, showing expected reclaimable space and providing a 1-click **Review for Cleanup** shortcut directly to the Cleaner drawer.
- **Scanning Methods**:
  - **Directory Walk**: Depth-first streaming traversal that tracks hard links and respects memory bounds.
  - **NTFS MFT Reader**: High-speed Master File Table parser for full volume scans when running with administrator privileges.
- **Safety**: Reparse points, symlinks, and directory junctions are never traversed.

---

## 4. Storage & Cleanup — Duplicates

In **Storage & Cleanup → Disk space → Duplicates**, locate redundant files safely:
- **Three-Stage Verification**:
  1. File size grouping to eliminate unique sizes immediately.
  2. 4 KiB edge hashing (start and end of candidate files).
  3. Full streaming SHA-256 hash comparison.
- **Hard-Link Awareness**: Hard links sharing the same physical cluster allocation on NTFS are identified and do not count toward wasted recoverable space.

---

## 5. Storage & Cleanup — Cleanup

Open **Storage & Cleanup → Cleanup** to clean accumulated build artifacts, package manager caches, and disposable temporary data:
- **Declarative Providers**: Built upon versioned (`schema_version: 1`) declarative provider definitions covering developer tooling, package managers, and temporary caches.
- **Preview First**: Calculates exact file counts and byte totals before asking for approval.
- **Snapshot Binding**: Cleanup execution verifies that on-disk files match the approved preview snapshot. Any unexpected file created during the preview is preserved.
- **Durable Quarantine**: Files are renamed into an app-data quarantine batch alongside a crash-consistent write-ahead recovery journal (`recovery_journal.jsonl`).
- **Cleaner quarantine**: The cleaner moves files into quarantine; permanent deletion only occurs if you explicitly purge a batch.
- **Restoration**: Restore any quarantine batch at any time. Existing destination files are never overwritten.

### Developer projects

Open **Storage & Cleanup → Cleanup → Projects** to find build and dependency folders without opening each repository. Cyron scans selected local drives and user-added folders, recognises projects from real manifests and Git roots, and measures only named artifacts:

- Installed dependencies: `node_modules` (a lockfile raises the evidence confidence; a missing lockfile is called out).
- Build output: `dist`, `build`, Rust/Maven `target`, Gradle `build` and project `.gradle` caches, and Next.js `.next/cache` (the rest of `.next` is not selected).
- Tool caches: `.turbo`, `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`.

Every artifact shows its exact path, logical and on-disk size, file count, last write time, why Cyron believes it belongs to the project, the confidence, what is lost while it is in quarantine, and how to rebuild it. **Old or stale is not permission**: activity labels are evidence, and nothing is selected automatically. Source code, `.git`, `.env` files, credentials, keys, databases and model weights are listed as protected and never become cleanup targets.

Select only the artifacts you reviewed, choose **Preview project cleanup**, inspect every target, then confirm. Files move to the same recoverable quarantine as the cache cleaner. After the run, Cyron re-measures the selected artifacts, reports any files that remained because they changed after the preview, and lists the quarantine batches you can restore.

Tool cache cleanup for Claude Code, Codex, Gemini CLI, Cursor, VS Code, Antigravity, Freebuff, Ollama, LM Studio, Hugging Face, npm, pnpm, Yarn, pip, uv, Poetry, Cargo, Gradle, Docker and WSL remains in the main **Cleanup** section. Model weights, conversation histories, LM Studio presets, Hugging Face tokens and Docker/WSL disks are protected by default. For model caches, use the owning tool's reviewed prune command (for example `hf cache prune`) rather than deleting model files.

---

## 6. History & Recovery

History & Recovery has two tabs. **Activity** lists past scans and cleanup outcomes; **Recovery** manages quarantined batches and the cleanup log. A run that was stopped or failed after moving some files is labelled **Partial**; the files already moved stay in quarantine and can be restored from **Recovery**.
- **Inspect Batches**: View quarantined files, original paths, file sizes, creation timestamps, and associated `.reg` registry backups.
- **Restore Batch**: Safely returns quarantined files to their original directories and supports in-app `.reg` registry restoration via verified import. Existing files are never overwritten.
- **Purge Batch**: Permanently deletes files inside the batch folder and removes the quarantine record once confirmed. This is the only way a quarantine batch is permanently removed.
- **Clear history**: Removes the local operation records only; it does not touch files on disk or empty the quarantine.

---

## 7. Settings & Safety Controls

Open **Settings** from the sidebar footer.

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
- **Software updates**: Open **Settings → About → Software updates** and choose **Check for updates**. Cyron checks the signed GitHub Releases manifest only when you ask; it does not poll in the background or send telemetry. If a newer version exists, review its version and notes, confirm the native prompt, and let Cyron download and verify the package. The installer closes and restarts Cyron when it finishes. A failed check or download leaves the installed version unchanged. Existing 0.2.1 installations require one manual 0.2.2 install before this update path exists. In-app updates apply to the per-user NSIS setup. MSI installations show that they must update from the published release page instead, so a per-machine install is never replaced by a second per-user copy.

---

## 8. Hunter Mode & Window Targeting

Hunter Mode allows you to identify, inspect, terminate, or uninstall any visible application running on your desktop:
- **Applications Action**: Click **Find by window** in the Applications header. Hunter Mode is not a separate navigation page.
- **Activation Confirmation**: The action opens a compact confirmation dialog explaining window targeting, available actions, and exit controls. Cursor tracking and target polling only begin after confirmation.
- **Session Lifecycle & Active Indicators**:
  - An active targeting indicator with an inline exit button is shown only while the session is running.
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

## 9. Performance — Startup & scheduled tasks

Open **Performance** and use the startup section to manage programs configured to start automatically with Windows:
- **Comprehensive Coverage**: Discovers startup registrations across HKCU/HKLM `Run` and `RunOnce`, WOW6432Node keys, and user/all-users Startup folders.
- **Reversible Toggling**: Disables or enables startup items per-user using Windows standard `StartupApproved` binary values (`0x02` enabled, `0x03` disabled) while preserving exact 12-byte payloads and updating FILETIME timestamps matching Windows Task Manager.
- **Elevation Awareness**: Machine-wide entries (HKLM / all users) require administrator privileges; Cyron verifies write permissions and cleanly disables toggles when unelevated rather than indicating misleading success.
- **System Protection**: Critical Windows security components (Windows Defender, SecurityHealthSystray, SmartScreen) are clearly badged as **System Protected** and cannot be disabled.
- **Operator Review**: Modifying any startup entry requires explicit confirmation.

## 10. Performance Diagnostics

Open **Performance** in the sidebar. Cyron samples CPU, memory, physical-disk pressure, fixed-volume free space, process usage and startup evidence every ten seconds while the page is visible; **Pause** stops the automatic refresh and **Refresh** takes a new sample immediately.

- **Why is my PC slow?**: choose the symptom that matches what you see, including **CPU spikes / kernel time**. Cyron filters the evidence and shows only a recommendation the sample supports. If nothing supports a change, it says so instead of inventing one. A short sample cannot explain every slowdown.
- **Catch a short spike**: a normal sample lasts 700 ms and can miss a brief lag. **Record 20 seconds** takes about 20 one-second readings and keeps the peak CPU, kernel and physical-disk values, plus the process using the most CPU at that point. The report says whether the spike was process-, kernel- or disk-dominated. You can stop early; nothing is written to disk, and leaving the page ends the capture.
- **Kernel, drivers and recent changes**: kernel time has no process row. When it dominates, Cyron shows the kernel share of busy CPU, the application CPU it does not explain, installed display/storage/network driver versions and package dates, and matching driver, storage, power or service errors from the Windows System log over the last seven days. A recent driver date beside a new slowdown is a lead, not proof of cause. Windows does not attribute DPC/ISR time to one driver in these counters; the optional trace is how you inspect that detail. **Open Reliability Monitor** shows the Windows reliability history without changing anything.
- **Startup and scheduled tasks**: each startup entry shows its exact command, source and state. Boot impact comes only from Windows' administrator-only Diagnostics-Performance log; without that access the page says **Unavailable**. Enabling or disabling an entry is reversible, preserves the registration, and asks for confirmation in a native dialog. Machine-wide entries need administrator rights and stay unavailable otherwise. Scheduled tasks are listed read-only; Cyron never disables or runs them.
- **Before and after**: record a labeled measurement, make the change you want to evaluate, then record another. The page compares CPU, memory, commit, disk busy and recorded boot time. Two samples show association, not causation, and Cyron does not claim a guaranteed speed gain.
- **Advanced diagnostic trace**: only helpful when the basic metrics and the 20-second capture cannot explain the lag. It requires Windows Performance Recorder and an administrator session, stays on this PC, and is never uploaded. Cyron does not elevate itself; reopen it with **Run as administrator** if you choose to trace.

Cyron does not offer RAM boosters, registry cleaning, service disabling, Prefetch deletion, page-file changes, Defender/update disabling or driver tweaks. Those are not supported performance fixes. Driver dates and event-log records are evidence for your own troubleshooting; Cyron never installs, rolls back or removes a driver. Recommendations carry their measured evidence, expected effect, risk and a way to verify the result.

## 11. Search files with Everything

Open **Search** in the sidebar to find files and folders by name across every drive that the local [Everything](https://www.voidtools.com/) index covers. Cyron uses the index that Everything already maintains; it does not create its own background index.

- **Setup**: install Everything 1.4.1 or newer and keep it running. Cyron does not bundle, install or start Everything, and it cannot search when Everything is closed. If Everything is still loading its index, the page says so and results may stay empty until the index finishes. Use **Check Everything** after starting it.
- **Read-only**: Search never adds, renames or deletes files, never changes Everything's index or settings, and never uses a network service. Results are only shown; revealing one opens File Explorer at that path.
- **Query syntax**: Everything's normal filename matching applies. For example, `report` matches names containing that word, `ext:pdf` limits by extension, `folder:` matches folders, and quoted text keeps a phrase together. Cyron passes the query to Everything unchanged.
- **Results**: each row shows the name, containing folder, size (for files) and last-modified date. Select a row or press Enter to reveal it in File Explorer. **Load more** continues through a large result set.
- **Unavailable state**: when Everything is missing, older than 1.4.1, closed or not answering, the page explains which condition applies and how to fix it. A loading index is called out explicitly; neither state is disguised as a successful empty search.

Everything is a separate third-party application. Cyron does not control its indexing rules or which drives it includes; adjust those in Everything itself.

---

## Reviewed Analyzer actions and Cleaner scope

In **Storage & Cleanup → Disk space**, select a file or folder in the listing or treemap, then use **Add selected item**. Add more items as needed, or exclude paths from the selection. **Move to Recycle Bin…** and **Permanently delete…** open separate reviews showing the exact paths, content counts and estimated logical bytes. Confirm only after reviewing the operation. Permanent deletion bypasses quarantine; Cyron cannot restore it. Recycling does not free space until Windows empties its bin. Files are recycled separately; empty selected folders are removed only when empty. Cancel stops the remaining work between items. Review any failures; the Analyzer refreshes after an attempt.

Cleanup can scan selected mounted local drives for its supported locations. It does not search every file on a drive for things to delete. Path, size, modification time, risk and consequences remain visible in the review; exclusions remain available through Settings. Unsupported generic temporary folders and installer servicing stores are left alone. Applications removed during this session and persistent removal history appear under known removed applications. You can explicitly scan for an app removed before Cyron was installed by supplying its name, publisher, or install path; such matches are never preselected and remain unverified until you review them. Quarantined space is still on disk until an explicit purge.

Uninstaller distinguishes a verified missing inventory entry from an uninstaller that only exited. **Remaining**, **Restart required**, **Cancelled**, and failed items need follow-up. Batch **Stop after current app** lets the vendor wizard finish and skips the rest of the queue. For Store packages, Cyron asks Windows Package Manager to remove the package for the current Windows account only; if Windows policy denies it, use Settings → Apps → Installed apps or winget.
