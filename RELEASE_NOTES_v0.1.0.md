# Cyron Uninstaller v0.1.0 (Release Candidate 1)

Cyron is a modern, transparent Windows uninstaller, disk space analyzer, and developer cache cleaner built with Tauri v2, Rust, and React. It combines the deep uninstallation of Revo Uninstaller, the visual disk exploration of WinDirStat/WizTree, and the bloat cleanup of BleachBit with safety-first guardrails, clear indications, and crash-consistent recovery.

> [!NOTE]
> **Build Status:** `CHRYYPT_SIGNED` / `Release Candidate 1`
> These binaries are Authenticode signed with a SHA-256 RSA-3072 certificate under the authority of the **Chryypt Organization** (`CN=Chryypt Organization`, publisher `Chryypt`).

---

### Features & Capabilities

#### 1. Windows Application Uninstaller
- **Multi-Source Discovery:** Discovers Win32 64-bit and 32-bit applications, Microsoft Store / AppX / MSIX packages (read-only inventory), Steam game installations across all configured libraries, and standalone portable executables.
- **Process Discovery & Snapshot-Verified Termination:** Detects running processes associated with target apps, blocks critical system processes, and guards against PID recycling before termination.
- **Reboot Code Handling:** Accurately recognizes Windows installer exit code `3010` (Reboot Required) alongside standard success/failure codes.
- **Deep Leftover Analysis & Safe Registry Cleanup:** Scans leftover directories, files, and registry keys with explicit confidence and risk levels (`safe`, `caution`, `protected`). Performs automatic, recursive multi-type `.reg` registry backups before subkey deletion.
- **Batch Uninstallation:** Sequential uninstallation of multiple selected applications with real-time logging and review gates.

#### 2. Disk Space Analyzer
- **Interactive Treemap & Virtualized Folder Tree:** Fast visual disk usage breakdown paired with a persistent, resizable ARIA-compliant folder tree.
- **Dual Scan Engine:** High-performance direct NTFS `$MFT` parsing for elevated volume root scans with automatic streaming depth-first fallback for non-elevated or subfolder paths.
- **Duplicate File Verification:** 3-stage duplicate detection (size grouping, 4 KiB edge hashing, streaming SHA-256) with hard-link deduplication.
- **Storage Intelligence:** Categorizes storage into 10 distinct categories with age distribution and actionable insights.

#### 3. Review-First Cache & Bloat Cleaner
- **Declarative Extensible Providers:** Structured catalog covering dev caches (Node/npm, Cargo, Gradle, Docker, Python/pip, Go, NuGet) and system temporary data.
- **Crash-Consistent Quarantine:** Safe write-ahead recovery journal (`recovery_journal.jsonl`), atomic `MoveFileExW` (no-overwrite) moves, and manifest-scoped recovery. Files are never deleted in place during cleanup; restoration is one-click.
- **Direct Analyzer → Cleaner Review:** Select recognized cache directories in the treemap or tree and jump directly to review.

#### 4. Windows Explorer Context Menu Integration
- Right-click folders or drives to **Analyze with Cyron**, **Find Duplicates**, or **View Storage Breakdown**.
- Right-click executables to **Uninstall with Cyron** or **Find Related Files**.
- Configured per-user (`HKCU`) without requiring elevation; toggleable and repairable via Settings.

---

### Assets

| File | Description |
|---|---|
| `Cyron Uninstaller_0.1.0_x64-setup.exe` | Standard NSIS Windows Installer |
| `Cyron Uninstaller_0.1.0_x64_en-US.msi` | Windows Installer (WiX MSI) Package |
| `cyron.exe` | Standalone Portable Executable |
| `SHA256SUMS.txt` | Cryptographic SHA-256 verification checksums |

---

### Verification & Testing
- **Rust Test Suite:** 171 tests passed (unit, integration, security, real_system, cleanup_stress, stress_scan).
- **Frontend Test Suite:** 71 Vitest tests passed.
- **Security Audit:** 0 open Dependabot/npm vulnerabilities; strict No-Rebase policy enforced; secret scanning active.
