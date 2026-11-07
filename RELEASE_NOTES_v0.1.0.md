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

#### 3. Hunter Mode & Desktop Targeting
- **External Window Targeting:** Native `WindowFromPoint` resolution targeting external desktop windows with strict directory boundary matching.
- **Isolated Snapshot Management:** Completely isolates Hunter target snapshots from standard Uninstaller process snapshots.
- **Ergonomic HUD & Docking:** Non-intrusive targeting controls styled with authentic design tokens that automatically yield when drawers or modals are active.

#### 4. Review-First Cache & Bloat Cleaner
- **Declarative Extensible Providers:** Structured catalog covering dev caches (Node/npm, Cargo, Gradle, Docker, Python/pip, Go, NuGet) and system temporary data.
- **Crash-Consistent Quarantine:** Safe write-ahead recovery journal (`recovery_journal.jsonl`), atomic `MoveFileExW` (no-overwrite) moves, and manifest-scoped recovery. Files are never deleted in place during cleanup; restoration is one-click.
- **Direct Analyzer → Cleaner Review:** Select recognized cache directories in the treemap or tree and jump directly to review.

#### 5. Windows Explorer Context Menu Integration
- Right-click folders or drives to **Analyze with Cyron**, **Find Duplicates**, or **View Storage Breakdown**.
- Right-click executables to **Uninstall with Cyron** or **Find Related Files**.
- Configured per-user (`HKCU`) without requiring elevation; toggleable and repairable via Settings.

---

### Verified Release Assets (Publisher: `Chryypt`)

| File | Size | SHA-256 Checksum |
|---|---|---|
| `Cyron Uninstaller_0.1.0_x64-setup.exe` | 1.86 MiB | `52ae5559ff5ad1aa7fe4863c7c47142119eba4a7cbd5ef29748fd78f4ba4fe62` |
| `Cyron Uninstaller_0.1.0_x64_en-US.msi` | 2.66 MiB | `a0bfdf78a4bab17c6c765273bccc2bd0597fa32136f8888f5d1a90496e62c8d1` |
| `cyron.exe` | 14.6 MiB | `103ca4e1606de9dc43237fe9a106cdfc9a0e538a7b1c95856774ce22d361a0b9` |
| `SHA256SUMS.txt` | 240 B | *Cryptographic checksum manifest attached below* |

---

### Verification Gates & Test Summary
- **Rust Core & Integration Suite:** 185 tests passed (`cargo test --all-targets`).
- **Frontend Test Suite:** 83 Vitest tests passed across 9 test suites.
- **TypeScript Diagnostics:** 0 errors (`npm run typecheck`).
- **Documentation Integrity:** 54 markdown files audited with 0 broken links (`npm run docs:check`).
- **Windows PE & WiX Metadata Audit:** 5/5 metadata checks passed with publisher `Chryypt` (`npm run verify:metadata`).
- **End-to-End Test Suite:** 8/8 disposable GUI and system targeting scenarios passed (`RC1_E2E_FIXTURE_OK`).
