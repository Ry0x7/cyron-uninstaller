# Cyron — Independent Release Audit (2026-11-17)

This is the Phase 8 audit of the current working tree. It records what was executed, on which
machine, and what remains unverified. It is not a production-readiness claim: the trust and
real-world removal gaps at the end still block a public release.

## Verdict

**NOT PRODUCTION READY — release blocked.**

The application is functionally strong on the paths that were exercised: the full Rust suite,
frontend suite, documentation check, cleanup/registry fixtures and ten real WebView2 desktop
journeys pass. The NSIS installer was installed, repaired, relaunched and uninstalled with app data
preserved; the release bundles pass metadata verification. The blockers are external trust and
unexercised native paths, not a known data-loss defect found by these suites:

1. The locally built installer and binaries are **unsigned**; the Chryypt certificate is
   **self-signed and untrusted**. The published `v0.1.0` NSIS asset is also `NotSigned`.
   SmartScreen will warn, and no warning-free install is claimed.
2. No real vendor/MSI uninstall, Microsoft Store/MSIX removal or elevated MSI install was executed.
3. Elevated MFT scanning, elevated boot-impact reading and WPR tracing are unverified.
4. 125%/150% OS scaling, Narrator/NVDA, forced colors and 200% text are unverified.
5. No competitor was installed, so no comparative performance or leftover-accuracy result exists.
   Competitor cells remain NOT RUN.
6. Crash/power-loss injection and cross-version upgrade/downgrade were not performed.

## Tested revision and machine

| Item | Value |
|---|---|
| Repository | `E:\Github\cyron-uninstaller`, branch `main` |
| Base commit | `3d9f2daddb527492afc578c43dc70c1a7a7ce6f2` plus the uncommitted Phase 8 working tree |
| OS | Windows 11 Pro x64, build `10.0.26200` |
| CPU | AMD Ryzen 7 5700X, 8 cores / 16 threads |
| RAM | 47.9 GB |
| Volumes | C: 464.7 GB (100.1 GB free), D: 238.5 GB (165.4 GB free), E: 476.9 GB (58.9 GB free) |
| Node.js | v24.18.0 |
| WebView2 / driver | 157.0.4341.46 |
| Rust toolchain | MSVC via `node scripts/msvc-env.mjs` |
| Elevation during audit | None |
| Desktop build tested | `src-tauri/target/debug/cyron.exe` |
| Release bundle tested | NSIS `0.1.0`, built from this tree; installed to `%LOCALAPPDATA%` |

## Reproducible scenarios and results

