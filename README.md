# Cyron

Cyron is a Windows desktop utility that unifies application uninstallation, disk-space analysis,
duplicate detection, and developer/AI cache cleaning. It provides a modern, review-first alternative to
Revo Uninstaller, WinDirStat/WizTree, BleachBit, and Bulk Crap Uninstaller (BCUninstaller) with plain-language
risk labels, built for everyday users and developers alike.

> **Release status:** [`v0.2.0-rc.2`](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.0-rc.2) is published as an **unsigned prerelease** for Windows 10/11 x64. It is a testing candidate, not a trusted production release; see the [release notes](docs/RELEASE_NOTES_v0.2.0-rc.2.md) and [Phase 8 audit](docs/RELEASE_AUDIT_2026-11-17.md).

> **Review-first safety & durable recovery.** Cyron measures files and caches, displays every single file and
> registry subkey it plans to touch, and executes modifications only after an explicit confirmation.
> Protected data — conversation histories, AI model weights, configurations, credentials, Docker/WSL virtual disks,
> and browser profiles — is enforced and refused by the Rust backend. Cleaning is off by default: Cyron starts
> in read-only mode until you deliberately enable it. Uninstallation verifies commands server-side, validates
> process snapshot creation times to prevent PID recycling races, unconditionally protects Windows critical processes,
> backs up registry subkeys into multi-type `.reg` files before deletion, and moves residuals to quarantine.

The desktop interface has five primary destinations: **Home**, **Applications**, **Storage & Cleanup**,
**Performance**, and **History & Recovery**. Settings lives in the sidebar footer. Hunter Mode is an
action inside Applications rather than a separate page. The shared visual system is documented in
[docs/DESIGN.md](docs/DESIGN.md).

---

## What Works in Cyron

### 1. Application Inventory & Uninstallation
- **Multi-Source Discovery**:
  - **Win32 Applications**: Reads 64-bit and 32-bit views across `HKEY_LOCAL_MACHINE` and `HKEY_CURRENT_USER`, filtering system components and updates. Identifies missing/broken uninstallers when executables no longer exist on disk.
  - **Microsoft Store / MSIX / AppX**: Current-user package inventory through the Windows Runtime package manager, with native reviewed removal (`RemovePackageAsync`) and no shell, `RemoveForAllUsers` or machine-wide deprovisioning.
  - **Steam Game Libraries**: Auto-detects Steam libraries across drives (`libraryfolders.vdf`), parsing `appmanifest_*.acf` to report game names, exact disk sizes, and launch URLs (`steam://uninstall/<appid>`).
  - **Portable Applications**: Bounded heuristic scanning in user app directories (`%LOCALAPPDATA%\Programs`) identifying standalone PE binaries without uninstaller registration.
- **Standard & MSI Uninstallation**: Validates vendor uninstall commands and Windows Installer GUIDs, rejecting dangerous shell interpreters (`cmd.exe`, `powershell.exe`). Reconciles exit codes against live inventory: 1605/1614 require confirmed absence, 1602 is cancellation, 3010/1641 require restart, and exit 0 alone is never reported as removed.
- **Process Manager**: Discovers running processes associated with the application's installation folder, verifies process creation times against snapshots to prevent PID recycling races, and unconditionally protects Windows critical system processes.
- **Leftover Scanner**: Detects residual files, directories, and registry keys under AppData, ProgramData, and Windows Registry. Automatically creates a complete recursive `.reg` backup preserving all 6 Windows value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) before subkey deletion, and moves residual files to quarantine.
- **Sequential Batch Uninstallation**: Review-first batch queue executing under server-held, 10-minute expiring single-use confirmation plans (`BatchUninstallPlan`) with failure isolation.

### 2. Disk Space Analyzer
- **Squarified Treemap**: High-performance canvas-based hierarchical visualizer with zoom, breadcrumbs, and color-coded categories.
- **Persistent Synchronized Tree**: Resizable virtualized folder tree (220–520 px) with ARIA keyboard navigation, synchronized selection, and drill-down.
- **Storage Intelligence**: Classifies disk occupancy into 10 categories (Applications, Games, Downloads, Documents, Media, System, Developer Tools, AI Models, Caches, Other) with known-vs-inferred confidence.
- **Duplicate Scanner**: Size grouping, 4 KiB edge hashing, and streaming SHA-256 verification. Hard links are identified and excluded from recoverable space.
- **Dual Engine**: High-speed NTFS Master File Table (MFT) parser for whole drives (10× faster than walk) with automatic fallback to streaming directory walking on subfolders or non-elevated shells.
- **Direct Cleaner Review**: Selecting a recognized cache folder in the treemap or folder tree surfaces a **Review for Cleanup** action, displaying expected reclaimable bytes and launching the review-first workflow.

