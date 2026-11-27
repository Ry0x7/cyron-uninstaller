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

### System health

Open **Storage & Cleanup → Cleanup → System health** and choose **Scan registry** to find two kinds of stale Windows data:

- **Startup entries**: a Run or RunOnce registry value whose program no longer exists after `%VAR%` expansion. Startup-folder shortcuts and bare command names are not treated as dead because Cyron cannot prove their target is missing.
- **Uninstall registrations**: an installed-application entry whose uninstaller and install folder are both gone. Windows components, protected keys and shared vendor roots are excluded.

Every row states why it is considered dead and shows the exact registry value or key. Entries that need an elevated Cyron session are marked **Needs elevation**; **Windows security** startup entries are protected and cannot be removed. Removal requires cleaning to be enabled, then a confirmation dialog. Before deleting, Cyron re-reads the live entry and refuses anything that changed or reappeared. A dead startup value is exported to a verified single-value `.reg` backup and recorded in the recovery journal; a dead uninstall registration uses the same recursive backup, fingerprint check and journal as broken-entry cleanup. Both backups can be restored from **History & Recovery → Recovery**. Re-scan after a removal to refresh the list.

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

Open **Performance** in the sidebar. It has four tabs:

- **Monitor**: a Task Manager-style view. Pick CPU, Memory, a disk, a network adapter or a GPU on the left to see 60 seconds of history, live figures and hardware details. The CPU view can switch to one chart per logical processor. Hover a chart to read an exact value.
- **Processes**: every running process grouped into Apps, Background processes and Windows processes, with live CPU, memory and disk columns. Click a column to sort, search by name, or switch to a flat list. Each row can open the program's file location; Cyron never ends or changes processes.
- **Startup** and **Diagnose**: the startup list and the diagnostics described below. These take a fuller sample every ten seconds while visible.

Monitor and Processes update once a second while visible. **Pause** stops automatic updates; **Refresh** takes a new diagnostic sample immediately.

- **Why is my PC slow?**: choose the symptom that matches what you see, including **CPU spikes / kernel time**. Cyron filters the evidence and shows only a recommendation the sample supports. If nothing supports a change, it says so instead of inventing one. A short sample cannot explain every slowdown.
- **Catch a short spike**: a normal sample lasts 700 ms and can miss a brief lag. **Record 20 seconds** takes about 20 one-second readings and keeps the peak CPU, kernel and physical-disk values, plus the process using the most CPU at that point. The report says whether the spike was process-, kernel- or disk-dominated. You can stop early; nothing is written to disk, and leaving the page ends the capture.
- **Kernel, drivers and recent changes**: kernel time has no process row. When it dominates, Cyron shows the kernel share of busy CPU, the application CPU it does not explain, installed display/storage/network driver versions and package dates, and matching driver, storage, power or service errors from the Windows System log over the last seven days. A recent driver date beside a new slowdown is a lead, not proof of cause. Windows does not attribute DPC/ISR time to one driver in these counters; the optional trace is how you inspect that detail. **Open Reliability Monitor** shows the Windows reliability history without changing anything.
- **Startup and scheduled tasks**: each startup entry shows its exact command, source and state. Boot impact comes only from Windows' administrator-only Diagnostics-Performance log; without that access the page says **Unavailable**. Enabling or disabling an entry is reversible, preserves the registration, and asks for confirmation in a native dialog. Machine-wide entries need administrator rights and stay unavailable otherwise. Scheduled tasks are listed read-only; Cyron never disables or runs them.
- **Before and after**: record a labeled measurement, make the change you want to evaluate, then record another. The page compares CPU, memory, commit, disk busy and recorded boot time. Two samples show association, not causation, and Cyron does not claim a guaranteed speed gain.
- **Advanced diagnostic trace**: only helpful when the basic metrics and the 20-second capture cannot explain the lag. It requires Windows Performance Recorder and an administrator session, stays on this PC, and is never uploaded. Cyron does not elevate itself; reopen it with **Run as administrator** if you choose to trace.

Cyron does not offer RAM boosters, registry cleaning, service disabling, Prefetch deletion, page-file changes, Defender/update disabling or driver tweaks. Those are not supported performance fixes. Driver dates and event-log records are evidence for your own troubleshooting; Cyron never installs, rolls back or removes a driver. Recommendations carry their measured evidence, expected effect, risk and a way to verify the result.

## 11. Search files and folders

Open **Search** in the sidebar to find files and folders across your fixed local drives. Cyron builds and keeps its own local index; Everything does not need to be installed, and search never uses the network.

