# Cyron — Active Handoff

Updated: 2026-11-26 (v0.2.4 published with the System health scanner for dead startup and uninstall entries; earlier: v0.2.3 published with the native read-only search index, parallel index build, faster name queries, multi-select and CSV/TXT export; earlier: v0.2.2 published with the in-app updater and read-only Everything 1.4.1+ index search; earlier: 0.2.1 packaging metadata release: Chryypt publisher in every Windows artifact, branded NSIS installer/uninstaller icon, homepage ARP links, metadata gate hardened; earlier: native desktop verification of the spike capture and driver evidence; v0.2.0-rc.2 published as an unsigned GitHub prerelease after a green release workflow; kernel/driver performance diagnostics and batch-eligibility fix; v0.2.0-rc.1 unsigned prerelease; 0.2.0 version bump, release notes and NSIS install/launch/uninstall/reinstall verification; Phase 6-8 feature commits and audit; Phase 7 real desktop E2E, Phase 6 workflow UX, Phase 5, Phase 4, Phase 3, Phase 2 and Phase 1 records preserved)

## 1. Current Verified State

- **Website, README and screenshots refresh (2026-11-27):** the landing page in `landing/` was rebuilt around the redesigned app: theme-matched light/dark preview images (`landing/assets/ui/*.webp`), a simulated live monitor demo, a pinned horizontal screenshot tour with a lightbox, a word-by-word manifesto, a scroll-tilted hero, nav section tracking and a five-step showcase (Applications, Search, Disk space, Caches, Performance). README, USER_GUIDE and DESIGN describe the Monitor/Processes tabs and the nested Disk space map. `docs/screenshots/` now holds browser previews with sample data from `npm run preview:screenshots` (`scripts/capture-ui-preview.mjs` + `scripts/preview/mock-tauri.js`), replacing the 0.2.0-rc.2 native captures. `npm run landing:check` passes; the page was checked in Chromium at 1440×900 and 390×844 in light and dark. Not yet seen in Safari or Firefox.

- **Branch:** `main`. Phase 5 is committed as `2eeb91f` with documentation follow-up `3d9f2da`. The Phase 8 safety hardening is `18cd4df`, the Phase 7 desktop E2E harness is `bd0de9d`, and the Phase 6-8 audit/evidence is `6c8defa`; release prep is `870d753` and the checksum-workflow fix is `ca51baf`. The 0.2.2 release is commit `0961737`, tagged `v0.2.2`. The 0.2.3 release is commit `8561127`, tagged `v0.2.3`, pushed to origin and published on 2026-11-26 after release workflow `45700921319` passed. The 0.2.4 release is commit `b8981bb`, tagged `v0.2.4`, pushed to origin and published on 2026-11-26 after release workflow `45862203481` passed. No rebase was made.
- **Phase 8 verdict:** **NOT PRODUCTION READY — production release remains blocked.** The unsigned 0.2.0-rc.2 testing prerelease is published; the trust and native-path blockers remain open. The full audit, evidence and blockers are in [docs/RELEASE_AUDIT_2026-11-17.md](docs/RELEASE_AUDIT_2026-11-17.md). The NSIS installer path is verified end to end; signing trust and real vendor/MSI/Store removal are the remaining release blockers.
- **Version:** `0.2.4` is the latest published release, tagged `v0.2.4` at commit `b8981bb` and published on 2026-11-26 after release workflow `45862203481` passed ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.4)). Its release notes are [docs/RELEASE_NOTES_v0.2.4.md](docs/RELEASE_NOTES_v0.2.4.md). The published `SHA256SUMS.txt` verified the setup, MSI, executable and manifest, and the live `latest.json` advertises only `windows-x86_64-nsis` at `0.2.4`. Existing 0.2.3 installations can update from **Settings → About → Software updates**; that specific update path has not yet been exercised. `0.2.3` was the previous release, tagged `v0.2.3` at commit `8561127` and published on 2026-11-26 after release workflow `45700921319` passed ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.3)). Its release notes are [docs/RELEASE_NOTES_v0.2.3.md](docs/RELEASE_NOTES_v0.2.3.md). The published `SHA256SUMS.txt` verified the setup, MSI, executable and manifest, and the live `latest.json` advertises only `windows-x86_64-nsis` at `0.2.3`. Existing 0.2.2 installations can update from **Settings → About → Software updates**; a real 0.2.2 → 0.2.3 in-app update passed on 2026-11-26 (G133). `0.2.2` was the previous release, tagged `v0.2.2` at commit `0961737` and published on 2026-11-25 after release workflow `45452657763` passed ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.2)). It adds the in-app updater and the first read-only Search destination, which used a local Everything IPC index. 0.2.3 replaces that IPC client with Cyron's own native index, parallelizes the build, speeds up broad name queries, and adds multi-select plus CSV/TXT export (see below). The published `SHA256SUMS.txt` verified all four hashed assets, and the live `latest.json` advertises only `windows-x86_64-nsis`. Existing 0.2.1 installations must install 0.2.2 manually once, because 0.2.1 predates the updater; later releases can use the updater. A fresh 0.2.1 installation upgraded with the published 0.2.2 setup and then updated to 0.2.3 in-app on 2026-11-26 (G134); a 0.2.3 → 0.2.2 downgrade and the 0.2.3 restore also kept history and quarantine intact (G135). `v0.2.1` was published on 2026-11-21 after release workflow `44860805315` passed ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.1)); the downloaded checksum file verified all three assets, and the published setup reports `Chryypt` with the Cyron icon. `v0.2.0` remains the previous stable release. `v0.2.0-rc.2` was published on 2026-11-18 as an unsigned GitHub prerelease ([release](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.0-rc.2)) after release workflow `44157576637` passed (10 m 37 s). Every published artifact remains `NotSigned`, so none is a publicly trusted production release. The previous `v0.1.0` assets remain `NotSigned` and predate this tree.
- **IPC surface:** 81 commands registered in `src-tauri/src/lib.rs` (including Phase 2
  `list_app_history`, `prepare_store_uninstall`, `execute_store_uninstall`, Phase 3
  `scan_developer_storage`, `preview_developer_cleanup`, `verify_developer_cleanup`, Phase 4
  `capture_performance_snapshot`, `sample_live_performance`, `list_performance_baselines`, `record_performance_baseline`,
  `start_performance_trace`, `stop_performance_trace`, `open_performance_settings`, and the
  native search index commands `search_index_status`, `build_search_index`,
  `cancel_search_index_build`, `query_search_index`, plus `export_search_results`, and the
  dead-entry commands `scan_dead_entries`, `prepare_dead_startup_removal`,
  `execute_dead_startup_removal`), documented in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), and wrapped in `src/lib/api.ts`. The current surface
  is checked against its handlers by the verification commands below.