| # | Scenario | Input | Method | Result | Evidence |
|---|---|---|---|---|---|
| A1 | Frontend contracts | 12 Vitest files | `npm test` | **PASS** — 100 tests | local command output |
| A2 | Type safety | whole frontend | `npm run typecheck` | **PASS** — exit 0 | local command output |
| A3 | Rust unit/integration | whole crate | `cargo test --manifest-path src-tauri/Cargo.toml` | **PASS** — 261 passed, 4 ignored, 0 failed | local command output |
| A4 | Documentation integrity | all Markdown | `npm run docs:check` | **PASS** | `DOCS_CHECK: PASSED` |
| A5 | Cleanup round trip | disposable `%TEMP%` fixture | `node scripts/verify-cleanup.mjs` | **PASS** — plan → journal → quarantine → restore → purge | `CLEANUP_FIXTURE_OK` |
| A6 | Uninstall/process/registry fixture | 8 disposable scenarios | `node scripts/verify-e2e.mjs` | **PASS** — 8/8 | `RC1_E2E_FIXTURE_OK` |
| A7 | Hunter targeting latency | 500–1,000 target checks | `node scripts/verify-hunter-perf.mjs` | **PASS** — p50 490 µs, p95 858 µs, p99 999 µs; working-set delta −60 KB | `HUNTER_BENCHMARK_OK` |
| A8 | Desktop executable build | debug build with embedded assets | `npm run build:desktop` | **PASS** — `src-tauri/target/debug/cyron.exe`, 19.02 MiB | `DESKTOP_BINARY_OK` |
| A9 | Real desktop journeys | disposable HKCU apps, uninstaller, project, analyzer and cache fixtures | `npm run test:desktop` | **PASS** — 10/10 journeys, 0 failed | [desktop receipt](evidence/2026-11-17-desktop-e2e.md), [phase8 results](evidence/phase8/results.json) |
| A10 | Directory-walk performance | repo `src`, repo `node_modules`, `C:\Windows\System32`; 3 runs each; unelevated | `CYRON_BENCH_RUNS=3 node scripts/benchmark-competitive.mjs` | **PASS (Cyron only)** — medians 0.00 s / 0.36 s / 0.21 s; peak working sets 5.5 / 11.1 / 8.4 MiB | [BENCHMARKS.md](BENCHMARKS.md) |
| A11 | Release bundle inspection | NSIS, MSI and standalone EXE produced from this tree | `npm run build:unsigned` then `verify-release-bundles` / `verify:metadata` / `verify:signatures` | **Bundles PASS; metadata PASS; signatures FAIL** — all three artifacts `NotSigned` | [installer receipt](evidence/2026-11-17-installer-release.md) |
| A12 | Competitor comparison | Revo, BCUninstaller, WizTree, TreeSize, BleachBit, PC Manager, Sysinternals | none installed or executed | **NOT RUN** — documentation only | [COMPETITOR_COMPARISON.md](COMPETITOR_COMPARISON.md) |
| A13 | NSIS install / repair / relaunch / uninstall / reinstall | locally built current-user installer | silent install, launch, silent uninstall, reinstall; app-data hash comparison and quarantine sentinel | **PASS** — install 1.07 s, repair 1.02 s, uninstall 1.24 s; 0 app-data entries changed; sentinels preserved | [installer receipt](evidence/2026-11-17-installer-release.md), [capture](evidence/phase8/34-installed-launch.png) |
| A14 | MSI install | locally built WiX MSI | `msiexec /qn` without elevation | **UNVERIFIED** — exit 1603 / MSI error 1925 (per-machine package requires administrator) | [installer receipt](evidence/2026-11-17-installer-release.md) |

### What the numbers mean

- Scan timings are wall-clock time around the scanner call only, on a warm cache,
  with no competitor process running. Working set is the process peak after each
  run, not incremental scan memory. Cold-cache and MFT timings are not claimed.
- Hunter p50 is backend targeting latency; WebView2 IPC adds milliseconds and the
  UI polls at a 200 ms non-overlapping interval.
- The desktop suite drives the compiled Tauri window through WebView2/WebDriver and
  real IPC. It does not mock operation results. Its fixtures are disposable; no
  real user cache or project was cleaned.
- Installer timings are wall-clock `Start-Process -Wait` times for silent NSIS runs.
  The window appeared and exited cleanly, but cold-start timing was not recorded as
  a performance metric.

## Adversarial review

