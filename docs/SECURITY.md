# Security model

## Scope and assets

Cyron (Phases 1–4) is a local Windows desktop scanner, cache cleaner and application uninstaller.
It reads installed-application metadata, enumerates filesystem metadata with either an NTFS MFT reader
or a directory walk, verifies duplicate file content by hashing, classifies storage, measures a fixed
catalog of 48 cache locations, moves caches and leftovers into a per-user recoverable quarantine with
atomic restore, safely executes standard application uninstallers, and controls associated application
processes with strict critical-process safeguards.

Protected assets: repositories and source code, credentials, SSH keys and tokens, application
settings, AI conversation histories and memories, AI model weights, Docker images/volumes/containers,
WSL distributions, browser profiles/history/cookies/passwords, the registry, Windows servicing state,
critical Windows system processes, and the integrity of the scan, quarantine, and recovery journals.

Child processes are strictly controlled:
1. `explorer.exe` is launched solely to reveal a validated existing path on explicit user action.
2. Registered application uninstallers are launched only after tokenization, rejection of shell wrappers
   (`cmd.exe`, `powershell.exe`, batch scripts), and explicit operator confirmation.
3. Processes can only be terminated if their executable resides strictly within the target application's
   install directory, and critical system processes are unconditionally protected.
4. Registry key deletion for leftovers is strictly guarded and preceded by an automated `.reg` backup.

## Trust boundaries

- **Frontend to Rust IPC (now partially destructive):** the webview is still treated as untrusted.
  Cleanup commands accept **catalog target ids**, never paths: `preview_cleanup` resolves ids through
  `caches::CATALOG`, refuses unknown and `protected` ids, and returns only a preview; the executable
  plan stays in backend state keyed by an opaque plan id. `start_cleanup` consumes that plan exactly
  once, rejects a selection the plan does not contain, enforces the 10-minute TTL and the backend
  read-only switch, and then re-runs the path guard for every target. Exclusions coming from Settings
  are validated (absolute, non-root, at most 128) before use and stored in the plan so preview and
  execution skip the same folders.
- **Windows filesystem and registry:** filesystem metadata and registry values are external input
  and may be missing, malformed, inaccessible or concurrently changed. Enumeration reports bounded
  errors rather than assuming access or completeness. Application discovery reads uninstall registry
  keys only.
- **Reparse points and links:** directory entries marked as reparse points are skipped and never
  followed. Before a file is moved, its `symlink_metadata` must report a regular file without a
  reparse attribute, and the canonicalized parent must still be inside the target root — so a
  junction swapped in after the preview cannot redirect the move. Quarantined files are stored in a
  flat per-run folder with an index prefix, so a crafted file name cannot escape it.
- **Quarantine and log storage:** quarantine batches and `cleanup-log.json` live in the per-user
  Tauri app-data directory. Restoration and purge take a `run-id` validated against
  `[A-Za-z0-9_-]`, and purge deletes only files recorded in that batch's manifest whose canonical
  path is still inside the batch folder.
- **Tauri webview/plugins:** the dialog plugin can request a folder selection. It does not grant
  delete or registry-write operations. Keep capabilities minimal and review new plugins before
  adding them.

## Scanner guarantees and limits

- The filesystem scanner is read-only: it enumerates directories and metadata and does not modify
  scan targets.
- Junctions, symbolic links, mount points and other reparse points are not traversed.
- Hard links are counted once for allocated bytes while logical bytes reflect file names. Tracking is
  capped at one million file IDs; after that the scanner reports that later links may count twice.
- Per-directory file nodes, retained nodes, extension keys and summaries, largest-file results,
  recursion depth, error messages and IPC tree size are bounded. The walker streams entries rather
  than collecting per-directory vectors. The MFT reader rejects a volume above its derived 768 MiB
  record budget before allocating and parses through one fixed 1,024-byte buffer.