- **Native search index (2026-11-26):** `searchindex.rs` replaces the Everything IPC client with
  Cyron's own read-only index over fixed local drives. The build walks with `fs.rs`, stores names,
  parent links, sizes, created/accessed/modified times and raw attributes in compact name pools,
  skips reparse traversal, and writes only `<app_data>\search-index.bin` (atomic temp-file publish,
  versioned header). Startup loads the cache in the background; one recursive
  `ReadDirectoryChangesW` watcher per volume applies add/remove/rename/metadata events in memory.
  Rebuilds use a separate index and swap it in, so the previous index stays searchable. The query
  parser implements AND/OR/NOT, phrases, wildcards, regex, `ext:`, `file:`/`folder:`, `size:`,
  `dm:`/`dc:`/`da:`, `attrib:`, `type:`/kind macros, `path:`, `dupe:` and `empty:`; results are
  filtered, sorted and paginated server-side. Search is always read-only and is not gated by
  `read_only`. The build now enumerates directories on up to four workers, keeps a separate name
  order so the common name query never re-sorts a whole result set, and uses top-k selection before
  sorting a page. Measured in release on 2026-11-26 against the recorded pre-change baseline: a
  20,201-entry fixture build fell from 355 ms to 105 ms, and a 500,000-entry name query from 21 ms to
  7 ms (size-sorted 15 ms, rare-name 4 ms); the benchmark's ceilings fail the old numbers.
  Results gained multi-select (checkbox, ctrl/cmd-click, shift-click, Space, Shift+Arrow, Ctrl+A,
  Escape) with a count/size bar for copying or exporting the checked rows, and CSV/TXT export for
  either the checked rows or the full sorted result set through the native save dialog, capped at
  1,000,000 rows. Verified by 18 engine/export unit tests (real fixture walk, persistence round trip,
  parser and filters, sort/pagination, rename/delete, corrupt cache, cancellation, watcher
  notifications and CSV/TXT validation), the Rust suite and 127 frontend tests. A full fixed-drive
  build, the native desktop Search click-through and live watcher behavior against a real drive
  remain **UNVERIFIED**.
- **Dead startup & uninstall scan (2026-11-26):** `health.rs` adds a read-only scan over every
  accessible Run/RunOnce hive and Win32 uninstall registration. A startup entry is dead only when
  its registry value still exists but the resolved executable (after `%VAR%` expansion) is gone;
  Startup-folder shortcuts and bare PATH names are deliberately excluded. An uninstall registration
  is dead only when `verify_live_broken_status` confirms both its uninstaller and install folder are
  missing; system components and protected roots are excluded. Removal is review-first and reuses
  the existing engines: dead startup values are removed through a 5-minute single-use
  `DeadStartupPlan` that writes and verifies a single-value `.reg` backup, a durable journal entry
  and a quarantine manifest, keeps the value recoverable through `restore_registry_backup`, and
  refuses a protected, changed or reappeared entry; dead uninstall registrations reuse
  `prepare_broken_inventory_cleanup`/`execute_broken_inventory_cleanup` with its live fingerprint
  check and recursive backup. The UI is a third **System health** section in Storage & Cleanup's
  Cleanup page with per-entry reasons, elevation/protection badges and a confirmation dialog;
  removal is disabled while read-only mode is on. Rust tests cover a live scan and a real HKCU
  remove-and-restore fixture. A native desktop click-through of the tab remains **UNVERIFIED**.
- **In-app updater (2026-11-25):** `tauri-plugin-updater` and `tauri-plugin-process` are registered; Settings → About exposes a user-initiated check against `https://github.com/Ry0x7/cyron-uninstaller/releases/latest/download/latest.json`. No background polling or telemetry is added. Release builds use `src-tauri/tauri.release.conf.json` (`createUpdaterArtifacts`) with `TAURI_SIGNING_PRIVATE_KEY`; the NSIS setup and `.sig` are produced, and `scripts/generate-updater-manifest.mjs` writes `latest.json` under `windows-x86_64-nsis` only. Settings enables the check only for an NSIS bundle; MSI installations are directed to the release page rather than offered the per-user NSIS package. `npm run verify:updater` passes and the published feed was downloaded and verified. The updater private key is at `.tauri/cyron-updater.key` locally (gitignored) and is stored as the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret; losing it prevents future updates for 0.2.2+ installations. The public key is embedded in `tauri.conf.json`. The 0.2.2 → 0.2.3 update was exercised in place on 2026-11-26 (G133); a fresh 0.2.1 installation then upgraded with the published 0.2.2 setup and updated to 0.2.3 in-app (G134). The 0.2.3 → 0.2.4 in-app update has not yet been exercised.
- **Code signing:** the published `v0.1.0` NSIS installer, the rc.2 assets, the v0.2.2 and v0.2.3
  installer/MSI/standalone executables and the local release artifacts are `NotSigned`. The v0.2.2
  and v0.2.3 update packages carry a Tauri updater signature, which is separate from Authenticode trust. The `CN=Chryypt Organization` certificate is self-signed (Subject == Issuer), so a
  locally signed build is cryptographically intact but not publicly trusted and earns no SmartScreen
  reputation. See [docs/CODE_SIGNING_POLICY.md](docs/CODE_SIGNING_POLICY.md).