| Attack / failure class | Control in this tree | Test evidence | Status |
|---|---|---|---|
| Path traversal through provider definitions | `providers.rs` rejects `..`, unapproved roots, wildcards and sensitive folders; any non-`protected` label on a protected-data marker is refused | `rejects_path_traversal`, `rejects_disallowed_roots`, `rejects_caution_risk_for_protected_data`, `rejects_safe_risk_for_protected_data` | **PASS** |
| Replacing a reviewed target (TOCTOU) | Force Delete binds the plan to volume serial, file index, creation time, size and attributes; directories additionally recheck file/dir/byte counts immediately before action | `force_delete_refuses_a_file_replaced_after_preview`, `force_delete_refuses_a_folder_that_gained_content_after_preview` | **PASS** |
| Junction/symlink retargeting | Force Delete refuses a reparse target; scanning and cleanup never follow reparse points; cross-volume quarantine copy refuses a tree containing a link | `force_delete_refuses_a_junction_instead_of_its_target`, `cross_volume_copy_refuses_a_link_instead_of_dropping_it`, `cleanup_never_moves_links_or_directories`, `scanner_does_not_follow_a_junction` | **PASS** |
| Forged quarantine manifest moving a file outside its batch | `restore_batch` canonicalizes the stored path and requires it inside the batch directory; relative originals are refused | `restore_refuses_a_manifest_entry_outside_its_batch` | **PASS** |
| Registry ownership/backup failure | Recursive multi-type `.reg` export with SHA-256 and scope checks; manifest-write failure prevents deletion; broken entries bind a live metadata fingerprint | `leftover_cleanup_creates_registry_backup_before_deletion`, `manifest_write_failure_prevents_registry_deletion`, `registry_tampering_and_conflict_prevention_tests` | **PASS (fixtures)** |
| PID reuse during termination | `VerifiedProcess` holds a process handle across the graceful wait and force fallback; creation time is revalidated; zero/unknown time fails closed | `verified_handle_rejects_a_wrong_creation_time`, `process_creation_time_fails_closed_when_unverified`, `critical_system_processes_cannot_be_terminated` | **PASS** |
| Interrupted quarantine writes / crash consistency | Write-ahead journal entry is appended and synced before a move; cross-volume copy completes before journalling; restores stage and publish without overwrite | `Fixture` round trips and `quarantine_writes_journal_and_restores_a_folder`; no power-loss injection was performed | **PASS (logic), UNVERIFIED (power loss)** |
| Malicious/hostile plan IDs and paths | Plan IDs are random, single-use, expiring and server-held; hostile run IDs and unknown/relative paths are rejected | `unknown_and_hostile_ids_are_rejected`, `quarantine_operations_reject_hostile_run_ids`, `rejects_missing_or_invalid_paths` | **PASS** |
| Incorrect uninstall claims | A vendor exit code alone is never removal; 1605/1614 require a complete inventory, 1602 is cancellation, 3010/1641 restart, incomplete inventory stays remaining | Desktop batch/single journeys plus `tests/security.rs` and `uninstall.rs` tests | **PASS (fixtures), UNVERIFIED (real MSI/Store)** |
| Crash handling / fuzzing | No fuzz harness or crash-injection pass was run | — | **UNVERIFIED** |
| Installer downgrade/upgrade and recovery across versions | Same-version NSIS install/repair/reinstall/uninstall verified; no version-to-version package or downgrade was built | [installer receipt](evidence/2026-11-17-installer-release.md) | **PASS (same version), UNVERIFIED (cross-version)** |

## Final PASS / FAIL / UNVERIFIED feature matrix

Direct evidence is named in each row. “PASS (fixtures)” means the real operation was executed
against disposable native fixtures, not that a real user's data was modified.

