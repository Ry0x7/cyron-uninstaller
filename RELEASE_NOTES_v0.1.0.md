# Cyron Uninstaller v0.1.0 (Release Candidate 1)

Cyron is a modern, transparent Windows uninstaller, disk space analyzer, and developer cache cleaner built with Tauri v2, Rust, and React. It combines the deep uninstallation of Revo Uninstaller, the visual disk exploration of WinDirStat/WizTree, and the bloat cleanup of BleachBit with safety-first guardrails, clear indications, and crash-consistent recovery.

> [!NOTE]
> **Publisher Metadata:** `Chryypt`  
> Complete Windows PE VERSIONINFO and WiX MSI properties verified under publisher **Chryypt** (Product Name: `Cyron Uninstaller`, Version: `0.1.0`, UpgradeCode: `{C2BCD648-205F-5904-9E5C-85726E376EC2}`).

---

### Features & Capabilities

#### 1. Windows Application Uninstaller
- **Multi-Source Discovery:** Discovers Win32 64-bit and 32-bit applications, Microsoft Store / AppX / MSIX packages (read-only inventory), Steam game installations across all configured libraries, and standalone portable executables.
- **Process Discovery & Snapshot-Verified Termination:** Detects running processes associated with target apps, blocks critical system processes, and guards against PID recycling before termination.
- **Reboot Code Handling:** Accurately recognizes Windows installer exit code `3010` (Reboot Required) alongside standard success/failure codes.
- **Deep Leftover Analysis & Safe Registry Cleanup:** Scans leftover directories, files, and registry keys with explicit confidence and risk levels (`safe`, `caution`, `protected`). Performs automatic, recursive multi-type `.reg` registry backups before subkey deletion.
- **Batch Uninstallation:** Sequential uninstallation of multiple selected applications with real-time logging and review gates.
- **Fail-Closed Broken Entry Removal:** Atomic-style backup, journal, and manifest verification before unregistering broken uninstall keys.

#### 2. Disk Space Analyzer & High-Performance Visualizer
- **Dual Scan Engine:** Direct elevated NTFS `$MFT` reader parsing raw FILE records paired with an automatic streaming depth-first fallback for standard permissions.
- **Memory Arena Allocation Method:** Employs typed contiguous memory arena buffers (`Float64Arena`) and object pools (`ObjectArena`) for $O(1)$ squarified treemap layout partitioning and tree virtualization, eliminating GC pauses and heap churn.
- **Duplicate File Verification:** 3-stage duplicate detection (size grouping, 4 KiB edge hashing, streaming SHA-256) with hard-link deduplication.
- **Storage Intelligence:** Categorizes storage into 10 distinct categories with age distribution and actionable insights.

#### 3. Hunter Mode & Desktop Window Targeting
- **Main Sidebar Placement:** Moved to the primary sidebar navigation directly below Uninstaller as a global session action.
- **Activation Confirmation:** Clicking Hunter Mode displays a compact confirmation dialog explaining window targeting, available actions, and exit controls before tracking begins.
- **Active Session Lifecycle:** Features an animated active sidebar badge with an inline exit control, session-bound targeting HUD/crosshair triggers, and instant `Esc` cancellation without background polling.
- **External Window Targeting:** Native `WindowFromPoint` resolution targeting external desktop windows with strict directory boundary matching.
- **Isolated Snapshot Management:** Completely isolates Hunter target snapshots from standard Uninstaller process snapshots.

#### 4. Review-First Cache & Bloat Cleaner
- **Declarative Extensible Providers:** Structured catalog covering dev caches (Node/npm, Cargo, Gradle, Docker, Python/pip, Go, NuGet) and system temporary data.
- **Crash-Consistent Quarantine:** Safe write-ahead recovery journal (`recovery_journal.jsonl`), atomic `MoveFileExW` (no-overwrite) moves, and manifest-scoped recovery. Files are never deleted in place during cleanup; restoration is one-click.
- **Direct Analyzer → Cleaner Review:** Select recognized cache directories in the treemap or tree and jump directly to review.

#### 5. Windows Explorer Context Menu Integration
- **Standard Shell Verbs:** Right-click folders or drives to **Analyze with Cyron**, **Find Duplicates**, or **View Storage Breakdown**. Right-click executables to **Uninstall with Cyron** or **Find Related Files**.
- **Advanced Destructive Verbs:**
  - **Files & Folders:** **Force Delete with Cyron** and **Find Locking Processes with Cyron**.
  - **Executables (`.exe`):** **Kill Running Process with Cyron**.
- **Review Dialogs & Expiring Plans:** Context menu actions launch directly into focused review modals backed by backend-held single-use plans (`StoredForceDeletePlan`, `StoredKillProcessPlan`).
- **Windows Restart Manager:** Detects active file/folder locking processes via `rstrtmgr.dll` with graceful closure (`WM_CLOSE`) and verified force termination.
- **Recoverable Quarantine Default:** Force Delete routes deleted items to a 7-day recoverable quarantine batch by default; permanent removal requires explicit multi-step confirmation.
- **System Root Refusal:** Protects Windows, boot, system directories, program files, user roots, and system-critical processes.
- **Settings Controls:** Independent toggles for Standard and Advanced/Destructive Explorer verbs with HKCU status badges and 1-click repair.

---

### Verified Release Assets (Publisher: `Chryypt`)

| File | Size | SHA-256 Checksum |
|---|---|---|
| `Cyron Uninstaller_0.1.0_x64-setup.exe` | 1.91 MiB | `eeabd584d0533f680c9c7264d41cdfd58b6d6f4d1c8a5b2d0e7654d86cd76e25` |
| `Cyron Uninstaller_0.1.0_x64_en-US.msi` | 2.81 MiB | `87256578b45672665336160be2ee20348bf526ddefc01ac06774807613c9639c` |
| `cyron.exe` | 5.64 MiB | `03ff61d7d999e8f9d1eba86cad08a600f355c9d3bc985be775e4bdd2df088b4c` |
| `SHA256SUMS.txt` | 240 B | *Cryptographic checksum manifest attached below* |

---

### Verification Gates & Test Summary
- **Rust Core & Integration Suite:** 186 tests passed (`cargo test --workspace`).
- **Frontend Test Suite:** 83 Vitest tests passed across 9 test suites.
- **TypeScript Diagnostics:** 0 errors (`npm run typecheck`).
- **Documentation Integrity:** 54 markdown files audited with 0 broken links (`npm run docs:check`).
- **Windows PE & WiX Metadata Audit:** 5/5 metadata checks passed with publisher `Chryypt` (`npm run verify:metadata`).
- **End-to-End Test Suite:** 8/8 disposable GUI and system targeting scenarios passed (`RC1_E2E_FIXTURE_OK`).