- **CI:** the rerun of `main` run `45700921629` is green for `8561127`: typecheck, 127 Vitest tests, build, `docs:check` and the Rust suite. Its first attempt failed only `phase3_project::next_cache_cleanup_leaves_the_rest_of_the_build_and_source` at `assert!(report.quarantined > 0)`; the same suite passed in release run `45700921319`, and rerunning the failed Rust job passed, so it is recorded as a transient Windows cleanup-timing flake rather than a reproducible regression. Release run `45862203481` is green for `b8981bb` and published the 0.2.4 assets after frontend, Rust, security, cleanup-stress, fixture, bundle, metadata and updater gates; `main` CI run `45862197461` is also green for `b8981bb`. Release run `45700921319` is green for `8561127` and published the 0.2.3 assets after frontend, Rust, security, cleanup-stress, fixture, bundle, metadata and updater gates. The current local tree has 127 Vitest tests in 17 files (7 updater-wrapper tests, 4 selection-helper tests and 3 toolbar render tests) and 287 Rust tests passed with 5 ignored (the manual search benchmark is one) after the native search index, selection, export and dead-entry work. Release run `45452657763` is green for `0961737` and published the 0.2.2 assets after frontend, Rust, security, cleanup-stress, fixture, bundle, metadata and updater gates. The separate `main` CI run `45452657552` failed only `phase3_project::disposable_project_cleanup_is_scoped_and_restorable` while the same test passed in the release run; that test now selects the sole scoped project rather than a transient package name. Release run `44860805315` is green and published the 0.2.1 assets after running the full Rust, security, cleanup-stress, fixture, bundle and metadata gates, including the new publisher and installer-icon assertions. Earlier, release run `44157576637` published the rc.2 assets. The earlier run `43200921453` failed only `docs:check` because `HANDOFF.md` linked into gitignored `.unlazy/`; that link now points to the repository-local
  [capability verification receipt](docs/evidence/2026-11-09-capability-verification.md), and
  `scripts/verify-docs.mjs` rejects links into gitignored directories.
- **Release bundles:** the published 0.2.4 assets were downloaded and verified against `SHA256SUMS.txt`: NSIS `3,569,950` bytes / `7fd44518700607442833cd3eaf8d7f2ba9560f0508eda365b2edf9d9541be8b3`; MSI `5,013,504` bytes / `0964eb4cc35f366ea829203fabcd0781cba96e1c556de08b96c447ae79a49d23`; `cyron.exe` `10,628,096` bytes / `672fed5d7a5095ba58f8b0334fa32af2759fede532fd965589341800066ededd`; `latest.json` `3,418` bytes / `b0aeb1cdad7d5ccd3ff43915ed0fe5f230adc66e9b730fc64a2af6fa86fd67fb`; all four checksums verified. The downloaded executable reports `FileVersion 0.2.4` / `ProductVersion 0.2.4` with `CompanyName=Chryypt`, and all three Windows binaries are `NotSigned`. The previously published 0.2.3 assets were downloaded and verified against `SHA256SUMS.txt`: NSIS `3,540,111` bytes / `e01193f0e292aa23fea1dd5cfac39faf9c7adb9d752994d1bbd2a96ca0196d06`; MSI `4,964,352` bytes / `7976927e10c6504d8ef97a7bec0af53a97103b9828ad0ff81daac5037d1b42fb`; `cyron.exe` `10,511,360` bytes / `aa34e6789657fd02944b91b47308b0c964010477891f9f9c91b1175ee73ffae8`; `latest.json` `3,594` bytes / `f99ea610a693160c2c2e364310af6c1eaf223b4d4c5ea665b8fd2ed1d2f5a0a2`; all four checksums verified. The earlier 0.2.2 `npm run build:release` produced NSIS `3,149,936` bytes / `fa004eaff621ad58573825c47742a84622115c3b4b0fe18374213c84e610231f`, MSI `4,395,008` bytes / `e26ca37b18ba17eea256307873368764ce633fd753d38c5ebfc9d592e56dda9f`, and `cyron.exe` `9,129,472` bytes / `17853d79acf435bcf79216e1a09872448f48dd3544e90fe9cd5eb1b67f4cd061`; the updater manifest and NSIS `.sig` agree (`UPDATER_ARTIFACTS_OK`). The published CI assets were downloaded and verified against `SHA256SUMS.txt`: NSIS `3,155,157` bytes / `e5af191698dc516525f5e77fde18b73f5cc96b163f4627d2d596ff1418957dc5`; MSI `4,395,008` bytes / `b87771c07e1199bc8557496c85ac426c041b36e2eb473720d1afb365276f79eb`; `cyron.exe` `9,106,432` bytes / `1f5b624a83f4187edc7ffe0562f8c20e2f8619ef6b41b2aa87ffb3207f2a48c5`; `latest.json` `3,571` bytes / `e1bf05ebfe24241a3982cc77875755e1f668525e6cfcffd097c211dc5eed267c`; all three executables report `NotSigned`. Earlier, `npm run build:unsigned` produced NSIS `2,270,729` bytes / `705b6378…e616`, MSI `3,260,416` bytes / `313a089f…fe57`, and `cyron.exe` `6,918,144` bytes / `ca78b575…ebfa`. Metadata audit passed; signature verification failed because all three artifacts are unsigned. The NSIS package installed (1.56 s), launched (37.1 MB, exit 0), uninstalled (1.25 s) and reinstalled; the 60,024-file app-data tree was unchanged. See [the 0.2.0 receipt](docs/evidence/2026-11-18-release-0.2.0.md). The rc.1 prerelease was rebuilt by CI from tag `v0.2.0-rc.1`; its downloaded `SHA256SUMS.txt` verified all three assets. Post-release commit `ca51baf` fixes checksum naming and line endings. The rc.2 candidate was rebuilt by CI from `e25e45c`: NSIS `2,296,767` bytes, MSI `3,289,088` bytes and `cyron.exe` `6,951,424` bytes were downloaded, the checksum file verified all three, and every binary is `NotSigned`. See [the rc.2 receipt](docs/evidence/2026-11-18-release-0.2.0-rc2.md).

## 1a. 0.2.1 packaging metadata release — 2026-11-21

`0.2.1` is a packaging release; no Rust or frontend behavior changed. It fixes the incomplete Windows metadata and branding shown in Explorer, Add/Remove Programs and the installers themselves.