| Area | Status | Direct evidence / limitation |
|---|---|---|
| Application inventory, search, filter, sort and selection | **PASS** | Four disposable HKCU entries plus this machine's real inventory; desktop journey 2 |
| Single and batch uninstall of disposable apps | **PASS** | Real fixture uninstallers executed; registry read-back; `1 still registered`, `1 cancelled`, `0 failed`; desktop journeys 3–4 |
| Real vendor/MSI uninstall with leftover reconciliation | **UNVERIFIED** | No real vendor installer or signed third-party MSI was run |
| Microsoft Store/MSIX inventory | **PASS** | Read-only current-user `PackageManager` inventory; real desktop inventory loaded |
| Microsoft Store/MSIX removal | **UNVERIFIED** | `RemovePackageAsync` plan path is implemented; no real package was removed |
| Broken-entry removal and registry restore | **PASS** | Ghost HKCU key removed after backup and restored (1 key / 4 values); desktop journey 5 |
| Leftover false positives at scale | **UNVERIFIED** | Name-only matches stay caution/unselected; no large real-world sample or competitor comparison was measured |
| Protected-path and reparse-point refusal | **PASS** | Security suite, cleanup/analyzer fixtures and provider validation |
| Process termination safety | **PASS** | Critical-process deny list, path/creation-time binding and held-handle PID protection; real third-party termination not performed |
| Cleaner scan, preview, cancellation, execution, restore and purge | **PASS (fixtures)** | `CLEANUP_FIXTURE_OK`; Rust round trips; no real user cache was moved |
| Analyzer recycle and permanent-delete plans | **PASS (fixtures)** | Analyzer action suite; an earlier receipt exercised the real Recycle Bin, not repeated here |
| Treemap, duplicates, storage categories (calculations) | **PASS** | Rust/frontend tests; desktop treemap drill and duplicate inspection remain **UNVERIFIED** |
| Developer/AI project cleanup | **PASS (fixtures)** | Disposable project round trip; no real project was cleaned |
| Performance diagnostics and recommendations | **PASS (read-only)** | Live snapshot, thresholds and recommendation tests; native Performance review receipt |
| Elevated boot impact and WPR trace | **UNVERIFIED** | Requires an administrator session; unelevated UI correctly disables them |
| Startup inventory and reversible toggle | **PASS** | Native UIA round trip wrote `0x03` then `0x02`; Run value preserved. Automated native `ask` dialog under WebDriver remains **UNVERIFIED** |
| Hunter Mode | **PASS** | Confirmation, targeting, Escape and 490 µs median backend targeting; desktop journey 10 |
| Explorer shell verbs | **PASS (registration/unit)** | HKCU registration and plan validation; real right-click in Explorer **UNVERIFIED** |
| Settings persistence | **PASS** | Theme survived a real WebView reload; startup section functional |
| Keyboard, focus, reduced motion | **PASS** | Desktop/UIA traversal, focus trap and Escape; reduced-motion emulation |
| Narrator/NVDA, forced colors, 200% text | **UNVERIFIED** | No screen reader or forced-colors session was run |
| Window sizes 1000×680, 1320×860, 1440×960 | **PASS** | 0 horizontal document overflow on all six destinations; named captures |
| 125%/150% OS scaling | **UNVERIFIED** | Requires a Windows sign-out; not exercised |
| Cold startup and UI responsiveness during scans | **PASS (responsiveness); UNVERIFIED (cold-start timing)** | Window responds during scans; launch was observed but not timed as a startup benchmark |
| NSIS install / repair / relaunch / uninstall / reinstall | **PASS** | Silent runs: 1.07 s / 1.02 s / 1.24 s; registry entry correct; app data unchanged; [receipt](evidence/2026-11-17-installer-release.md) |
| App data and quarantine preservation across installs | **PASS** | 0 hash changes; root and quarantine sentinels survived install, uninstall and reinstall |
| WiX MSI install | **UNVERIFIED** | Per-machine package returns 1603 / error 1925 without elevation; elevated install not run |
| Version-to-version upgrade and downgrade | **UNVERIFIED** | Only same-version reinstall was exercised |
| Signature verification negative tests | **PASS** | Negative tests reject unsigned/tampered/wrong-subject files; no signed release artifact exists to verify |
| Public certificate trust | **FAIL** | Local certificate chain `UntrustedRoot`; all local artifacts and published `v0.1.0` NSIS `NotSigned` |
| Documentation integrity | **PASS** | `DOCS_CHECK: PASSED` |
| Zero telemetry / application network calls | **PASS** | No `fetch`, XHR, WebSocket or network API in `src/`; no network call in `src-tauri/src/`. Tauri's crate graph links `reqwest` transitively, so “no network stack” is not literally true at the dependency level |
| CI on the tested revision | **PASS at `3d9f2da`; UNVERIFIED for this tree** | Run `43954491263` is green; the working tree is uncommitted, so it has no CI run |
| Competitor comparison | **UNVERIFIED** | Not run; [COMPETITOR_COMPARISON.md](COMPETITOR_COMPARISON.md) is documentation only |

## Packaging and installer results

The current tree produced both installers from `npm run build:unsigned`:

| Artifact | Size | SHA-256 |
|---|---:|---|
| NSIS `Cyron Uninstaller_0.1.0_x64-setup.exe` | 2.17 MiB (2,271,846 B) | `16a378226f6162baa93373830817bb20df65c59e3f9ae589f34b15dbbdec479f` |
| WiX `Cyron Uninstaller_0.1.0_x64_en-US.msi` | 3.18 MiB (3,330,048 B) | `3f1e64a7594fba9df4fbecfb7537bde36175dd11166ac6fc346ebe88b6612630` |
| Standalone `cyron.exe` | 6.60 MiB (6,918,144 B) | `95c3f7effec7ce3ff9a922997f8bc3ccb275ead8df88167d07f7dd0268ca20d4` |

`RELEASE_BUNDLES_OK` and `WINDOWS_METADATA_OK` passed. `verify:signatures` reported all three
artifacts `NotSigned` and exited 1; that is the expected result for the unsigned RC build, and it is
a release blocker, not a test to weaken.

The NSIS package is a per-user install under `%LOCALAPPDATA%\Cyron Uninstaller`. It was installed,
repaired, relaunched, uninstalled, reinstalled and uninstalled again. Every step exited 0; the
uninstall registry entry was created and removed correctly; app-data hashes changed 0 entries; and
disposable quarantine sentinels survived. The installed window was captured in
[34-installed-launch.png](evidence/phase8/34-installed-launch.png).

