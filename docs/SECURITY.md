# Security model

## Scope and assets

Cyron Phase 1 is a local Windows desktop scanner. It reads installed-application metadata, enumerates filesystem metadata, measures a fixed catalog of cache locations, and stores scan history in the current user's Tauri application-data directory. The protected assets include user files, repositories, credentials, application settings and histories, AI model weights, container/WSL data, registry contents, and the integrity of the scan results.

The current backend does not uninstall applications, clean scanned files, terminate processes, write registry values, or invoke a shell. The only launched child process is `explorer.exe`, used to reveal a validated existing path on explicit user action. Clearing the Activity history is limited to Cyron's own `history.json`; it does not clear scan targets.

## Trust boundaries

- **Frontend to Rust IPC:** The webview is treated as an untrusted caller. Commands validate scan job IDs and validate user paths as absolute, existing paths; scan roots are canonicalized and required to be directories. Exclusions and depth are bounded at the command boundary. No user-provided value is interpolated into a shell command.
- **Windows filesystem and registry:** Filesystem metadata and registry values are external input and may be missing, malformed, inaccessible, or concurrently changed. Enumeration reports bounded errors rather than assuming access or completeness. Application discovery reads uninstall registry keys only.
- **Reparse points and links:** Directory entries marked as reparse points are skipped; junctions and symbolic links are not followed. Stable file IDs deduplicate hard-linked file allocations, subject to the bounded tracking cap below.
- **Tauri webview/plugins:** The dialog plugin can request a folder selection. This does not grant the backend delete or registry-write operations. Keep Tauri capabilities minimal and review new plugins and permissions before adding them.
- **Local history/settings:** Settings are stored by the frontend in local storage. Scan history is written under Tauri's per-user app-data directory, capped at 100 entries. It can include scanned root paths and summary counts; users should treat the local history as private profile data.

## Scanner guarantees and limits

- The filesystem scanner is read-only: it enumerates directories and metadata and does not modify scan targets.
- Junctions, symbolic links, mount points, and other reparse points are not traversed.
- Hard links are counted once for allocated bytes while logical bytes reflect file names. Tracking is capped at one million file IDs to limit memory. After that limit, the scanner reports that later links may count twice; it does not claim perfect deduplication for larger walks.
- Per-directory file nodes, error messages, extension summaries, largest-file results, recursion depth and IPC tree size are bounded. The displayed tree is a summarized view, not a complete filesystem listing.
- Permission failures and missing paths can produce incomplete results; absence from a result is not proof that a location is empty. Cache discovery may be unable to inspect system-protected folders without elevation and does not request elevation automatically.
- The scanner is a directory traversal implementation; it does not use the NTFS Master File Table (MFT). No WizTree-level performance claim is made.
- Windows Store/MSIX applications are not currently discovered. Duplicate-file hashing, cleanup, uninstall, process termination, and registry modification are not implemented.
- A source-inspection integration test blocks known destructive filesystem, registry, process, and shell API patterns in the backend. Source inspection is a regression tripwire, not a complete security proof; review dependencies and changes to the test itself.

## Network and data handling

There is no application network or telemetry code. Scanned paths and result metadata remain local. The scanner does not upload data. Normal Windows services and the application's dependencies are outside this statement; Cyron itself contains no network feature in Phase 1.

## Gates before destructive capabilities

No delete, cleaner, uninstaller, process termination, or registry-write path may be added as a hidden shortcut. Before Phase 3/4 destructive work is enabled, each operation must have all applicable safeguards:

1. A narrowly scoped provider that derives eligible targets from a reviewed catalog or verified application metadata, never from a user-controlled arbitrary path or name alone.
2. A dry-run preview that shows exact paths/registry values/processes, expected impact, risk, and reasons; protected locations must be excluded in backend enforcement, not merely hidden in the UI.
3. Explicit, contextual confirmation immediately before execution, with revalidation of identity and scope to mitigate time-of-check/time-of-use changes.
4. Path canonicalization, boundary checks, reparse-point checks, and safeguards against root, profile, repository, credential, model, Docker/WSL, and shared-dependency removal.
5. Registry export/backup before registry modifications, plus rollback/recovery for supported file operations and clear reporting when rollback is not possible.
6. Process identity checks and critical-process protection before termination; never use shell interpolation for process control or uninstall execution.
7. An append-only or otherwise tamper-aware operation record with outcome, timing, and recovery details; do not log secrets or file contents.
8. Tests against temporary fixtures and mocked/faulted conditions, including permissions, races, locked files, reparse points, and rollback failure. Run security review and explicit acceptance checks before enabling the feature in a release.