- **Publisher:** `bundle.publisher` was already `Chryypt`, so the NSIS uninstall registry `Publisher` value and the MSI `Manufacturer` were already correct. Tauri's NSIS template does not emit a `CompanyName` version key, so the setup executable's Windows file properties showed an empty publisher. [src-tauri/nsis/installer-hooks.nsh](src-tauri/nsis/installer-hooks.nsh) now adds `CompanyName`, `InternalName` and `OriginalFilename`; NSIS reuses the same version resource for the generated uninstaller. A negative control confirmed the pre-fix 0.2.0 setup reports `CompanyName=''` while 0.2.1 reports `Chryypt`.
- **Icon:** `bundle.windows.nsis.uninstallerIcon` now points at `icons/icon.ico` (the installer icon already did), so the uninstaller no longer renders the generic NSIS icon. The MSI `ProductIcon` embedded the same 25,817-byte `.ico`.
- **Links:** `bundle.homepage` is set, so NSIS writes `URLInfoAbout`, `URLUpdateInfo` and `HelpLink`, and the MSI carries `ARPURLINFOABOUT`, `ARPHELPLINK` and `ARPURLUPDATEINFO`.
- **Gate:** `scripts/verify-windows-metadata.ps1` now fails when the NSIS `CompanyName`, `InternalName` or `OriginalFilename` are missing or wrong, when the installer icon lacks 16/32/48/256 sizes, when the setup icon differs from the application icon, or when the MSI ARP author, icon or links regress.
- **Artifacts (local tree):** NSIS `2,282,544` bytes / `91f19e242864d8e3e32b39bfe82833e16aeab8c3df02613cc1781756d7589123`; MSI `3,284,992` bytes / `38fb62be98035b2b1f7be641c2993aa97edc3a10d5a257f9e30b3c7df481a80c`; `cyron.exe` `6,964,736` bytes / `f34c34533552a0fa598a5101b20698cbbc2fe10bd732f9dd6bffd974ce886a43`. `WINDOWS_METADATA_OK` and `RELEASE_BUNDLES_OK`.
- **Published artifacts (CI build, downloaded and verified):** NSIS `2,288,707` bytes / `42d5926718fd8c2d28281bcfc64389966b52be4d6d48e13339b4931a17c2945e`; MSI `3,289,088` bytes / `587eb966499e761ac3aaf811ef2afced1dc51dc6f1a5e678a3f4a21082832d85`; `cyron.exe` `6,951,424` bytes / `49f94b1b5abb0d363b058dfee4f5b258506a00d3f341955e45e2315aae06e8f1`. The published setup reports `CompanyName=Chryypt`, `InternalName=CyronUninstallerSetup`, `OriginalFilename=CyronUninstallerSetup.exe`, and its icon pixels match the published application icon.
- **Installed entry:** the existing local 0.2.0 installation reports `Publisher = Chryypt` in HKCU Add/Remove Programs. The 0.2.1 uninstaller icon was verified from the generated NSIS script (`MUI_UNICON = icons/icon.ico`) and the matching setup/application icon, not by reinstalling over the local 0.2.0 installation.

## 2. Phase 2 implementation state — 2026-11-16

This section records the Phase 2 implementation and verification state on 2026-11-16.

- **Store/MSIX:** `store.rs` now reads current-user main packages through `PackageManager` and removes one reviewed package through `RemovePackageAsync(...).join()`. It never calls `RemoveForAllUsers`, deprovisions, elevates automatically or launches a shell. Access-denied, in-use and not-found outcomes are classified. Plans bind full name, family name and publisher ID; execution revalidates all three and checks the live inventory afterward.
- **Lifecycle history:** `app_history.rs` persists `<app_data_dir>/app-history.json` atomically, newest activity first, capped at 2,000 records. A partial inventory can refresh current apps but can never mark a missing app removed. Records are descriptive and never authorize cleanup.
- **Completion semantics:** `uninstall.rs` classifies 1605/1614 as not-installed, 1602 as cancelled, 3010/1641 as restart-required, and never treats exit 0 alone as removal. A bounded post-exit inventory poll reconciles single and batch runs; incomplete inventory or remaining registration stays `remaining`.
- **Discovery accuracy:** `apps.rs` detects validated MSI product codes in commands or GUID-named uninstall subkeys and reads PE `ProductName`/`CompanyName`/`FileVersion` metadata for portable executables.
- **Remnant evidence:** name-only file, shortcut and registry matches are `caution` and unselected; exact install-location or exact registration identity is the only `safe` preselection. An incomplete inventory blocks removed-app classification, and history records never authorize cleanup.
- **Quarantine:** cleanup and uninstaller-residual moves use `MoveFileExW` with `MOVEFILE_COPY_ALLOWED` for cross-volume sources and without `MOVEFILE_REPLACE_EXISTING`, so existing destinations are never overwritten. A failed move removes its write-ahead journal entry and leaves the source in place.
- **Removed-app history and explicit scans:** `list_removed_apps` merges retained identity snapshots with persistent removal history, and `scan_named_remnants` reviews apps removed before Cyron using an explicit name, publisher or path. Every explicit result is caution and unselected. The Cleaner lists persistent history and provides the explicit form.
- **UI:** the uninstall wizard has a Store mode that reviews the exact package identity before invoking Windows, reports permission denial truthfully, and offers Settings/winget fallback. Store rows and the app drawer open that mode. The UI path is typechecked but has not been driven in a native desktop session.

UNVERIFIED in this tree: a real Store package removal, a real MSI uninstall with 1605/1614 reconciliation, and the Store wizard/history UI in a native desktop session. Run the Phase 2 gates in `scripts/verify-phase2.mjs`; `--live-store`, `--live-msi` and `--manual-ui` deliberately fail with UNVERIFIED until those checks are actually performed. The Phase 2 verification run is recorded in `GATES.md` G67–G73; the Phase 1 table below remains historical.

## 2b. Phase 3 implementation state — 2026-11-16

Phase 3 adds an evidence-based developer storage path without changing the review-first cleanup engine or the no-rebuild/no-force policy.

