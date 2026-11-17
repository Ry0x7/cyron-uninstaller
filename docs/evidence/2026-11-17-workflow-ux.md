# Workflow UX completion — 2026-11-17

Phase 6 applied the existing design system to every Cyron workflow and closed the state and layout gaps found while walking the production desktop build as a first-time user.

Tree: uncommitted Phase 6 working tree based on `3d9f2da` (`main`). Captured 2026-11-17.

## Environment and scope

- Native Tauri v2 desktop build: `npm run build:desktop`, executable `src-tauri/target/debug/cyron.exe`.
- Embedded production assets, WebView2 page inspected through the remote debugging protocol.
- Windows 11 Pro 25H2 build 26200.9711, 179 real installed applications, 39 measured cache locations.
- Window sizes: 1320×860 and the minimum 1000×680 (984×641 client area).
- Light and dark themes, with the theme preference restored to **System** after the review.
- All measurements came from this machine. No cache, project, application or registry entry was removed during the review.

## Workflow audit

| Workflow | What was verified |
| --- | --- |
| Home | Real system, volume, application, cleanup and recent-activity measurements. Loading, stale and empty states exist per panel. No health score or invented recommendation. |
| Applications | 179 real entries, search, scope/source filters, sorting, row selection, bulk bar, per-row uninstall, Store guidance, broken/portable badges and the application drawer. Table widths remain usable at 1000 px and full details stay in the drawer. |
| Storage & Cleanup | Cleanup and Disk space tabs; measured Windows, application, developer and AI caches; risk composition; protected-data list; removed-application evidence; project cleanup; folder scan; Map, Files, Duplicates and By type; reviewed Analyzer actions. |
| Performance | Symptom-guided investigation, live bounded sample, findings, startup and task lists, baseline comparison and optional trace. The empty recommendation state explains that the current sample does not support a change. |
| History & Recovery | Activity summaries and every recorded operation; Recovery quarantine empty state; cleanup log with completed and failed runs. Narrow layouts now keep the outcome line instead of hiding it. |
| Settings | Appearance, scanning, startup, Explorer integration, safety and about. Only functional controls; startup copy now confirms **Copied**, and the reversible-toggle behaviour is explained. |
| Hunter | **Find by window** opens a real dialog with scope, protected-process limits and Escape exit before tracking starts. |

## Fixes made

1. **Document scroll trap:** `html` is now fixed and no longer scrolls independently of the view slots. A stray programmatic or focus-driven scroll previously moved the whole shell upward and left blank space. See [app.css](../../src/styles/app.css).
2. **Plain-language storage badge:** the header now says **Review before cleanup** instead of the internal **Read-only until confirmed** wording. See [StorageCleanup.tsx](../../src/features/storage/StorageCleanup.tsx).
3. **Honest initial measurement:** the Cleaner reports **Measuring known cache folders on C:, D:, E:…**, disables the duplicate scan action while the first measurement runs, and uses **Not checked yet** before the first result. See [Cleaner.tsx](../../src/features/cleaner/Cleaner.tsx).
4. **Review loading state:** opening a cleanup preview now clears the previous preview, shows a spinner with **Reading the selected locations…**, explains that nothing moves yet, and labels the dismissal **Cancel preview**. Closing the drawer clears the pending preview state. See [Cleaner.tsx](../../src/features/cleaner/Cleaner.tsx).
5. **Audit detail at minimum width:** the Recovery log keeps its outcome line under the operation title at 1000 px; failed and partial runs no longer collapse to a bare badge. See [history.css](../../src/features/history/history.css).
6. **Partial operation status:** a cleanup or cancellation that already moved some files is now recorded as **partial** rather than **completed** or a bare **cancelled**. Activity, Home and the Recovery log all render the fourth state with a caution tone. See [commands.rs](../../src-tauri/src/commands.rs), [Activity.tsx](../../src/features/activity/Activity.tsx), [Recovery.tsx](../../src/features/history/Recovery.tsx) and [Overview.tsx](../../src/features/overview/Overview.tsx).
7. **Startup clarity and feedback:** Settings explains that disabling an app keeps its startup entry and can be reversed, and the Copy action changes to **Copied** for two seconds. See [Settings.tsx](../../src/features/settings/Settings.tsx).
8. **Balanced system summary at 1000 px:** the Home system grid uses two columns with the fifth item spanning the spare row instead of leaving a blank cell. See [overview.css](../../src/features/overview/overview.css).

