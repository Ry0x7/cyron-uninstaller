# Cyron — Product Roadmap

Cyron is a Windows desktop utility unifying application uninstallation, disk-space analysis, duplicate detection, and review-first cache cleaning with transparent risk communication, built for everyday users and developers alike.

Legend: `[x]` Implemented and verified by automated test suites; `[ ]` Planned future work.

For granular historical implementation narratives of early phases, see [docs/archive/PHASE_HISTORY.md](docs/archive/PHASE_HISTORY.md).
For the current feature matrix and verified limitations, see [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md).

---

## Completed Milestones

### Phase 1 — Foundation & Desktop Shell (`[x]`)
- Tauri v2 + React 19 + TypeScript desktop shell with dark/light design system.
- Basic Win32 installed application discovery (HKLM/HKCU, 64/32-bit views).
- Native Win32 directory enumeration and cancellable filesystem scanning.
- Interactive squarified treemap and synchronized folder tree.
- Initial 48-location developer, AI, and system cache catalog with risk ratings.
- Local scan history persisted in `%APPDATA%\dev.cyron.uninstaller\history.json`.

### Phase 2 — Disk Intelligence & Hybrid Engine (`[x]`)
- High-speed NTFS Master File Table (MFT) reader for elevated whole-volume scans.
- Streaming depth-first directory walker with bounded memory and hard-link tracking.
- Automatic scanner method selection (MFT on drive roots when elevated; walk elsewhere).
- Virtualized resizable folder tree column (220–520 px) with ARIA tree keyboard navigation.
- Multi-field filtering (size, age, name, category, extension) and filtered totals.
- 3-stage duplicate file verification (size buckets -> edge hash -> full SHA-256).
- 10-tier storage categorization with age distribution and explainable insights.

### Phase 3 — Review-First Cache Cleaner (`[x]`)
- Review-first cache cleaning engine over fixed catalog specifications.
- Server-held, single-use, 10-minute expiring confirmation plans (`CleanupPlan`).
- Recoverable quarantine (`%APPDATA%\dev.cyron.uninstaller\quarantine\<run-id>\`).
- Durable write-ahead recovery journal (`recovery_journal.jsonl` with `sync_all`).
- Atomic no-overwrite file restoration via Win32 `MoveFileExW`.
- Backend safety guard refusing protected paths and their ancestors.
- Audit logging in `cleanup-log.json` (max 100 entries).

### Phase 4 & 4.1 — Uninstaller Engine & Priority 0 Hardening (`[x]`)
- Standard uninstaller execution with reboot exit code 3010 tracking.
- Shell injection rejection (refusing `cmd.exe`, `powershell.exe`, command chaining).
- Process discovery and snapshot-verified termination with critical Windows process deny list.
- Re-validation of PID creation times (`GetProcessTimes`) to block PID recycling races.
- Residual leftover scanning across AppData, ProgramData, and Windows Registry.
- Automated recursive multi-type `.reg` registry backups before subkey deletion.
- Sequential batch uninstallation queue with failure isolation.
- 20 GB quarantine storage cap (`MAX_QUARANTINE_TOTAL_BYTES`) and 512 MB low-space safety guard.

### Phase 5 — Packaging & Verification Automation (`[x]`)
- Multi-source application discovery (Win32, Store packages, Steam games, portable heuristics).
- Automated release workflow in GitHub Actions (`release.yml`).
- NSIS setup installer and WiX MSI packaging (`RELEASE_BUNDLES_OK`).
- Application data preservation across in-place upgrades.

### Phase 6 — Competitive Feature Expansion (`[x]`)
- Windows Explorer context menu integration (`HKCU\Software\Classes`) with safe routing.
- Disk Analyzer -> Cleaner direct review integration (`inspect_cleanable_directory`).
- Extensible declarative cleanup providers (`schema_version: 1`) with schema validation.
- Competitive performance benchmarks and file counting documentation.

### Phase 7 — Code Signing Architecture & Release Provenance (`[x]`)
- Windows PE VERSIONINFO, WiX MSI properties, and NSIS header audit (`WINDOWS_METADATA_OK`).
- Chryypt Organization code signing pipeline (`CHRYYPT_SIGNED`, `SIGNATURES_VERIFIED_OK`).
- Automated negative signature test suite (detecting unsigned, tampered, or rogue certificates).
- Public code signing policy authored (`docs/CODE_SIGNING_POLICY.md`).
- Git history privacy audit: verified clean commit tree with zero `.chryypt` files.
- Release Candidate 1 (`v0.1.0`) tagged and published on GitHub.
- Automated documentation integrity gate (`npm run docs:check`) enforced in CI: links, commands, versions, and evidence references.

### Phase 7.4 — Hunter Navigation & Explorer Destructive Actions (`[x]`)
- Moved Hunter Mode into main sidebar navigation below Uninstaller as a global session action.
- Added compact Hunter activation confirmation dialog explaining window targeting, actions, and exit controls.
- Active sidebar indicator with direct exit control and instant `Esc` cancellation without stale background polling.
- Rendered HUD banner and floating crosshair triggers strictly during active Hunter sessions.
- Added Explorer right-click shell verbs: *Force Delete with Cyron*, *Find Locking Processes with Cyron*, and *Kill Running Process with Cyron*.
- Built review-first execution plans (`StoredForceDeletePlan`, `StoredKillProcessPlan`) preventing direct deletion or termination from shell arguments.
- Integrated Windows Restart Manager (`rstrtmgr.dll`) for active file/folder locking process detection.
- Enforced system root, boot, and Windows component protections (`is_protected_delete_target`).
- Safe process termination reusing core engine (`process::terminate_single_process`) with graceful closure and verified force termination.

### Performance Diagnostics & Startup Evidence (`[x]`)
- Live CPU, memory, PDH physical-disk, process and storage sampling with bounded overhead.
- Startup application and boot/logon scheduled-task inventory with provenance and live usage.
- Per-app boot impact only when Windows' administrator-only event log records it; otherwise **Unavailable**.
- Reversible startup toggles through Windows `StartupApproved` behind a native confirmation dialog.
- Evidence-bound recommendations with measured effect, risk and a verification method.
- Five symptom guides for "Why is my PC slow?", local before/after baselines, and an optional local WPR trace.
- Explicit refusal of RAM boosters, registry cleaning, service disabling, Prefetch deletion, page-file changes, Defender/update disabling and driver tweaks.

---

## Upcoming Milestones

### Phase 8 — Trusted Signing & Scale (`[ ]`)
- [x] **Native WinRT Store Package Removal**: Current-user packages are removed through `PackageManager::RemovePackageAsync` after a server-held plan (live removal remains UNVERIFIED; see [IMPLEMENTATION_STATUS](docs/IMPLEMENTATION_STATUS.md)).
- [ ] **Multi-Drive Concurrent Scanning**: Parallelized drive root scanning for workstations with multiple physical NVMe/SSD storage volumes.
- [ ] **Publicly Trusted Release Signing**: Replace the local self-signed development certificate with a publicly trusted Authenticode certificate, or label every release explicitly as unsigned.