### 3. Review-First Cache Cleaner & Extensible Providers
- **Declarative Provider System**: Versioned (`schema_version: 1`) provider definitions with strict path validation, risk categorization (**Safe**, **Caution**, **Protected**), and application detection.
- **Developer & AI Cache Catalog**: Covers Claude Code, Codex, Gemini, Cursor, VS Code, Antigravity, Freebuff, Ollama, LM Studio, Hugging Face, npm, pnpm, Yarn, pip, Poetry, uv, Cargo, Gradle, Docker, WSL, Windows temp files, and browsers. A conservative install check distinguishes detected tools from orphaned caches without ever auto-selecting them.
- **Developer Project Storage**: Scans selected local drives and user-added folders for real projects by manifest and Git root, then measures only named regenerable artifacts (`node_modules`, `dist`, `build`, `target`, `.next/cache`, `.turbo`, `__pycache__`, and equivalents). Each artifact shows exact file ownership evidence, confidence, last write time, rebuild instructions and what is lost. Source, `.git`, `.env`, credentials, databases and model weights are protected and never offered. Cleanup uses the same preview, quarantine, restore and verification engine.
- **Enforced Backend Protections**: Protected data (chat logs, models, credentials, settings, Windows Update downloads) is unconditionally rejected from cleaning in the Rust backend.
- **Recoverable Quarantine**: Cleaned files are moved into `<app_data>\quarantine\<run-id>\` alongside an append-only, durable write-ahead recovery journal (`recovery_journal.jsonl` with physical `sync_all`). Restores use atomic `MoveFileExW` no-overwrite guarantees.

### 4. Windows Explorer Context Menu Integration
- **Per-User HKCU Integration**: Optional right-click shell verbs registered under `HKEY_CURRENT_USER\Software\Classes` without requiring administrator privileges:
  - **Standard Verbs**: *Analyze with Cyron*, *Find Duplicates with Cyron*, *View Storage Breakdown with Cyron*, *Uninstall with Cyron*, *Find Related Files with Cyron*.
  - **Advanced & Destructive Verbs**:
    - **Files & Folders**: *Force Delete with Cyron*, *Find Locking Processes with Cyron*.
    - **Executables (`.exe`)**: *Kill Running Process with Cyron*.
- **Review-First Deletion & Termination**: Shell actions never execute silently or delete directly from command-line arguments. Cyron launches directly into focused review modals backed by backend-held, single-use execution plans (`StoredForceDeletePlan`, `StoredKillProcessPlan`).
- **Windows Restart Manager**: Detects active file/folder locking processes via `rstrtmgr.dll`, providing graceful process closure (`WM_CLOSE`) and verified force termination with PID creation-time checks.
- **Recoverable Quarantine Default**: Force Delete routes objects through a 7-day recoverable quarantine batch with write-ahead recovery journals (`recovery_journal.jsonl`); permanent removal requires explicit, separate confirmation.
- **System & Root Refusal**: Refuses Windows, boot, System32, Program Files, user roots, and critical system components.
- **Settings Controls**: Independent toggles for Standard and Advanced/Destructive Explorer integration with 1-click status checking and repair.

### 5. Hunter Mode & Desktop Window Targeting
- **Applications Action**: **Find by window** sits in the Applications header next to Refresh; Hunter Mode is not a separate navigation page.
- **Activation Confirmation**: Clicking **Find by window** opens a compact confirmation dialog explaining external window targeting, available actions, and exit controls before any cursor tracking begins.
- **Session Lifecycle**: Cursor tracking and targeting resolution only run during an active confirmed session; the HUD banner and floating crosshair are hidden when inactive.
- **Active Indicator & Exit**: An active targeting indicator with an inline exit control is shown only during the session; `Esc` cancels immediately without leaving stale targets or background polling.

### 6. Performance Diagnostics & Startup Evidence
- **Live Local Metrics**: Samples total CPU, physical memory and commit charge, Windows PDH physical-disk busy/queue counters, fixed-volume capacity, and per-process CPU, working set and I/O with a 700 ms measured window and a bounded 4,096-process cap.
- **Short-Spike Capture**: A 20-second local burst records the peak CPU, kernel and disk values between the normal ten-second samples. It reports whether the peak was process-, kernel- or disk-dominated without inventing a driver attribution.
- **Startup and Task Provenance**: Lists HKCU/HKLM `Run`, `RunOnce`, WOW6432Node and Startup-folder registrations plus boot/logon scheduled tasks. Per-app boot impact is shown only when the administrator-only Diagnostics-Performance event log actually records it; otherwise the page says **Unavailable** instead of inventing a score.
- **Reversible Startup Changes**: Enabling or disabling an entry uses the existing Windows `StartupApproved` bytes, preserves the original registration, and requires a native confirmation showing the exact entry and location. Machine-wide entries stay read-only without elevation.
- **Kernel & Driver Evidence**: When kernel time dominates, the page shows the kernel share, the measured application CPU it does not explain, installed display/storage/network driver versions and package dates, and matching driver/device errors from the System event log. It never claims that Windows attributed DPC/ISR time to a named driver; that requires the optional trace.
- **Evidence-Bound Recommendations**: Recommendations appear only for measured pressure (CPU, memory, physical disk, capacity, or recorded boot cost) and each carries its evidence, expected effect, risk and verification method. A high per-process I/O total alone is not treated as disk pressure because Windows includes network and device transfers.
- **Guided Investigation & Baselines**: *Why is my PC slow?* filters the evidence across six symptoms, including CPU spikes and kernel time. Users can record local before/after measurements; the page reports association, not causation, and never claims a guaranteed speed gain. An optional WPR trace is offered only when basic metrics cannot explain the lag, requires the user to relaunch as administrator, and is stored locally.
- **No Placebo Tweaks**: No RAM boosters, standby-memory clearing, registry cleaning, service disabling, Prefetch deletion, page-file changes, Defender/update disabling or driver tweaks.

---

## Architecture & Safety Guarantees

```mermaid
flowchart TD
    subgraph UI ["Frontend (React 19 + TypeScript)"]
        Nav[Sidebar Navigation]
        Views[Home / Applications / Storage & Cleanup / Performance / History & Recovery]
    end

    subgraph IPC ["Tauri v2 IPC Boundary (CamelCase JSON)"]
        Cmds[72 Validated Commands]
    end

    subgraph Backend ["Rust Core Engine"]
        AppMgr[App Discovery: Win32, Store, Steam, Portable]
        Uninst[Uninstaller Runner & Batch Confirmation Plans]
        ProcMgr[Process Snapshot & Creation Time Verification]
        LeftoverMgr[Residual Scanner & Multi-Type .REG Backup]
        Scanner[Hybrid Scanner: MFT Reader + Streaming Walk]
        ProvMgr[Declarative Provider Schema Validator]
        CleanupEng[Cleanup Engine: Guard & Read-Only Check]
        Quarantine[Crash-Consistent Journal & Atomic MoveFileExW]
        ExplorerMgr[HKCU Shell Context Menu Verbs]
        PerfMgr[Performance Diagnostics: Win32 + PDH + Event Log]
    end

    Views <--> Cmds
    Cmds <--> AppMgr
    Cmds <--> Uninst
    Cmds <--> ProcMgr
    Cmds <--> LeftoverMgr
    Cmds <--> Scanner
    Cmds <--> ProvMgr
    Cmds <--> CleanupEng
    Cmds <--> ExplorerMgr
    Cmds <--> PerfMgr
    CleanupEng --> Quarantine
    LeftoverMgr --> Quarantine