- Duplicate hashing is read-only and reports files that change while being hashed instead of
  trusting them.
- The MFT reader opens a raw volume handle (`\\.\X:`) and therefore needs administrator rights;
  without them it reports the reason and the directory walk runs. No WizTree-level performance claim
  is made.

## Cleanup guarantees and limits

- **Catalog-only.** There is no command that deletes a caller-supplied path. Eligibility comes from
  the fixed catalog and a guard that refuses protected locations and their ancestors.
- **Moves, not deletes.** Execution renames each eligible file into the run's quarantine batch and
  writes a manifest during the run. The only permanent removal is `purge_quarantine`, which deletes
  the files of one manifest that are still inside that batch folder. There is no recursive delete;
  empty directories are removed one at a time with `remove_dir`.
- **Off by default.** `AppState::read_only` starts `true`. Previewing still works there because it
  only reads sizes and paths (that is the dry run); `start_cleanup` refuses while the switch is set.
  Enabling cleaning is an explicit user action, and every run still needs the confirmation checkbox.
- **Protected data.** 12 catalog entries are `protected` (Claude Code projects, Codex sessions,
  Gemini history, Ollama and LM Studio models, VS Code/Cursor workspace storage and local history,
  Docker data, WSL distros, Firefox profiles, the Windows Update download cache). They are measured
  and explained but never planned. Hard-linked and reparse-point files are skipped.
- **Locked and system-owned files are left alone.** No elevation request, no restart manager, no VSS
  snapshot, no service stop, no process termination. Failures are reported per item.
- **User exclusions are enforced server-side**, so a preview cannot show a file that will be left
  behind, and an execution cannot touch an excluded folder even if the UI is wrong.
- **Quarantine retention is metadata, not a timer.** The 7-day `expiresAt` is recorded and shown;
  nothing is deleted automatically, and purging is always an explicit user action.
- **Residual risk:** a malicious local process running as the same user can read the quarantine and
  the cleanup log, or modify the plan registry through the same privileges it already has. Cyron
  does not defend against an attacker who already controls the user's session; it defends against
  its own mistakes, hostile inputs through IPC, and UI-level accidents.

## Network and data handling

There is no application network or telemetry code. Scanned paths, duplicate hashes, quarantine
manifests and the cleanup log remain local. Cyron contains no network feature.

## Gates satisfied across Phases 3 and 4

Phases 3 and 4 satisfy all safety requirements for file cleanup, process termination, uninstaller
execution, and residual leftover management:

1. A narrowly scoped provider that derives eligible targets from a reviewed catalog or verified
   application metadata, never from a user-controlled arbitrary path or name alone. *(Phase 3 & 4:
   satisfied — catalog ids and verified application install roots only.)*
2. A dry-run preview showing exact paths/registry values/processes, expected impact, risk and
   reasons, with protected locations excluded by backend enforcement. *(Satisfied for files, processes,
   and registry leftovers.)*
3. Explicit, contextual confirmation immediately before execution, with revalidation of identity and
   scope to defeat time-of-check/time-of-use changes. *(Satisfied — plan TTL, selection re-check,
   snapshot binding, per-file metadata and parent revalidation.)*
4. Path canonicalization, boundary checks, reparse-point checks, and safeguards against root, profile,
   repository, credential, model, Docker/WSL and shared-dependency removal. *(Satisfied for catalog
   caches and leftover paths; system roots and top-level profile folders refused.)*
5. Registry export/backup before registry modifications, plus rollback/recovery for supported file
   operations and clear reporting when rollback is impossible. *(Satisfied — automatic `.reg` backup
   to quarantine batch folder before subkey deletion; durable write-ahead recovery journal for files.)*
6. Process identity checks and critical-process protection before termination; never use shell
   interpolation for process control or uninstall execution. *(Satisfied — executable path must be
   inside application root; critical system processes protected; Win32 APIs used without shells.)*
