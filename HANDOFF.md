# Cyron — Active Handoff

Updated: 2026-11-06 (Phase 4.1 Priority 0 Hardening & Phase 5 Release Engineering complete)

## Current status

Phases 1–5 and Priority 0 security hardening are implemented and verified across the Rust backend
and React desktop interface. Cyron discovers installed Windows applications, scans folders with an
NTFS MFT reader or a streaming directory walk, renders a treemap with a persistent synchronized folder
tree, verifies duplicates, categorizes storage, measures 48 developer/AI/Windows cache locations, cleans
caches safely via recoverable quarantine, uninstalls applications with server-side command resolution
and reboot alerts, terminates associated processes with snapshot verification and PID recycling guards,
scans leftovers with risk ratings, exports multi-type recursive registry backups, cleans residuals into
quarantine with durable write-ahead journals (`sync_all`), and executes sequential batch uninstall queues.

Priority 0 hardening has secured all destructive operations:
1. **Uninstaller execution boundary**: Client-supplied executable paths and arguments are untrusted.
   Commands are resolved server-side from registered keys into single-use, 10-minute expiring plans
   (`UninstallPlan`), with unquoted path support, MSI normalization, and live-registry revalidation.
2. **Process termination**: Verified process snapshots with 64-bit creation times (`GetProcessTimes`),
   strict executable install-location verification, PID recycling denial, and unconditional critical
   system process protection.
3. **Leftover cleanup**: Server-side `LeftoverPlan` with TTL, bounded non-following directory walker
   skipping junctions and symlinks, and durable write-ahead recovery journals (`sync_all`).