## Native captures

| Capture | File |
| --- | --- |
| Home, light, loaded | [01-home-light.png](phase6/01-home-light.png) |
| Applications, light | [02-applications-light.png](phase6/02-applications-light.png) |
| Performance, light | [03-performance-light.png](phase6/03-performance-light.png) |
| Cleaner initial measurement | [04-storage-loading-light.png](phase6/04-storage-loading-light.png) |
| Activity, light | [05-history-activity-light.png](phase6/05-history-activity-light.png) |
| Recovery log, light | [06-recovery-light.png](phase6/06-recovery-light.png) |
| Settings with startup explanation | [07-settings-startup-light.png](phase6/07-settings-startup-light.png) |
| Storage & Cleanup, light, loaded | [08-storage-loaded-light.png](phase6/08-storage-loaded-light.png) |
| Home, minimum width | [10-home-min.png](phase6/10-home-min.png) |
| Applications, minimum width | [11-applications-min.png](phase6/11-applications-min.png) |
| Storage & Cleanup, minimum width | [12-storage-min.png](phase6/12-storage-min.png) |
| Activity, minimum width | [13-activity-min.png](phase6/13-activity-min.png) |
| Recovery log, minimum width with outcomes | [14-recovery-min.png](phase6/14-recovery-min.png) |
| Settings, minimum width | [15-settings-min.png](phase6/15-settings-min.png) |
| Performance, minimum width (initial sample) | [16-performance-min.png](phase6/16-performance-min.png) |
| Hunter dialog, minimum width | [17-hunter-dialog-min.png](phase6/17-hunter-dialog-min.png) |
| Storage & Cleanup, dark | [18-storage-dark.png](phase6/18-storage-dark.png) |
| Home, dark | [19-home-dark.png](phase6/19-home-dark.png) |

## Verification

| Check | Command | Observed |
| --- | --- | --- |
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production build | `npm run build` | exit 0; 563.72 kB JS / 128.05 kB CSS; chunk-size advisory only |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED` |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; `src-tauri/target/debug/cyron.exe` built |
| Layout probe | WebView2 evaluation at 984 px | 0 horizontal overflow on all five destinations and Settings; document scroll height stays 641 px |
| Navigation | WebView2 evaluation | Home, Applications, Storage & Cleanup, Performance, History & Recovery, Settings and Find by window opened; no dead route |
| Settings copy feedback | WebView2 evaluation of the production desktop build | 23 startup rows loaded; the Copy action changed to **Copied**, then returned to **Copy** after two seconds. `navigator.clipboard.writeText` was stubbed for the probe so this machine's clipboard was not changed. |
| Partial status | `cargo test cancelled_and_failed_cleanups_get_honest_statuses` | passed; cancelled-with-moved-files and partly-failed runs report `partial` |

## Limitations

- No real cache, project, application or registry change was executed. Destructive controls were reviewed in their real confirmation surfaces; the per-item execution paths remain covered by the existing fixture and Rust suites.
- Screen-reader traversal, Windows forced-colors visuals and 200% OS text scaling were not exercised.
- The first cache measurement on this machine takes minutes for 39 locations and 22.6 GB. The new progress copy and disabled rescan button are honest about that work, but the scan itself was not optimized in this phase.
- The production build still prints Vite's chunk-size advisory; it exits 0.