The MSI is per-machine. Unelevated execution returned MSI error 1925 and installed nothing.
An elevated MSI install/uninstall was **not** performed.

The published `v0.1.0` release remains a separate, older artifact set: its NSIS hash matches its
`SHA256SUMS.txt`, but its Authenticode status is `NotSigned`.

## Defects found and fixed in this audit

| Defect | Fix | Regression test |
|---|---|---|
| Force Delete canonicalized a junction and could act on its target | Preserve the caller path, refuse reparse targets, bind identity, recheck counts | `force_delete_refuses_a_junction_instead_of_its_target` |
| A same-path replacement could be deleted after the user reviewed a different file | `FileIdentity` on volume/file index/creation/size/attributes, checked again at execution | `force_delete_refuses_a_file_replaced_after_preview` |
| Cross-volume quarantine silently omitted links from a copied tree | Refuse the copy when any link is encountered; source is left intact | `cross_volume_copy_refuses_a_link_instead_of_dropping_it` |
| A forged manifest entry could restore a file from outside its batch | Canonicalize and scope `storedPath` to the batch; refuse relative originals | `restore_refuses_a_manifest_entry_outside_its_batch` |
| A process handle was closed before the graceful wait, allowing PID reuse | Hold a verified process handle through the wait and fallback | `verified_handle_rejects_a_wrong_creation_time` |
| Provider definitions could label protected data `caution` instead of `protected` | Reject any non-`protected` risk on sensitive markers | `rejects_caution_risk_for_protected_data` |
| Historical benchmark document mixed fabricated competitor rows with Cyron runs | Replace with a generated Cyron-only report and a documentation-based comparison | [BENCHMARKS.md](BENCHMARKS.md), [COMPETITOR_COMPARISON.md](COMPETITOR_COMPARISON.md) |
| “No network stack” wording overstated the dependency graph | Documentation now distinguishes no application network calls from Tauri's transitive HTTP crates | `AGENTS.md`, [SECURITY.md](SECURITY.md) |

## Outstanding issues

1. **Public signing trust.** The Chryypt certificate is self-signed. Standard Windows reports an
   untrusted root, and the published `v0.1.0` NSIS asset is `NotSigned`. Do not describe either
   installer as trusted or warning-free. A publicly trusted Authenticode certificate and a signed
   release build are required before a public release.
2. **MSI and version-upgrade coverage.** The MSI needs elevation; only its metadata and refusal
   were verified. A clean-VM install, version-to-version upgrade, downgrade and MSI upgrade
   sequence were not run. The NSIS same-version repair path passed.
3. **Real application removal.** A signed third-party MSI and a vendor uninstaller with leftover
   reconciliation were not run. Store/MSIX removal was not run either.
4. **Elevated diagnostics.** MFT scan timing, boot-impact event-log reading and WPR recording need
   an elevated session; none was measured here.
5. **DPI and accessibility.** 125%/150% scaling requires a Windows sign-out; Narrator, NVDA,
   forced colors and 200% text were not exercised.
6. **Comparative claims.** No competitor was installed. The comparison document is a documented
   capability matrix, not a benchmark or an accuracy study.
7. **Crash injection.** The journal is designed to be crash-consistent, but no power-loss/kill-at-
   each-write test was performed.
8. **Desktop cache execution.** The desktop suite deliberately did not move this machine's real
   caches; fixture evidence is the only execution evidence for cleanup and project removal.
9. **CI on this tree.** The last committed revision is green; the Phase 8 tree is uncommitted and
   has no CI result. Do not publish from a tree with red or absent CI.

## Reproduction commands

```powershell
npm run typecheck
npm test
npm run docs:check
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/verify-cleanup.mjs
node scripts/verify-e2e.mjs
node scripts/verify-hunter-perf.mjs
npm run build:desktop
node scripts/verify-desktop-binary.mjs
npm run test:desktop
CYRON_BENCH_RUNS=3 node scripts/benchmark-competitive.mjs
npm run build:unsigned
node scripts/verify-release-bundles.mjs
npm run verify:metadata
npm run verify:signatures
```

Raw desktop results and the tested revision are committed under
[phase8/](evidence/phase8/); the installer run is recorded in
[the installer receipt](evidence/2026-11-17-installer-release.md).