- **Project scanner:** `devscan.rs` recognises projects from real manifests, Git roots and package/Cargo/Python framework metadata. It scans explicit folders plus bounded automatic candidates on selected fixed/removable drives, skips reparse points and dependency/build trees, and caps roots, directories, projects and artifacts.
- **Artifacts:** only named regenerable folders become candidates: `node_modules`, `dist`, `build`, Rust/Maven `target`, Gradle `build`/`.gradle`, `.next/cache`, `.turbo`, `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`. Each record has exact path/ownership evidence, confidence (a lockfile raises `node_modules` confidence), logical/allocated bytes, counts, last write, activity, rebuild command and loss copy. Source, `.git`, `.env`, credentials, keys, databases and model weights are reported as protected; they never enter a plan.
- **Id-only preview:** `scan_developer_storage` keeps a 1-hour server-held scan. `preview_developer_cleanup` resolves selected artifact ids, refuses unknown/protected artifacts and any path outside its recorded project, then reuses `guard_path`, snapshot binding, journaled quarantine and the single-use `CleanupPlan`.
- **Verification:** `verify_developer_cleanup` re-measures selected artifacts, reports removed/remaining bytes and files, and lists quarantine batches that still hold matching targets. The Projects tab re-scans after a run and shows the result; it does not claim success from exit status alone.
- **UI:** Cleaner has a **Projects** tab with project/activity filtering, per-artifact evidence and loss/rebuild details, protected components, selective checkboxes, exact-target preview, confirmation, result items and verification. Project roots are persisted in `cyron.settings.v1`. Canonical `\\?\` prefixes are hidden from display only, and opening Quarantine refreshes it so a just-finished run cannot look empty.
- **Catalog safety corrections:** Gemini CLI `tmp` was reclassified `protected` because its chats, shell history and checkpoints live there; Cursor and VS Code `workspaceStorage` were reclassified `protected` because they contain chat/extension state; LM Studio current paths, Hugging Face, Antigravity agent data and Freebuff settings were added as protected. Tool cache rows now carry `installState`; only `not-detected` is shown as orphaned and nothing is auto-selected.

### Phase 3 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 98 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (514.16 kB JS, 101.42 kB CSS; Vite chunk-size warning only) |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 244 passed, 4 ignored, 0 failed |
| Phase 3 disposable project | `cargo test --test phase3_project` | 3 passed: scoped quarantine and restore; `.next/cache` cleaned without touching `.next/server` or `BUILD_ID`; unknown artifact refused |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 76 files |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| RC1 E2E fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK`; all 8 scenarios, after raising the disposable GUI window and polling the hit-test |
| Native Projects workflow | UI Automation against the Tauri debug app (Vite dev server) | PASS: discovered a disposable project, previewed 1 file, quarantined it, verified `1 removed / 0 remaining`, restored it, and found a second disposable project on E:`\Projects`. Source, `.env`, `.git` and `dist` stayed. Screenshots were reviewed locally; raw captures are not committed per [docs/evidence/README.md](docs/evidence/README.md). |
| Native multi-drive discovery | UI Automation against the Tauri debug app (Vite dev server) | PASS: 2 project roots (`Documents\GitHub` on C: and `E:\Projects`) and the E: disposable project appeared in the running window. |

The native run found two UI defects that are now fixed: canonical paths displayed as `\\?\C:\...` (display-only stripping; operations keep the canonical path) and Quarantine showing a stale "empty" after a Projects cleanup (the tab now refreshes on open). The fixtures were removed after the run. UNVERIFIED remains: live `hf cache prune`/`cargo clean`/package-manager commands are guidance only and are never executed by Cyron.

## 2c. Performance diagnostics implementation state — 2026-11-17

Performance diagnostics are read-only sampling plus one existing reversible startup path; no system tuning was added.

- **Metrics:** `perf.rs` measures a 700 ms system-CPU window, memory and commit, PDH physical-disk busy/queue, fixed-volume capacity, and bounded per-process CPU/working-set/read/write counters. Processes whose handles cannot be opened are counted and omitted, not guessed. Cyron's own process and protected/system processes are labeled.
- **Startup and tasks:** HKCU/HKLM Run/RunOnce, WOW6432Node and Startup folders are listed with exact command, location, state, live usage and protected flags. Boot/logon scheduled tasks are read-only and include author, trigger, delay and Microsoft-task provenance. Per-app boot cost is read only from the administrator-only Diagnostics-Performance event log and matched by exact executable path; otherwise it is **Unavailable**.
- **Reversible toggle:** the page calls the existing `set_startup_item_enabled`, which writes `StartupApproved` 0x02/0x03, preserves the Run registration and refuses protected or machine-wide entries when unelevated. A native Tauri dialog now confirms the exact entry and location. This replaced `window.confirm`, which produced no visible dialog in WebView2.
- **Recommendations:** every recommendation requires a measured threshold and carries evidence, expected effect, risk and verification. Per-process I/O is not treated as disk pressure because Windows includes network, pipe and device transfers; the native run exposed that exact false positive and the gate is now physical-disk busy + queue only.
- **Guides, baselines and trace:** five symptom guides filter recommendations; up to 50 labeled local baselines compare CPU, memory, available memory, commit, disk busy and recorded boot time with an association-not-causation note; the optional WPR trace invokes only `%SystemRoot%\System32\wpr.exe` with fixed arguments, requires administrator rights, stays local and never self-elevates.
- **Freshness:** manual **Refresh** calls `capture_performance_snapshot(forceStartup = true)` to bypass the 120-second startup-evidence cache; live sampling keeps the cache.
- **Short-spike capture:** **Record 20 seconds** takes 20 one-second samples through the same read-only command, keeps the peak CPU, kernel, memory and physical-disk values plus the process at the CPU peak, and labels the burst process-, kernel-, disk-, mixed- or idle-dominated. It is cancellable, writes nothing to disk and stops when the page is left. `spikeAnalysis.test.ts` covers every verdict.
- **Kernel and driver evidence:** the snapshot adds display, storage and network driver names, versions and package dates from the class registry, and classified driver/device/storage/power/service errors from the System log over the last seven days. `kernel >= 20%` and `>= 60%` of busy CPU produces a `system` recommendation that states the kernel share and the application CPU it does not explain. It never names a driver as the cause: the counters do not attribute DPC/ISR time. A denied event log is reported as unavailable, not empty.

### Performance diagnostics verification (this tree)

