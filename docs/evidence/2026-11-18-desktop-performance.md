# Native desktop verification — spike capture and driver evidence

Recorded 2026-11-18. This run drives the real compiled Tauri/WebView2 application; no IPC result is
mocked.

## Tested revision and environment

| Item | Value |
|---|---|
| Git revision | `b274e412b01ca89a38ae7358e5074168126e6c20` plus the uncommitted spec addition in [02-secondary.spec.mjs](../../tests/desktop/02-secondary.spec.mjs) |
| Application | `src-tauri/target/debug/cyron.exe` (embedded production frontend) |
| Driver | `tauri-driver` + Microsoft Edge WebDriver `157.0.4341.46` |
| WebView2 runtime | `157.0.4341.46` |
| Node.js | `v24.18.0` |
| OS | Windows 11 Pro `10.0.26200`, real interactive desktop session |
| Window | 1320×860 default; the suite also exercises 1000×680 and 1440×960 |

The suite ran with `DESKTOP_E2E_OK`. The fixture, HKCU entries, temp workspace and Cyron app data
were torn down or restored by the harness.

## Result

| Item | Value |
|---|---|
| Spec files | 2 passed, 0 failed |
| Journeys | **11 passed, 0 failed, 0 skipped** |
| Total duration | 20 m 08 s |
| New journey | `spike capture and kernel/driver evidence render from live data`, 55.6 s |
| Machine-readable results | [results.json](2026-11-18-native/results.json), [revision.json](2026-11-18-native/revision.json) |

The ten pre-existing journeys passed unchanged, so the new Performance controls did not regress
navigation, uninstall, recovery, themes or the minimum-window layout.

## What the new journey asserted

1. **Live driver inventory.** The overview's **Drivers listed** count matched the number of rendered
   driver rows, and the panel listed four drivers: AMD Radeon RX 9060 XT (Display, v32.0.31041.1004,
   17 Aug 2026), Intel Ethernet Controller I225-V, TP-Link Wireless USB Adapter and a Disk drive.
2. **System-event provenance.** The evidence block rendered **Driver and device errors from the last
   7 days**. This run found 0 matching events and showed the explanatory note rather than an empty
   box. The panel states that a missing event is not proof of a healthy driver.
3. **Kernel guide.** Selecting **CPU spikes / kernel time** changed the answer to **Separate kernel
   and driver work from applications** and kept the evidence filtered to the selected symptom.
4. **20-second capture.** Starting the capture produced the progress state and a chart of readings.
   The completed capture showed all 20 readings, peak CPU 42%, peak kernel 14% (39% of the peak),
   physical disk 0.4%, and `brave.exe` as the largest measured process. The verdict was **The spike
   had no single dominant source**, which is the honest outcome for that sample.
5. **Layout.** The diagnostics and capture panels reported no horizontal document overflow at the
   default window.

## Evidence

- [Kernel and driver evidence](2026-11-18-native/34-kernel-driver-evidence.png) — 4 live drivers, kernel 7.5%
  (30% of busy CPU), user 17%, 0 recent System events.
- [Spike capture result](2026-11-18-native/35-spike-capture.png) — 20 readings, peak CPU 42%, kernel 14%, disk
  0.4%, `brave.exe`, mixed verdict.

The captures contain this machine's real driver inventory and process names. They are acceptance
evidence, not a synthetic fixture; no driver or system setting was changed.

## Limitations — explicitly UNVERIFIED

- The capture was not stopped early by a deterministic in-page action; this run completed its 20
  readings. The `Stop capture` path is exercised only by the component's own logic, not asserted
  against a partial chart.
- No kernel-dominant spike occurred during this run. The kernel finding's threshold is covered by
  Rust unit tests, but a live kernel-heavy sample was not reproduced.
- The elevated Diagnostics-Performance boot impact and WPR trace paths remain UNVERIFIED.
- 125%/150% OS scaling, screen-reader traversal and forced-colors rendering remain UNVERIFIED.
- One run on one machine; no claim is made about other hardware or driver sets.
