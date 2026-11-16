# Performance Diagnostics — Native Verification Receipt

Date: 2026-11-17
Tree: uncommitted performance diagnostics work on `main`
Platform: Windows 11 x64, native Tauri debug build (`src-tauri/target/debug/cyron.exe`), local Vite dev server
Driver: Windows UI Automation (UIA) against the real desktop window

This receipt records what was exercised in the actual desktop app. Raw captures are committed here because
this directory is the repository's evidence location; no `.unlazy/`, `tmp/` or local-only path is cited.

## Commands

```powershell
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib perf::tests
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test performance -- --nocapture
npm run typecheck
npm test
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
npm run docs:check
```

`perf::tests` passed 8/8, including the new regression
`high_process_io_with_an_idle_physical_disk_is_not_disk_pressure` and
`windows_binary_accepts_only_fixed_relative_names`. The live `tests/performance.rs` snapshot passed
on this machine. The full Rust suite passed 253 tests with 4 ignored.

## Native checks

| Check | Observed |
|---|---|
| Live metrics | The Performance page showed a 702 ms sample across 179 processes: 15% total CPU, 37% memory in use (30.3 GB available of 47.9 GB), 1.1% physical-disk busy with queue 0.03, and E: at 13% free. These are the same values the snapshot reported. See [performance page](phase4-native-performance.png). |
| Guided investigation | UIA selected **Startup takes forever**, **Disk is almost full** and **The fan is loud**. Each switched the guided title and filtered recommendations; with no supporting pressure the page showed **No pressure matches this symptom** instead of inventing an action. |
| Startup evidence | The startup tab showed 11 of 23 entries enabled, source (`HKCU\Run` or Startup Folder), boot impact **Unavailable** because the Diagnostics-Performance log is administrator-only, live usage where measured, state and a per-row action. |
| Reversible startup toggle | A disposable HKCU `Run` value `CyronPerfFixture` was created. Clicking **Disable** opened a native confirmation dialog with the exact name, location and consequence. **Cancel** closed it and left the value enabled. **Disable** wrote `03 00 00 00 ...` to `StartupApproved\Run`, preserved the Run value, showed the disabled notice and changed the row action to **Enable**. **Enable** wrote `02 00 00 00 ...`, restored the enabled state and reversed the row. The fixture value and approval bytes were removed afterwards. See [confirmation dialog](phase4-native-startup-confirmation.png). |
| Native confirmation, not `window.confirm` | The dialog is a real Win32 `#32770` window with **Disable** and **Cancel** buttons. This replaced `window.confirm`, which produced no visible dialog in WebView2 during the audit. |
| Before/after baselines | Two labeled measurements were recorded through the form. The page compared previous and latest CPU, memory, available memory, commit and disk busy, with the text **Two samples show association, not causation**. The JSON file persisted both records; it was removed after the check. See [baselines](phase4-native-baselines.png). |
| Advanced trace availability | Unelevated, the page reported **Needs administrator**, explained that Windows Performance Recorder requires it, disabled **Record trace**, and did not offer an elevation prompt. |
| Fixed Windows binaries | After the security-boundary review, `schtasks.exe`, `taskmgr.exe`, `resmon.exe`, Explorer and `wpr.exe` are resolved by `windows_binary` under `%SystemRoot%` instead of PATH. The rebuilt native page still listed 23 scheduled tasks with their paths, authors and triggers. |
| Manual refresh freshness | A disposable startup item appeared only after a manual **Refresh** bypassed the 120-second startup-evidence cache. Live sampling still reuses the cache. This was a real defect found during the run and fixed. |
| Disk recommendation | The first native run recommended **Measured disk I/O is busy** while physical disk busy was 0.1% and queue 0.00, because the gate also accepted 40.8 MB/s of per-process I/O (which includes network transfers). The gate now requires measured physical-disk busy and queue thresholds; the rebuilt app showed no disk recommendation at 1.1% busy. The regression test above covers the idle-disk/high-process-I/O case. |

## Not exercised

- Recording and opening a real WPR trace as administrator. The unelevated path was verified to be
  honest and disabled; the recording path remains **UNVERIFIED**.
- Per-app boot impact from the Diagnostics-Performance event log under an elevated session. The
  unelevated path correctly showed **Unavailable** and explained why; the elevated path is
  **UNVERIFIED**.
- Reproducing a real user slowdown, temperature or fan measurement. Cyron does not measure either.
- Changing a machine-wide (`HKLM` / all-users) startup entry, which requires administrator rights.
- Scheduled-task changes. The page lists boot/logon tasks read-only and explicitly says it does not
  disable or run them.

## Limitations carried forward

A short sample cannot prove causation; the page says so and offers an optional trace only when the
basic metrics cannot explain the lag. CPU, memory, disk and process figures are point-in-time.
Per-process I/O combines file, network, pipe and device transfers. Windows uses free memory as cache,
so lower memory use is not itself a speed improvement. No RAM booster, registry cleaner, Defender
change, service disabling, Prefetch deletion, page-file change, update disabling or driver tweak is
implemented.