7. An append-only or otherwise tamper-aware operation record with outcome, timing and recovery
   details, never logging secrets or file contents. *(Satisfied — write-ahead recovery journal,
   manifest, `cleanup-log.json`, and Activity entries.)*
8. Tests against temporary fixtures and faulted conditions, including permissions, races, locked
   files, reparse points and rollback failure, plus a security review before release. *(Phase 3:
   `tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`; review pending a
   release build.)*

---

## Priority 0 Security Hardening

Phase 4.1 adds critical defense-in-depth hardening across all destructive operations:

1. **Uninstaller Execution Boundary**:
   - The backend never trusts client-supplied executable paths, arguments, or `isValid` flags.
   - The uninstaller command is resolved server-side from registered Windows uninstall keys using `app_id`.
   - Single-use, expiring confirmation plans (`UninstallPlan`, 10-minute TTL) are retained in `AppState`.
   - Re-validation against live system registry immediately prior to execution ensures the registered command was not tampered with.
   - Shell wrappers (`cmd.exe`, `powershell.exe`, `pwsh`, `bash`, scripts) are strictly rejected.
   - MSI product codes and flags are normalized; command-line argument chaining (`&`, `|`, `>`, `<`) is stripped.

2. **Process Termination Verification**:
   - Process discovery captures a verified snapshot (`StoredProcessSnapshot`) with 64-bit creation times (`GetProcessTimes`).
   - Termination requires `app_id` and checks against the verified snapshot.
   - Process image path must reside strictly inside the application's canonical install location.
   - Process creation time is re-queried immediately before termination to prevent attacks against recycled PIDs.
   - Critical system processes (`csrss.exe`, `smss.exe`, `services.exe`, `lsass.exe`, `explorer.exe`, etc.) and system directories (`System32`, `SysWOW64`) are unconditionally protected.
   - Separate confirmation is enforced for force termination (`TerminateProcess`) after graceful `WM_CLOSE` is attempted.

3. **Leftover & Residual Scanner Hardening**:
   - Residual candidates are generated server-side with a single-use plan (`LeftoverPlan`, 10-minute TTL).
   - Candidate directory paths are re-guarded (`guard_leftover_path`); top-level roots, user profile roots, and system directories are refused.
   - Non-following, bounded directory walker skips reparse points, symlinks, and junctions.
   - Quarantining uses a durable write-ahead journal (`recovery_journal.jsonl`) with physical disk synchronization (`flush()` and `sync_all()`).

4. **Registry Safety & Multi-Type Backup**:
   - Recursive registry export preserves all value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) and all nested subkeys.
   - Backup file integrity (header, size, non-empty structure) is strictly verified before calling `delete_subkey_all`.
   - Shared vendor keys (`Software\Microsoft`, `Software\Google`, `Software\Apple`, `Software\Intel`, etc.) are protected from deletion.
   - Roundtrip backup and restoration is verified using isolated test fixtures.

5. **Recovery Correctness**:
   - Recovery journals are appended and physically synchronized (`sync_all`) before any destructive `fs::rename`. If journal write or synchronization fails, the move is skipped (fail closed) to ensure recoverability.
   - Restoring files verifies that destination parent directories exist and are not symlinks/junctions.
   - Win32 `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` prevents destination overwrites; on the same volume this provides atomic rename, and across volumes it safely copies and removes without overwriting.

6. **Batch Uninstallation & Process Identity Fail-Closed Security (RC1)**:
   - Batch uninstalls require a server-held, single-use, 10-minute expiring `BatchUninstallPlan`. The client cannot execute caller-chosen commands or arbitrate batch queue order.
   - Exact executable and argument lists are presented to the operator for mandatory review and explicit confirmation before batch execution.
   - Live registry uninstaller commands are re-validated immediately before launching each queue item.
   - Process creation time verification fails closed: if a PID's creation time cannot be determined or does not match the snapshot, termination is refused to protect against PID recycling races.
   - Recursive registry backup propagates all subkey enumeration and value errors without silent skipping.
   - Registry backup validation checks syntax structure across all 6 Windows value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) and nested subkeys before deleting any key.
   - Registry restoration reports all failures and never claims success on partial recovery.

