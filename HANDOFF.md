# Cyron — Active Handoff

Updated: 2026-11-06 (Release Candidate 1 — RC1 Hardening complete)

## Current status

Phases 1–5, Priority 0 security hardening, and Release Candidate 1 (RC1) hardening are implemented and verified across the Rust backend
and React desktop interface. Cyron discovers installed Windows applications, scans folders with an
NTFS MFT reader or a streaming directory walk, renders a treemap with a persistent synchronized folder
tree, verifies duplicates, categorizes storage, measures 48 developer/AI/Windows cache locations, cleans
caches safely via recoverable quarantine, uninstalls applications with server-side command resolution
and reboot alerts, terminates associated processes with snapshot verification and PID recycling guards,
scans leftovers with risk ratings, exports multi-type recursive registry backups, cleans residuals into
quarantine with durable write-ahead journals (`sync_all`), and executes sequential batch uninstall queues under server-held single-use confirmation plans.

RC1 Hardening completed:
1. **Batch uninstall confirmation plan**: Server-held, single-use, 10-minute expiring confirmation plan (`BatchUninstallPlan`), live registry command revalidation, and explicit operator approval of executables and arguments before execution.
2. **Fail-closed leftover file cleanup**: Durable write-ahead journal entry with physical disk synchronization (`sync_all`) must succeed before moving any file; failures abort file movement to prevent unrecoverable data loss.
3. **Recursive registry backup error propagation**: All subkey enumeration, value reading, and file write errors propagate immediately instead of silently skipping entries.
4. **Registry backup integrity verification & multi-type roundtrip**: Checks header, file length, and syntax structure across all 6 Windows value types (`REG_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) and nested subkeys before deleting any key.
5. **Registry restoration error reporting**: Reports exact counts of restored keys and values, captures every failure, and never claims success on partial restoration.
6. **Process creation-time fail-closed verification**: Process termination rejects processes whose creation time cannot be determined or does not match the snapshot, preventing PID recycling races.
7. **CI runner environment fix**: Patched MSVC toolchain wrapper (`scripts/msvc-env.mjs`) to preserve Windows `Path` and locate `cargo` in CI runners, resolving the `verify-cleanup.mjs` failure.
8. **End-to-end fixture test suite**: Added `rc1_e2e_fixture.rs` and `scripts/verify-e2e.mjs` (`RC1_E2E_FIXTURE_OK`) covering uninstallation, process management, registry backup/restore, and residual cleanup.
9. **UI/UX audit & branding cleanup**: Removed obsolete "Phase 1 is read-only" from bundle descriptions and Settings, verified all six views.
10. **Release automation gating**: `.github/workflows/release.yml` enforces test suites, runs installer verification, and publishes unsigned release candidates as draft pre-releases with explicit disclaimers.

Release packaging has been verified locally (`RELEASE_BUNDLES_OK`):
- **NSIS Setup**: `src-tauri/target/release/bundle/nsis/Cyron_0.1.0_x64-setup.exe` (1.42 MiB, SHA-256: `29d884334a871c310047bc5191c9d2c2b9a1410b7450967943635c419bdcc0e0`).
- **WiX MSI Package**: `src-tauri/target/release/bundle/msi/Cyron_0.1.0_x64_en-US.msi` (2.00 MiB, SHA-256: `29453c42694a87723fe30b4827d4e9a92e52413038e9daded7592fd277271dc8`).

Latest measured verification:
- `npm run typecheck`: exit 0 (0 diagnostics).
- `npm test`: 71/71 tests pass across 7 files.
- `node scripts/verify-dist.mjs`: `DIST_OK` (2 assets, 450.6 KiB).
- `cargo test`: **157 tests pass** (120 unit, 25 security, 5 cleanup_stress, 1 real_system, 6 stress_scan) with 3 ignored.
- `node scripts/verify-cleanup.mjs`: `CLEANUP_FIXTURE_OK`.
- `node scripts/verify-e2e.mjs`: `RC1_E2E_FIXTURE_OK`.
- `node scripts/verify-desktop-binary.mjs`: `DESKTOP_BINARY_OK` (15.89 MiB).
- `node scripts/smoke-desktop.mjs`: `DESKTOP_SMOKE_OK` ("Cyron").
- `node scripts/verify-release-bundles.mjs`: `RELEASE_BUNDLES_OK`.

## Verification table

| Check | Command | Last result |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend tests | `npm test` | 71/71 pass, 7 files |
| Frontend build | `node scripts/verify-dist.mjs` | `DIST_OK` (450.6 KiB) |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 157 pass, 3 ignored |
| Permission boundaries | `... --test security -- --nocapture` | 25/25 pass |
| Cleanup stress | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| End-to-end fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK` |
| Real system | `... --test real_system -- --nocapture` | Pass |
| Scanner stress | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored) |
| Desktop binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (15.89 MiB) |
| Launch smoke test | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron") |
| Release packaging | `node scripts/verify-release-bundles.mjs` | `RELEASE_BUNDLES_OK` (NSIS + MSI) |
| GitHub CI & Release | `.github/workflows/ci.yml`, `release.yml` | Workflows gated & configured |

## Key files

- `src-tauri/src/process.rs`: process discovery, snapshot tracking, PID creation time checks (`GetProcessTimes`), critical process deny list, fail-closed controlled termination.
- `src-tauri/src/uninstall.rs`: server-side command resolution, `UninstallPlan` & `BatchUninstallPlan` creation, safe uninstaller tokenization, execution, exit code tracking.
- `src-tauri/src/leftovers.rs`: server-side `LeftoverPlan`, bounded non-following directory walker, recursive multi-type `.reg` backup and restore, backup integrity verification, fail-closed residual quarantine.
- `src-tauri/src/cleanup.rs`: cleanup engine, durable write-ahead recovery journal (`sync_all`), quarantine, atomic no-overwrite restore, audit log.
- `src-tauri/src/commands.rs`: 28 validated IPC commands.
- `src-tauri/tests/security.rs`, `tests/cleanup_stress.rs`, `examples/cleanup_fixture.rs`, `examples/rc1_e2e_fixture.rs`: security, stress, & e2e suites.
- `src/features/uninstaller/AppDrawer.tsx`, `Uninstaller.tsx`: uninstaller UI, plan-based execution, process manager, residual reviewer, reviewed batch queue.
- `scripts/verify-release-bundles.mjs`: verification of release MSI and NSIS installers with SHA-256 hashes.
- `scripts/verify-e2e.mjs`: runner for uninstaller and residual recovery end-to-end fixture.
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