- **Build the index once**: the first visit offers **Build index**. Cyron walks every fixed drive and stores names, paths, sizes, created/accessed/modified dates and file attributes. It reads no file contents and changes nothing on disk. Up to four directory readers work at once, enough to keep an SSD busy without turning the build into a CPU or disk hog. Progress shows the current drive, folder and entry count; **Cancel** stops the build. On a large drive the first pass can take several minutes.
- **Saved between sessions**: the index is written to `%APPDATA%\dev.cyron.uninstaller\search-index.bin` and loaded in the background on the next launch. While Cyron is running, file-system watchers add, remove, rename and update entries live. Those live changes are held in memory for the session, so **Rebuild index** is what folds in changes made after the last build, including while Cyron was closed; the page warns when the saved index may be stale.
- **Search as you type**: names match by substring and are case-insensitive by default. Answers come from memory; a broad query over 500,000 indexed names measured 7 ms on the test machine. Results show the name, containing folder, size and modified date. Click a column to sort, double-click a row (or press Enter) to reveal it in File Explorer, or copy its full path. **Load more** pages through large result sets.
- **Filters and options**: the chips limit results to files, folders, audio, video, images, documents, executables or compressed files. **Match case**, **Whole word**, **Match path** and **Regex** change how each term is matched.
- **Select and export**: check rows with the row checkbox, ctrl/cmd-click, shift-click for a range, or from the keyboard with Space and Shift+Arrow. Ctrl+A checks every loaded row and Escape clears the selection. The bar above the table shows the checked count and size and can copy their paths or export just those rows as CSV or TXT. The footer's CSV and TXT buttons export every matching row of the current query and sort order, even when no row is checked. Both use the native save dialog.

### Search syntax

| Query | Meaning |
|---|---|
| `report` | Names containing `report`. |
| `annual report` | Both terms must match (space is AND). |
| `report | invoice` | Either term matches (OR). |
| `!backup` | Exclude names containing `backup`. |
| `"annual report"` | Match the phrase as typed. |
| `*.pdf` | Wildcards: `*` for any run of characters, `?` for one. |
| `ext:pdf,png` | Match one of several extensions. |
| `file:` / `folder:` | Only files, or only folders. |
| `size:>1gb` | Size comparison. Ranges use `1mb..10mb`; units are b, kb, mb, gb and tb. |
| `dm:thisweek` | Modified date. Also `today`, `yesterday`, `lastmonth`, `thisyear`, an absolute date like `2026-01-31`, comparisons with `>` or `<`, and ranges with `..`. |
| `dc:` / `da:` | Created and accessed dates, with the same values as `dm:`. |
| `type:video` | Built-in kinds: audio, video, image, document, executable, compressed. |
| `path:src` | Match the full path instead of only the name. |
| `attrib:h` | Files with an attribute such as h (hidden), s (system), r (read-only), a (archive) or d (folder); prefix `-` to require its absence. |
| `dupe:` | Keep only names that appear more than once in the matches. |
| `empty:` | Empty folders and zero-byte files. |
| `regex:^log-\d+` | Treat the rest of that term as a regular expression. |

Examples: `report ext:pdf dm:thisweek`, `size:>1gb`, `folder: node_modules`, `ext:psd,ai,sketch`, `dupe: ext:jpg`.

### What the index covers

- **Fixed local drives only.** Removable, network and optical drives are skipped so indexing never blocks on them.
- **Metadata only.** Cyron never opens a file's contents, so it cannot search inside documents. It indexes paths, not full-text.
- **Reparse points are not followed.** Junctions and symlinks are listed as entries but their targets are not traversed, matching the disk analyzer's rule.
- **Unreadable folders are reported.** Windows-protected or locked folders appear as warnings; the rest of the drive is still indexed. The index also stops at a 3,000,000-entry safety cap and says so.
- **Live updates apply while Cyron runs.** A rebuild reflects the moment of scanning; use **Rebuild index** after large changes made while Cyron was closed.

Search is read-only: it never adds, renames or deletes files or changes the index other than its own regenerable cache. Export writes only the generated rows to the `.csv` or `.txt` path you pick in the native save dialog, and never opens or indexes the destination. The local cache contains file names and paths and can be deleted at any time; it is rebuilt on demand. Revealing a result uses the same validated File Explorer action as the rest of the app.

---

## Reviewed Analyzer actions and Cleaner scope

In **Storage & Cleanup → Disk space**, select a file or folder in the listing or treemap, then use **Add selected item**. Add more items as needed, or exclude paths from the selection. **Move to Recycle Bin…** and **Permanently delete…** open separate reviews showing the exact paths, content counts and estimated logical bytes. Confirm only after reviewing the operation. Permanent deletion bypasses quarantine; Cyron cannot restore it. Recycling does not free space until Windows empties its bin. Files are recycled separately; empty selected folders are removed only when empty. Cancel stops the remaining work between items. Review any failures; the Analyzer refreshes after an attempt.

Cleanup can scan selected mounted local drives for its supported locations. It does not search every file on a drive for things to delete. Path, size, modification time, risk and consequences remain visible in the review; exclusions remain available through Settings. Unsupported generic temporary folders and installer servicing stores are left alone. Applications removed during this session and persistent removal history appear under known removed applications. You can explicitly scan for an app removed before Cyron was installed by supplying its name, publisher, or install path; such matches are never preselected and remain unverified until you review them. Quarantined space is still on disk until an explicit purge.

Uninstaller distinguishes a verified missing inventory entry from an uninstaller that only exited. **Remaining**, **Restart required**, **Cancelled**, and failed items need follow-up. Batch **Stop after current app** lets the vendor wizard finish and skips the rest of the queue. For Store packages, Cyron asks Windows Package Manager to remove the package for the current Windows account only; if Windows policy denies it, use Settings → Apps → Installed apps or winget.