| Check | Command | Observed |
|---|---|---|
| Targeted unit tests | `cargo test --lib perf::tests` | 10 passed, including the idle-disk/high-process-I/O regression, kernel-share gating, driver/event normalization and fixed System32 path validation |
| Live snapshot | `cargo test --test performance -- --nocapture` | passed on this machine; asserts the kernel/driver and recent-event fields |
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 112 passed in 14 files (includes `spikeAnalysis.test.ts`) |
| Full Rust suite | `cargo test --manifest-path src-tauri/Cargo.toml` | 255 passed, 4 ignored, 0 failed |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED` |
| Native Performance workflow | UIA against the native Tauri debug app | PASS on 2026-11-17: 702 ms sample / 179 processes, 15% CPU, 37% memory, 1.1% disk busy; guide switching; 23 startup entries; disposable startup Cancel/Disable/Enable round trip with 0x03→0x02 bytes; two baselines compared; trace correctly disabled unelevated. See [the receipt](docs/evidence/2026-11-17-performance-diagnostics.md). |
| Native desktop diagnostics | WebdriverIO against the compiled Tauri/WebView2 app | PASS on 2026-11-18: 11/11 journeys in 20 m 08 s (`DESKTOP_E2E_OK`); the new journey took 55.6 s. 4 live drivers rendered and the overview count matched; 0 recent System events reported as such; the 20-reading capture showed peak CPU 42%, kernel 14%, disk 0.4%, `brave.exe` and a mixed verdict; no horizontal overflow. See [the receipt](docs/evidence/2026-11-18-desktop-performance.md), [driver evidence](docs/evidence/2026-11-18-native/34-kernel-driver-evidence.png) and [spike result](docs/evidence/2026-11-18-native/35-spike-capture.png). |

The native run found and fixed two defects: the manual Refresh was served stale startup evidence for two minutes, and a disk recommendation fired from process I/O while the physical disk was idle. The 2026-11-18 desktop run then drove the new controls end to end: the driver count matched the rendered inventory, the System-event panel reported its provenance, the sixth guide answered correctly, and a 20-reading capture rendered its peak CPU/kernel/disk/process summary. UNVERIFIED remains: an elevated Diagnostics-Performance boot-impact reading, a real WPR trace recording/opening, a deterministic early-stop assertion, and a live kernel-dominant spike.

## 2d. Design-system and navigation rebuild — 2026-11-17

Phase 5 replaces the old dashboard surface with a calmer Windows-native utility shell. Backend safety plans, quarantine and IPC payloads are unchanged; this is presentation and information architecture only.

- **Navigation:** exactly five destinations — Home, Applications, Storage & Cleanup, Performance, History & Recovery. Settings sits in the sidebar footer. Hunter Mode is **Find by window** inside Applications; there is no separate Hunter page. Storage & Cleanup contains Cleanup and Disk space; History & Recovery contains Activity and Recovery.
- **Design system:** `src/styles/tokens.css` now defines one neutral family, one blue accent, one type hierarchy, one radius scale and light/dark surfaces. `src/components/ui.tsx` provides the shared primitives (`ViewHeader`, `SectionHeader`, `PageTabs`, `Panel`, `Stat`, `Badge`, `Button`, `SearchField`, `Segmented`, `Toggle`, `Select`, `Field`, `ProgressBar`, `Skeleton`, `EmptyState`, `Banner`, `Drawer`, `Modal`, `KeyValue`). Feature CSS never defines a second palette.
- **New feature shells:** `src/features/storage/StorageCleanup.tsx` hosts Cleanup and Disk space; `src/features/history/HistoryRecovery.tsx` hosts Activity and Recovery. Existing Cleaner, Analyzer, Uninstaller, Overview and Performance views were restyled into those destinations rather than duplicated.
- **States and responsive behaviour:** loading skeletons match final layout; empty/error/stale/disabled states exist for scans, history, quarantine, projects and performance; tables hide secondary columns at 1000 px; actions and headings never collapse below usable widths.
- **Accessibility:** visible `:focus-visible` rings, 32 px controls, dialog focus trap and Escape return, `aria-current` navigation, labelled icon-only controls, reduced-motion support and a forced-colors block. A `prefers-reduced-motion` emulation and native keyboard traversal were exercised; screen-reader software and forced-colors visuals were not.
- **Native audit fixes:** the Projects tab no longer inherits the Caches & leftovers header or cache warnings; the Recovery log badge now fits its grid at 1000 px; the Activity table hides Type at 1000 px instead of scrolling; the Analyzer review strips `\\?\` from display paths; legacy `Cleanup · 1 locations` titles and `1 files permanently removed` copy were corrected.
- **Evidence:** selected native captures and the full command ledger are in [docs/evidence/2026-11-17-design-system.md](docs/evidence/2026-11-17-design-system.md). Design rules are in [docs/DESIGN.md](docs/DESIGN.md).

### Phase 5 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (562.28 kB JS, 127.63 kB CSS; chunk-size warning only) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 79 Markdown files |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; debug executable built |
| Native navigation and keyboard | WebView2 remote debugging against the debug app | PASS: five destinations; Settings footer; Hunter in Applications; focus trap; Escape return; reduced motion; 0 horizontal overflow at 1000x680 |
| Contrast | WCAG token calculation | PASS: sampled text pairs 4.57:1–16.14:1 |

UNVERIFIED in this tree: screen-reader traversal, forced-colors visuals, 200% OS text scaling and a clean-VM installer run. The screenshots show this machine's real 179-application inventory; no real user cache or project was cleaned.

## 2e. Workflow UX completion — 2026-11-17

Phase 6 applied the Phase 5 system to every workflow and fixed the state and layout gaps found in the production desktop build. No Rust safety plan, quarantine rule or IPC permission changed.

- **Home:** real system, volume, application, cleanup and recent-activity measurements; no health score. At 1000 px the five-item system summary uses two columns with Uptime spanning the spare row instead of leaving a blank cell.
- **Cleaner:** the first measurement now says **Measuring known cache folders on C:, D:, E:…**, disables the redundant scan action while it runs, and shows **Not checked yet** before results. Opening a preview clears the previous preview, shows a spinner, explains that nothing moves, and labels dismissal **Cancel preview**.
- **Storage & Cleanup:** the header now says **Review before cleanup** instead of the internal read-only phrasing.
- **History & Recovery:** at 1000 px the cleanup log keeps each outcome under its operation title instead of hiding failed and partial runs. A stop or failure that already moved files is recorded as **partial**, not completed or a bare cancellation; Activity, Home and the Recovery log render the fourth state.
- **Settings:** the startup section explains that disabling an entry preserves it and can be reversed; Copy now confirms **Copied** for two seconds.
- **Shell:** `html` is fixed so a stray focus or programmatic scroll cannot shift the whole window onto blank space. Each view slot remains the only scroll container.
- **Evidence:** native captures at 1320x860 and 1000x680, light/dark, and the full command ledger are in [docs/evidence/2026-11-17-workflow-ux.md](docs/evidence/2026-11-17-workflow-ux.md).

### Phase 6 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production frontend | `npm run build` | exit 0 (563.72 kB JS, 128.05 kB CSS; chunk-size warning only) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 80 Markdown files |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; debug executable built |
| Native layout probe | WebView2 evaluation at 984 px client width | 0 horizontal overflow on all five destinations and Settings; document scroll height stayed 641 px |
| Native workflow review | WebView2 against the production desktop build | All five destinations, Settings, Hunter dialog, cleanup preview and Recovery log opened; no dead route or placeholder control |

UNVERIFIED in this tree: real cache/project/application mutation, screen-reader traversal, forced-colors visuals, 200% text scaling and a clean-VM installer run. The per-item destructive paths remain covered by fixtures and the Rust suite.

## 2g. Real desktop E2E — 2026-11-17

Phase 7 adds a real Windows desktop acceptance harness. It does not weaken or change any production
safety plan; it drives the compiled Tauri window through WebView2/WebDriver against disposable
HKCU, project and cache fixtures.

- **Harness:** `npm run test:desktop` builds `desktop_e2e_fixture`, starts `tauri-driver` with the
  matching `msedgedriver`, runs WebdriverIO through `@wdio/tauri-service`, restores app data and
  removes fixture keys. Named screenshots, `results.json` and the exact revision are written under
  `docs/evidence/phase7/`; the full run log stays in gitignored `test-results/desktop/run.log`.
- **Journeys:** first launch and every destination; inventory search/filter/sort/select; batch and
  single uninstall; broken-entry registry backup/delete/restore; 1000×680 and 1440×960 light/dark
  overflow checks; Settings theme persistence across reload; Storage disk-space state; Performance
  evidence; startup provenance; History records; Hunter confirmation/targeting/Escape.
- **Real IPC:** no operation result is mocked. Fixture uninstallers are executed, HKCU keys are read
  back independently, and the registry restore is asserted from the Windows registry.
- **Fixes:** display-scaled pointer coordinates, stale HKCU fixture residue, a malformed fixture
  uninstaller command, redirected-profile WebView2 startup and an invalid UIA condition were fixed in
  the harness.

### Phase 7 verification (this tree)

| Check | Command | Observed |
|---|---|---|
| Real desktop suite | `npm run test:desktop` | `DESKTOP_E2E_OK`; 10 journeys passed, 0 failed |
| Desktop harness build | built automatically by the suite | `desktop_e2e_fixture` example compiled with MSVC |
| Tested revision | `git rev-parse HEAD` | `3d9f2daddb527492afc578c43dc70c1a7a7ce6f2` plus Phase 7 working tree |
| Evidence | `docs/evidence/2026-11-17-desktop-e2e.md` | named captures, `results.json`, `revision.json` and explicit limitations |

UNVERIFIED by this harness: 125%/150% OS scaling, the native startup `ask` dialog under WebDriver,
real cache cleanup and quarantine execution, AI/project cleanup, duplicate/treemap interaction,
Explorer verbs and Store/MSI removal. The dedicated UIA helper exists, but those native/external
windows were not completed in this run. See the receipt for the exact list. Installer/relaunch was
closed separately in the Phase 8 packaging pass below.

## 2h. Phase 8 independent audit, packaging and installer verification — 2026-11-17

The final audit compiled and packaged the current tree, then exercised the real per-user installer.
The packaging pass added no application behavior; the audit's separately tested safety fixes are
listed in [the release audit](docs/RELEASE_AUDIT_2026-11-17.md).
Signing and real application removal remain release blockers.

- **Suites:** 100 frontend tests in 12 files, typecheck, `DOCS_CHECK`, 261 Rust tests (4 ignored),
  cleanup and 8-scenario RC1 fixtures, Hunter benchmark, and 10/10 real WebView2 desktop journeys.
- **Bundles:** `build:unsigned` exit 0; `RELEASE_BUNDLES_OK`; `WINDOWS_METADATA_OK`; all three
  artifacts `NotSigned` (`verify:signatures` exits 1 by design for this RC). The local Chryypt
  certificate chain is `UntrustedRoot`. The published `v0.1.0` NSIS hash matches its
  `SHA256SUMS.txt` but the asset is `NotSigned`.
- **Installer:** NSIS install 1.07 s, repair 1.02 s, uninstall 1.24 s; install directory and HKCU
  uninstall entry created and removed correctly; installed app launched, responded and exited 0;
  reinstall after uninstall succeeded; 0 app-data hash changes and quarantine sentinels preserved.
- **MSI:** unelevated `msiexec /qn` returned 1603 / error 1925. The per-machine package needs an
  elevated install test; it did not install anything unelevated.
- **Evidence:** [installer receipt](docs/evidence/2026-11-17-installer-release.md),
  [installed-app capture](docs/evidence/phase8/34-installed-launch.png), and
  [final matrix](docs/RELEASE_AUDIT_2026-11-17.md).

## 3. Verification — Phase 1 tree, 2026-11-16

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0, 0 diagnostics |
| Frontend tests | `npm test` | 92 passed in 11 files |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 213 passed, 4 ignored (174 library, 26 security, 5 cleanup-stress, 1 real-system, 5 scanner-stress, 2 analyzer-actions) |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 74 Markdown files |
| Production frontend | `npm run build` | exit 0 |
| Desktop debug build | `npm run build:desktop` | `src-tauri/target/debug/cyron.exe` built |
| Native Force Delete → restore | UIA-driven desktop session | disposable folder on E: quarantined into `%APPDATA%` on C:, listed in Cleaner > Quarantine, restored to E: through the UI with contents intact |
| Native Force Delete pre-fix run | UIA-driven desktop session | cross-volume folder restore failed at publish (`os error 80`); fixed by the staging path |

Not run for this tree: installer bundles (`build:unsigned`), signing and negative signature checks,
metadata audit, `verify:dist`, `verify:cleanup`, `verify:e2e`, Hunter benchmark. Real vendor
uninstallation, Store removal, real user-cache mutation, screen-reader traversal, and a clean-VM
installer test remain unexercised.

## 4. Release blockers resolved in the Phase 1 audit

1. **CI documentation gate:** links into `.unlazy/`, `tmp/`, and other gitignored directories now fail
   with a clear message instead of passing until a clean clone breaks. Verified with a temporary
   negative link that exited 1.
2. **Explorer Force Delete quarantine:** the batch had no write-ahead journal, stored only a filename
   in its manifest (so restore and purge could not find it), and fell through to permanent deletion
   when a quarantine rename failed. It now journals before the move, stores the full path, refuses
   unknown modes, reports partial results truthfully, and is covered by
   `explorer::tests::quarantine_writes_journal_and_restores_a_folder` and
   `purge_removes_a_quarantined_folder_entry`.
3. **Cross-volume folder restore:** `restore_batch` failed to publish a directory from another volume.
   It now copies into a same-volume staging folder, publishes with a no-overwrite rename, and only
   then removes the quarantined copy. Tested by `cleanup::tests` and the native run above.
4. **Process termination identity check:** a plan whose expected creation time was 0 silently skipped
   PID-recycling verification. It now refuses such plans, and Kill Process plans omit processes whose
   creation time cannot be read.
5. **Dead Hunter widget setting:** `showHunterWidget` was persisted and toggleable but no desktop
   widget exists. The setting was removed; Settings now explains that Hunter Mode is a confirmed
   session action.
6. **Documentation drift:** IPC counts (34/51) and test counts were stale, the Architecture IPC table
   named commands that do not exist, signing was described as trusted, and a limitation claimed
   cross-volume folder restore was unsupported. All corrected against the tree.

## 5. Prioritized backlog

### P0 — release blockers

1. **Resolve release signing trust.** `v0.1.0` is unsigned and the local certificate is self-signed.
   Either obtain a publicly trusted Authenticode certificate or label every artifact explicitly as
   unsigned/self-signed in release notes. Files: `docs/BUILD_AND_SIGNING.md`,
   `docs/CODE_SIGNING_POLICY.md`, `scripts/sign-build.ps1`, `.github/workflows/release.yml`.
2. **Run an elevated MSI and clean-VM/version-upgrade test.** The NSIS current-user install,
   repair and uninstall path passed on this machine, but the WiX MSI is per-machine and returned
   error 1925 without elevation. Test the MSI elevated, then a version-to-version upgrade and a
   clean Windows VM install. Files: `src-tauri/tauri.conf.json`, `docs/RELEASE_CHECKLIST.md`.
3. **Verify native Store and MSI removal end to end.** Use a disposable Windows profile and a
   disposable MSI package. Confirm the Store plan revalidates identity, `RemovePackageAsync`
   reports truthfully, a denied current-user removal is not escalated, and 1605/1614 do not become
   `completed` without inventory evidence. These are UNVERIFIED today.
4. **Verify the elevated performance paths.** Run one session as administrator to read the real
   Diagnostics-Performance boot impact and record/open a short WPR trace. The unelevated UI was
   verified to be honest and disabled; those two elevated paths are UNVERIFIED. Files: `perf.rs`,
   `src/features/performance/Performance.tsx`.

### P1 — correctness and coverage

1. **Reparse-point refusal lacks a fixture.** Cross-volume restore now refuses a quarantined tree that
   contains a junction or symlink and leaves the batch intact. Add a fixture that creates a junction
   and asserts the refusal; creating one may need developer mode, so gate it or use a fixture helper.
2. **Store removal UI is UNVERIFIED.** The native plan/execute commands and the wizard's Store
   mode exist, but the desktop flow has not been exercised against a real package. Treat
   `--live-store` and `--manual-ui` as open release gates.
3. **Store removal remains current-user only.** Machine-wide and provisioned packages are
   intentionally not attempted; the UI directs those users to Windows Settings or an
   administrator-managed tool.
4. **Provider catalog has no UI consumer.** `list_cleanup_providers` is registered and tested but no
   view lists provider definitions. Build the provider browser or stop advertising it.
5. **Native scheduled-task and service remnants are not discovered.** `leftovers.rs` limits
   destructive candidates to files, shortcuts and registry keys, and explicitly does not infer
   service or task ownership from a name. Add read-only discovery and reviewed ownership evidence
   before offering any cleanup there.
6. **Native desktop flows are manual.** Force Delete quarantine/restore, UAC presentation, and
   Explorer verbs have no automated Tauri desktop test. Keep them as recorded manual gates until a
   WebDriver harness exists.
7. **Real-system release validation.** Vendor uninstallers, Store apps, real caches and real project
   folders have not been exercised end to end. Use a disposable Windows VM; the Recycle Bin
   interactive fixture is ignored by default and must be invoked explicitly.
8. **Performance sampling is not a root-cause engine.** A 700 ms sample cannot prove causation, per-process I/O combines network and device traffic, and elevated boot impact/WPR tracing are unverified. Reproduce the lag, use the comparison honestly, and do not treat "old" or "large" as permission to remove anything. Files: `perf.rs`.
9. **Project discovery coverage is bounded.** Automatic roots, 60,000-directory/400-project caps,
   pnpm per-drive stores, custom `UV_CACHE_DIR`/`PIP_CACHE_DIR`/`HF_HOME`/`OLLAMA_MODELS` and Yarn
   PnP/Zero-Installs are not discovered. Add explicit roots or use the owning tool's prune command;
   do not infer ownership from a name.

### P2 — maintainability and small correctness

1. **Dead scan-index IPC.** `query_directory_children`, `search_indexed_files`, and
   `get_scan_index_stats` are registered and populated but unused by any view. Build the tree/search
   consumer or drop the commands, index build, and client wrappers together.
2. **Cleaner is 1580 lines plus the Projects tab component.** Scan setup, selection, quarantine,
   log, and dialogs still share one file; `DeveloperStorage.tsx` is a separate bounded feature.
   Extract `QuarantinePanel`, the scan header, and the log panel into feature files. No behavior
   change is needed.
3. **Settings is 740 lines; `App.tsx` is 428.** Extract the General, Scanning, Startup, Explorer, and
   Safety tabs into components to reduce merge risk and make settings wiring auditable.
4. **Persisted settings residue.** Old profiles still contain `showHunterWidget` in
   `cyron.settings.v1`; it is ignored. No migration is required unless the schema version changes.

## 6. Next step

The combined tree now passes typecheck, 100 frontend tests, 261 Rust tests, documentation checks, the
production frontend build, ten real desktop journeys, and the NSIS install/repair/uninstall/reinstall
path. Remaining release blockers are public signing trust and the unexercised live vendor/MSI/Store
paths; elevated MSI, cross-version upgrade, 125%/150% scaling and elevated performance tracing are
still UNVERIFIED. Commit and push this tree so CI can prove the gates on a clean clone, then obtain a
publicly trusted certificate and run the remaining native release checks. Do not call Store/MSIX
removal, elevated performance tracing or the MSI install path verified until those runs are recorded.