7. **Explorer Integration Boundary & Path Sanitization (Phase 6)**:
   - Shell verb registration is strictly per-user under `HKCU\Software\Classes` and does not require administrator privileges.
   - Context-menu commands invoke Cyron with tightly restricted command-line arguments: `--analyze`, `--duplicates`, `--storage`, `--uninstall-target`, and `--find-related`.
   - All input paths received from CLI arguments are canonicalized and validated (`sanitize_target_path`). Raw UNC device paths (`\\.\`), wildcard characters, control characters, and nonexistent paths are rejected.
   - Invoking Cyron from Explorer opens the corresponding inspection view in Cyron; it **never** triggers uninstallation, process termination, file quarantine, or any destructive action automatically.
   - Executable right-clicks (`--uninstall-target` and `--find-related`) query registered Win32, Store, Steam, and portable applications server-side to match the target binary. They never execute the clicked file or assume an unregistered executable can be safely uninstalled.

8. **Extensible Declarative Cleanup Provider Schema Validation (Phase 6)**:
   - Providers are validated against `schema_version: 1` specifications before being accepted into the cleaning engine.
   - Target roots must strictly use approved environment variable tokens (`%USERPROFILE%`, `%LOCALAPPDATA%`, `%APPDATA%`, `%TEMP%`, `%PROGRAMDATA%`, `%SYSTEMROOT%`).
   - Path traversal components (`..`) and directory wildcards are strictly rejected.
   - Providers are forbidden from targeting entire system roots, user profiles, or Windows system folders.
   - Providers cannot override or demote protected catalog targets (source repositories, virtual environments, Docker/WSL, model weights, browser profiles) to safe or caution risks.

9. **Broken Inventory Cleanup, Hunter Mode & Startup Safety Hardening (Phase 7.1 & 7.2)**:
   - Broken inventory cleanup resolves the actual subkey name from discovered application metadata (`registry_key_name`), strictly rejecting frontend-supplied arbitrary paths.
   - Server-held, single-use, 5-minute expiring confirmation plan (`BrokenEntryPlan`) requires explicit operator confirmation displaying the exact registry path.
   - Plans are bound to a cryptographic SHA-256 fingerprint of live registry metadata (`BrokenEntryMetadata`: hive, subkey name, registry path, display name, uninstall string, quiet uninstall string, install location, publisher, display version).
   - Validates that the entry qualifies as broken (missing uninstaller or invalid path) at preparation and execution time.
   - Live metadata and broken status are re-read and re-verified immediately before deletion; altered, tampered, recovered, or no-longer-broken entries are refused without deletion (defeats TOCTOU races).
   - Generates collision-resistant plan IDs (`generate_collision_resistant_plan_id`) combining process ID, monotonic timestamps, and crypto nonces.
   - Creates a verified multi-type `.reg` backup in quarantine, writes a durable write-ahead recovery journal (`recovery_journal.jsonl`) and manifest (`manifest.json`), and respects backend read-only mode.
   - Supports verified in-app restoration of quarantined registry backups via native `winreg` restoration (`restore_registry_backup` IPC command) with SHA-256 checksum binding, section scope restriction, and silent overwrite prevention.
   - System-protected entries, root uninstall keys, altered paths, and replay attacks are refused.
   - Hunter Mode separates cursor targeting from application discovery; targets match against cached in-memory apps in under 1 µs, eliminating repeating disk/registry scans.
   - Hunter process snapshots and app snapshots are completely isolated into dedicated state maps (`hunter_snapshots`, `hunter_app_snapshots`) using unique opaque tokens (`hunter-target-<nanos>-<nonce>-<pid>`), ensuring concurrent Hunter cursor polling never overwrites or mutates active Uninstaller process snapshots.
   - Target acquisition registers verified process snapshot (`StoredProcessSnapshot`) and app identity in backend state; termination revalidates PID, path, and creation time immediately before kill.
   - Strict directory boundary matching (`is_path_in_directory`) prevents prefix collisions (`C:\Apps\Foo` vs `C:\Apps\FooBar`).
   - Unregistered application forced removal creates a backend-verified app snapshot, protecting system roots and requiring operator confirmation.
   - Startup Manager toggling uses Windows standard `StartupApproved` registry keys, preserving the exact 12-byte binary payload, setting byte 0 (`0x02` enable, `0x03` disable) and updating FILETIME timestamps in bytes 4..12 matching Windows Task Manager. HKLM write permissions are checked (`is_hklm_startup_writable`), disabling toggles (`can_toggle: false`) when running without elevation instead of indicating false success. Original commands and Windows security components (Windows Defender, SecurityHealthSystray, SmartScreen) are strictly protected.

10. **Explorer Destructive Actions & Restart Manager Locking Security (Milestone 7.4)**:
    - Context-menu commands never perform direct deletion or termination from shell arguments; they always route to interactive modal dialogs backed by server-held, single-use, 10-minute expiring plans (`StoredForceDeletePlan`, `StoredKillProcessPlan`).
    - Protected system targets are strictly refused by `is_protected_delete_target`: Windows directory, System32, SysWOW64, Program Files roots, User Profile roots, Volume roots (`C:\`), boot configuration files, pagefile/swapfile/hiberfil, and WindowsApps.
    - File and directory deletion re-validates path existence, canonical roots, and metadata immediately prior to deletion.
    - Deletion defaults to a recoverable 7-day quarantine batch with crash-consistent recovery journals (`recovery_journal.jsonl` with `sync_all`); permanent deletion requires explicit multi-step confirmation with item count verification.
    - Active file/folder locking processes are detected using official Windows Restart Manager APIs (`rstrtmgr.dll`: `RmStartSession`, `RmRegisterResources`, `RmGetList`).
    - Process termination requires matching exact executable paths (never filename alone), captures 64-bit creation times to block PID recycling, unconditionally protects critical Windows system processes, and reuses the single safe engine (`process::terminate_single_process`).
    - Read-only attribute clearing uses safe standard `std::fs::set_permissions`; directory deletion removes children individually without unchecked recursive deletion.

---

## Code Signing & Release Security

- **Release Classifications**:
  - `UNSIGNED_RC`: Explicitly labelled unsigned release candidate for private testing.
  - `CHRYYPT_SIGNED`: Official Authenticode code-signed releases under the Chryypt Organization (`CN=Chryypt Organization`, RSA 3072, SHA-256) via `scripts/sign-build.ps1` (`npm run build:signed`).
- **Verification Tooling**:
  - `npm run verify:metadata`: Validates PE VERSIONINFO, WiX MSI properties, NSIS headers, and application identifiers (`WINDOWS_METADATA_OK`).
  - `npm run verify:signatures`: Validates cryptographic file integrity, certificate thumbprints, and negative test boundaries (`SIGNATURES_VERIFIED_OK`).
- **Reproducible Checksums**:
  - Every release publishes SHA-256 hashes in `SHA256SUMS.txt`.

---

## Git History Privacy Findings & Remediation

- **Remediation Executed**:
  Per explicit operator directive ("remove .chryypt in github"), historical internal session metadata (`.chryypt/sessions/` and `.chryypt/state/`) has been purged from the repository history using `git-filter-repo --path .chryypt --invert-paths`.
- **Enforcement**:
  The `.gitignore` strictly rejects any `.chryypt/` directory or file from ever being tracked in future commits.
  Pending authorization, access controls and branch protection remain in place.