4. **Registry safety**: Recursive `.reg` export supporting all registry value types (`REG_SZ`, `REG_DWORD`,
   `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) and subkeys, verified backup integrity
   prior to deletion, and verified restore helper.
5. **Recovery correctness**: Physical disk synchronization (`sync_all`), destination ancestor reparse
   checks, safe purge retries, and honest same-volume vs cross-volume restoration semantics.

Release packaging has been verified locally (`RELEASE_BUNDLES_OK`):
- **NSIS Setup**: `src-tauri/target/release/bundle/nsis/Cyron_0.1.0_x64-setup.exe` (1.42 MiB, SHA-256: `29d884334a871c310047bc5191c9d2c2b9a1410b7450967943635c419bdcc0e0`).
- **WiX MSI Package**: `src-tauri/target/release/bundle/msi/Cyron_0.1.0_x64_en-US.msi` (2.00 MiB, SHA-256: `29453c42694a87723fe30b4827d4e9a92e52413038e9daded7592fd277271dc8`).

Latest measured verification:
- `npm run typecheck`: exit 0 (0 diagnostics).
- `npm test`: 71/71 tests pass across 7 files.
- `node scripts/verify-dist.mjs`: `DIST_OK` (2 assets, 448.8 KiB).
- `cargo test`: **153 tests pass** (120 unit, 22 security, 5 cleanup_stress, 1 real_system, 5 stress_scan) with 3 ignored.
- `node scripts/verify-cleanup.mjs`: `CLEANUP_FIXTURE_OK`.
- `node scripts/verify-desktop-binary.mjs`: `DESKTOP_BINARY_OK` (15.83 MiB).
- `node scripts/smoke-desktop.mjs`: `DESKTOP_SMOKE_OK` ("Cyron").
- `node scripts/verify-release-bundles.mjs`: `RELEASE_BUNDLES_OK`.

## Verification table

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend tests | `npm test` | 71/71 pass, 7 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` (448.8 KiB) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 153 pass, 3 ignored |
| Permission boundaries | `... --test security -- --nocapture` | 22/22 pass |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Scanner stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (15.83 MiB) |
| Launch smoke test | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron") |
| Release packaging | `node scripts/verify-release-bundles.mjs` | `RELEASE_BUNDLES_OK` (NSIS + MSI) |
| GitHub CI & Release | `.github/workflows/ci.yml`, `release.yml` | Workflows configured |

## Key files

- `src-tauri/src/process.rs`: process discovery, snapshot tracking, PID creation time checks, critical process deny list, controlled termination.
- `src-tauri/src/uninstall.rs`: server-side command resolution, `UninstallPlan` creation, safe uninstaller tokenization, execution, exit code tracking.
- `src-tauri/src/leftovers.rs`: server-side `LeftoverPlan`, bounded non-following directory walker, recursive multi-type `.reg` backup and restore, residual quarantine.
- `src-tauri/src/cleanup.rs`: cleanup engine, durable write-ahead recovery journal (`sync_all`), quarantine, atomic no-overwrite restore, audit log.
- `src-tauri/src/commands.rs`: 27 validated IPC commands.
- `src-tauri/tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`: security & stress suites.
- `src/features/uninstaller/AppDrawer.tsx`, `Uninstaller.tsx`: uninstaller UI, plan-based execution, process manager, residual reviewer, batch queue.
- `scripts/verify-release-bundles.mjs`: verification of release MSI and NSIS installers with SHA-256 hashes.
- `docs/USER_GUIDE.md`, `CONTRIBUTING.md`, `docs/RELEASE_CHECKLIST.md`: comprehensive documentation.
- `GATES.md`: acceptance ledger with recorded evidence.

## Known limitations & Release considerations

- **Authenticode signing**: Binary and installers are unsigned release candidates. Production distribution requires an Authenticode certificate and timestamping (documented in `docs/RELEASE_CHECKLIST.md`).
- **Store/MSIX apps**: Enumeration is deferred to a distinct read-only inventory source; uninstallation of Store apps requires native WinRT COM interfaces (`Windows.Management.Deployment.PackageManager`) without shell scripts (documented in `docs/ARCHITECTURE.md`).
- **Git history privacy**: Historical internal session data under commit `62414cc` remains in Git blobs. Remediation via `git-filter-repo` is documented and awaits operator authorization.
- **Elevation**: Cyron never requests administrator rights for cleanup; locked or system-owned files are reported and skipped. MFT scanning requires an elevated shell for raw volume handle access.
- **Batch uninstaller**: sequential execution queue with isolated per-item results and exit codes.
  Prevents automatic follow-up cleanup approval, requiring separate user review before residual deletion.
- **Priority 0 cleanup recovery hardening**:
  - Durable crash-consistent write-ahead recovery journal (`recovery_journal.jsonl`) written and
    flushed before any file move. Corrupted or missing manifests reconstruct automatically.
  - Atomic no-overwrite restoration using Win32 `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`,
    preventing destination overwrite races.
  - Snapshot binding: files created after preview approval are detected and skipped with warnings.
  - Safe purge retry: locked files leave remaining manifest and journal items intact.
  - Quarantine storage caps (`MAX_QUARANTINE_TOTAL_BYTES` = 20 GiB) and minimum free disk space check
    (`MIN_FREE_SPACE_BYTES` = 512 MiB via Win32 `GetDiskFreeSpaceExW`).
- **UI Integration**: `AppDrawer.tsx` extended with Segmented tabs (Overview, Processes, Uninstall, Leftovers),
  live process management, uninstaller execution launcher, and leftover scanner with candidate selection.
  `Uninstaller.tsx` updated with multi-select checkboxes and a bottom batch uninstall drawer.

## Key files

- `src-tauri/src/process.rs`: process discovery, critical process deny list, termination.
- `src-tauri/src/uninstall.rs`: safe uninstaller tokenization, execution, exit code handling.
- `src-tauri/src/leftovers.rs`: leftover detection, .reg backup, residual cleanup.
- `src-tauri/src/cleanup.rs`: cleanup engine, recovery journal, quarantine, guard, log.
- `src-tauri/src/commands.rs`: 26 validated commands, process/uninstall/leftover IPC.
- `src-tauri/tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`: security & stress evidence.
- `src/features/uninstaller/AppDrawer.tsx`, `Uninstaller.tsx`: uninstaller UI, drawer tabs, batch queue.
- `src/lib/api.ts`, `src/lib/types.ts`: IPC contracts.
- `GATES.md`: Phase 4 acceptance ledger with recorded evidence.

## Verification

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass |
| Frontend tests | `npm test` | 71/71 pass, 7 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` (448.7 KiB) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 148 pass, 3 ignored |
| Permission boundaries | `... --test security -- --nocapture` | 21/21 pass |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Scanner stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored 20k-file test) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (15.73 MiB) |
| Launch | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron") |
| Cleaner UI pass | Native window, both themes, `scripts/capture-window.ps1` | Pass — 28/48 locations exist, 24.8 GB detected; two defects fixed |
| Windows CI | GitHub Actions | Workflow extended; remote run not observed |

## Known limitations

- The debug desktop executable loads the Vite dev server; launching it without `npm run dev` shows a
  connection error. Release packaging/signing is unverified.
- Preview reads sizes, so read-only mode gates execution only; the Cleaner still requires the plan
  id and the confirmation checkbox, and refuses to plan Protected entries.
- Cleaning is catalog-scoped: custom install paths, projects and files outside the 48 known
  locations are never touched.
- Locked, in-use or permission-denied files are skipped and reported; no restart, service stop or
  process termination is used to force them.
- Quarantine has no size cap or automatic expiry sweep; the 7-day `expiresAt` is informational.
- Restore can fail if the original path now exists (the quarantined copy is kept and reported) or if
  the original folder was removed and cannot be recreated.
- The Windows Update download cache is measured as protected; Storage Sense/Disk Cleanup remain the
  supported route.
- MFT needs administrator rights and NTFS; whole-volume MFT and walk totals differ by counting rule.
- Benchmarks are warm-cache, single-machine; no WizTree comparison.
- Store/MSIX inventory is still missing; the IPC tree is pruned; duplicates are candidate-based.
- Git history still contains the original `.chryypt` files. Rewriting history needs explicit
  approval.
- Windows CI has been added but its remote run has not been observed from this machine.

## Next directive

Phase 4 (uninstall engine) may start from this baseline. It needs process discovery with a
critical-process deny list, registry export/backup and restore, a plan model for non-file
operations, shared-dependency and system-component protection, and fixtures for vendor uninstaller
execution — plus extending `tests/security.rs` module boundaries to every new destructive module.
Do not add a hidden deletion, termination or registry-write path outside that plan/confirm/quarantine
pattern.
