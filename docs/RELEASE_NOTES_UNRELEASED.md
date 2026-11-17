# Cyron — Unreleased Changes

Changes after the `v0.2.0-rc.1` testing prerelease. No new version has been tagged or published.

## Uninstaller

- **Batch selection matches what the backend can execute.** The checkbox, select-all action, queue
  count and selection cleanup now share one eligibility rule. Only applications with a registered
  Win32 vendor uninstaller can be queued; Store/MSIX packages, Steam games, portable applications,
  broken registrations, protected applications and blank commands cannot.
- **Quiet mode is part of eligibility.** An application whose only command is a quiet uninstaller
  can be queued while the silent switch is on and is removed from the queue when it is switched off.
  A refreshed inventory is re-checked the same way.
- **Disabled checkboxes explain themselves.** The tooltip names the reason, for example that a
  Steam game is removed through Steam or that an unregistered executable needs Forced Removal.
- **The server refuses a mixed batch instead of running a subset.** `prepare_batch_uninstall`
  re-validates every selected application and fails with the application name and reason, so the
  earlier "none of the selected applications have a valid uninstaller command" plan cannot be
  created again.

## Performance diagnostics

- **Kernel share is visible.** The page separates kernel and user CPU and shows kernel as a share
  of busy CPU, because kernel-mode work has no process row.
- **Kernel-dominant samples get an evidence-bound finding.** A `system` recommendation appears only
  when kernel is at least 20% absolute and at least 60% of busy CPU. It states the kernel share and
  the measured application CPU it does not explain, and says that these counters cannot attribute
  DPC/ISR time to a named driver.
- **20-second spike capture.** Records about 20 one-second readings and keeps peak CPU, kernel,
  memory and physical-disk values plus the process at the CPU peak. It labels the burst
  process-, kernel-, disk-, mixed- or idle-dominated, can be stopped early, writes nothing to disk
  and ends when the page is left.
- **Driver inventory.** Display, storage and network driver names, versions and package dates are
  read from the class registry.
- **Recent System evidence.** Classified driver, device, storage, power and service errors from the
  Windows System log over the last seven days are listed with their source. A denied event log is
  reported as unavailable, never as an empty result.
- **Sixth symptom guide.** **CPU spikes / kernel time** filters the evidence for this case and
  points to the capture and driver/event panels before any change is made.

All of the above is read-only. Cyron does not install, roll back or remove a driver, clear standby
memory, disable services, delete Prefetch, change the page file, or apply driver/power tweaks.

## Verification

- `npm run typecheck`: exit 0.
- `npm test`: 112 passed in 14 files, including `batchEligibility.test.ts` and
  `spikeAnalysis.test.ts`.
- Rust: full suite 255 passed, 4 ignored, 0 failed; focused `perf::tests` 10 passed; the live
  performance integration test passed.
- `npm run docs:check`: passed.
- The new spike and driver panels have not yet been driven in the native window; that native UI
  verification remains open.