```

- **Server-Held Confirmation Plans**: Every destructive action (cleaning, uninstallation, process termination, leftover removal, batch uninstallation, Analyzer deletion and Explorer Force Delete) requires an expiring, single-use plan generated server-side.
- **Fail-Closed Execution**: If process creation time cannot be verified, or if write-ahead journal synchronization fails, operations abort immediately.
- **Quarantine by Default**: Cleaner, leftover and Explorer Force Delete targets move into journaled quarantine batches. Permanent removal is a separate, explicitly confirmed Analyzer or Explorer action and is plan-scoped.
- **No Network Telemetry**: Cyron does not contain tracking, analytics, or network-bound phone-home code.

---

## Getting Started

### Prerequisites
- Windows 10 (1809+) or Windows 11 (64-bit).
- **Node.js 22+** and npm.
- **Rust stable (1.77+)** with the `x86_64-pc-windows-msvc` target.
- **Visual Studio 2022 Build Tools** with *Desktop development with C++* workload.
- On standard Developer shells or Git Bash, `scripts/msvc-env.mjs` automatically configures MSVC environment variables (`PATH`, `LIB`, `INCLUDE`).

### Running Locally

```powershell
npm install
npm run dev:desktop        # Vite + Tauri desktop window
```

Build a standalone desktop executable:

```powershell
npm run build:desktop      # src-tauri/target/debug/cyron.exe
```

---

## Verification & Testing

Every build is verified with automated test suites, stress harnesses, and end-to-end fixtures:

```powershell
npm run typecheck                                      # TypeScript type validation (0 errors)
npm test                                               # 100 Vitest frontend tests
npm run test:desktop                                   # Real Tauri/WebView2 desktop E2E: DESKTOP_E2E_OK
npm run build                                          # Production frontend bundle
node scripts/verify-dist.mjs                           # Frontend bundle assertions (DIST_OK)
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test real_system -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test stress_scan -- --nocapture
node scripts/verify-cleanup.mjs                        # Isolated cleanup fixture: CLEANUP_FIXTURE_OK
node scripts/verify-e2e.mjs                            # Complete RC1 E2E fixture: RC1_E2E_FIXTURE_OK
node scripts/verify-desktop-binary.mjs                 # Desktop binary validation: DESKTOP_BINARY_OK
node scripts/smoke-desktop.mjs                         # Desktop window launch test: DESKTOP_SMOKE_OK
node scripts/benchmark-competitive.mjs                 # Cyron-only scan benchmark, no competitor figures: BENCHMARK_OK
node scripts/verify-release-bundles.mjs                # Release bundles check: RELEASE_BUNDLES_OK
npm run verify:metadata                                # Windows PE & MSI metadata audit: WINDOWS_METADATA_OK
npm run verify:signatures                              # Signed builds: SIGNATURES_VERIFIED_OK. The unsigned RC exits 1 by design.
```

The release bundles were also installed and exercised: the per-user NSIS package installs, repairs,
relaunches, uninstalls and reinstalls with app data preserved. The MSI is per-machine and requires
elevation. Hashes, timings and limitations are in
[docs/evidence/2026-11-17-installer-release.md](docs/evidence/2026-11-17-installer-release.md).

---

## Current Status & Limitations

- **Store / MSIX Uninstallation**: Current-user Microsoft Store packages can be removed through a server-held plan and the native Windows Runtime package manager. Cyron never calls `RemoveForAllUsers`, deprovisions machine-wide, elevates automatically or launches `winget`/PowerShell. Machine-wide or provisioned packages require Windows Settings or an administrator-managed tool. Live Store removal through the desktop UI is UNVERIFIED as of 2026-11-16.
- **Performance Trace & Elevated Boot Impact**: The Performance page is read-only unless you toggle a startup entry. Recording a Windows Performance Recorder trace and reading per-app boot impact require an administrator session; the unelevated UI explains and disables both. Those elevated paths are UNVERIFIED. Cyron does not measure temperature, fan speed or guarantee any speed improvement.
- **Portable Applications**: Discovered via explainable heuristics; standard uninstaller commands are disabled because portable apps lack registry uninstallers. Leftover and disk analysis are available.
- **Code Signing Architecture**: Local Authenticode signing under the Chryypt Organization:
  - `CHRYYPT_SIGNED`: builds signed with the **self-signed** `CN=Chryypt Organization` certificate (RSA 3072, SHA-256) via `npm run build:signed` (`scripts/sign-build.ps1`). Integrity is verifiable; public trust is not. The certificate chain reports `UntrustedRoot`.
  - Unsigned builds remain clearly labeled `UNSIGNED_RC` for private testing. The published v0.1.0 NSIS asset and all current local release artifacts are `NotSigned`. A self-signed certificate does not earn SmartScreen reputation; expect an unknown-publisher warning.
  - The 2026-11-17 audit blocks a public release on this and the other open items in [docs/RELEASE_AUDIT_2026-11-17.md](docs/RELEASE_AUDIT_2026-11-17.md).
- **Installer Scope**: The NSIS package is a verified per-user install. The WiX MSI is per-machine, requires administrator elevation, and its elevated install/uninstall path is UNVERIFIED. A version-to-version upgrade and clean-VM install were not executed; the NSIS same-version reinstall path passed with app data preserved.

---

## Documentation Map

- [ROADMAP.md](ROADMAP.md): Project phases, completed milestones, and future work.
- [HANDOFF.md](HANDOFF.md): Engineering state, verified commit history, and immediate priorities.
- [GATES.md](GATES.md): Verifiable acceptance criteria and test evidence.
- [AGENTS.md](AGENTS.md): Architectural contracts, IPC rules, and developer guidelines.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): Component interactions, IPC contract, and data flows.
- [docs/DESIGN.md](docs/DESIGN.md): Design tokens, navigation, primitives, states and accessibility rules.
- [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md): Detailed verification ledger and test counts.
- [docs/SECURITY.md](docs/SECURITY.md): Threat model, security boundaries, and confirmation plan safety.
- [docs/USER_GUIDE.md](docs/USER_GUIDE.md): End-user walkthrough of Explorer integration, scanning, cleaning, and uninstallation.
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md): Cyron-only measured scan time and working set, with explicit methodology and limitations.
- [docs/COMPETITOR_COMPARISON.md](docs/COMPETITOR_COMPARISON.md): Documentation-based capability comparison; competitor scenarios are marked NOT RUN.
- [docs/RELEASE_AUDIT_2026-11-17.md](docs/RELEASE_AUDIT_2026-11-17.md): Independent PASS/FAIL/UNVERIFIED matrix, adversarial review, blockers and evidence.
- [docs/evidence/2026-11-17-installer-release.md](docs/evidence/2026-11-17-installer-release.md): 0.1.0 release artifact hashes, NSIS install/repair/uninstall/reinstall results, MSI elevation result and signature trust.
- [docs/evidence/2026-11-18-release-0.2.0.md](docs/evidence/2026-11-18-release-0.2.0.md): 0.2.0 RC1 artifact hashes, metadata, signature result and NSIS install/launch/uninstall/reinstall receipt.
- [docs/RELEASE_NOTES_v0.2.0-rc.2.md](docs/RELEASE_NOTES_v0.2.0-rc.2.md): Unsigned 0.2.0 RC2 notes, including batch eligibility and kernel/driver diagnostics.
- [docs/RELEASE_NOTES_v0.2.0-rc.1.md](docs/RELEASE_NOTES_v0.2.0-rc.1.md): Unsigned 0.2.0 release candidate 1 notes.
- [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md): Release packaging and VM verification steps.
- [docs/BUILD_AND_SIGNING.md](docs/BUILD_AND_SIGNING.md): Build instructions, installer targets, WebView2 requirements, and code signing architecture.
- [CONTRIBUTING.md](CONTRIBUTING.md): Code standards, safety invariants, and contribution workflow.

---

## Contributors

- **Ry0x7** — Project Creator, Systems Architecture & Core Engineering
- **Gemini** — Contributor (Advanced Agentic Systems & Implementation)
- **Claude Code** — Contributor (Systems Hardening & Feature Development)
- **Codex** — Contributor (Code Intelligence & Implementation)

---

## License & Git Workflow Policy

- **Source-Available / No-Reuse License**: Cyron Uninstaller is source-available for viewing, security auditing, and personal compilation under the [Cyron Source-Available License](LICENSE). **Code reuse, redistribution, or extraction into any other project or commercial product is strictly prohibited** without prior written permission from the author (Ry0x7).
- **Strict No-Rebase Policy**: Rebasing (`git rebase`) is permanently prohibited across all shared branches and pull requests to preserve immutable cryptographic release provenance, Authenticode signatures, and audit trails. GitHub enforces `allow_rebase_merge: false` and branch protection disallowing force-pushes (`allow_force_pushes: false`). Local clones enforce this via the repository's `pre-rebase` hook (`scripts/setup-git-hooks.ps1`). Integration must always use merge commits (`git merge --no-ff`) or squash merges.
