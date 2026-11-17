# Cyron — 0.2.0 Release Candidate 2 Notes

**Status: unsigned release candidate (2026-11-18).** The 2026-11-17 audit's trust and native-path
gaps are still open; this candidate is for testing and must not be described as a publicly trusted
or warning-free release. The detailed blockers remain in
[the audit](RELEASE_AUDIT_2026-11-17.md#outstanding-issues).

## Changes since release candidate 1

### Uninstaller

- **Batch selection matches what the backend can execute.** The checkbox, select-all action, queue
  count and selection cleanup share one eligibility rule. Only applications with a registered Win32
  vendor uninstaller can be queued; Store/MSIX packages, Steam games, portable applications, broken
  registrations, protected applications and blank commands cannot.
- **Quiet mode is part of eligibility.** An application whose only command is a quiet uninstaller
  can be queued while the silent switch is on and is removed from the queue when it is switched off.
  A refreshed inventory is re-checked the same way.
- **Disabled checkboxes explain themselves.** The tooltip names the reason, for example that a
  Steam game is removed through Steam or that an unregistered executable needs Forced Removal.
- **The server refuses a mixed batch instead of running a subset.** `prepare_batch_uninstall`
  re-validates every selected application and fails with the application name and reason.

### Performance diagnostics

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

All new sources are read-only. Cyron does not install, roll back or remove a driver, clear standby
memory, disable services, delete Prefetch, change the page file, or apply driver/power tweaks.

## Safety and correctness

- **Force Delete cannot be retargeted by a junction or symlink.** The plan refuses reparse
  targets, binds to the file identity (volume serial, file index, creation time, size and
  attributes), and rechecks directory counts immediately before acting.
- **Quarantine restores are scoped to their batch.** A forged or corrupted manifest path outside
  the batch is refused, and relative destinations are rejected.
- **Cross-volume quarantine refuses a tree that contains a junction or symlink** instead of
  silently dropping the link from the recovered copy.
- **Process termination holds a verified handle** through the graceful wait and force fallback, so
  a reused PID cannot be terminated by mistake.
- **Provider definitions cannot label protected data as `caution`.** Conversation history,
  credentials, model data and Windows update state require `protected`.
- **Partial cleanup outcomes are recorded honestly.** A stop or failure after files moved is
  `partial`, not `completed`; total failure and cancellation remain distinct.

## Interface

- Five destinations: Home, Applications, Storage & Cleanup, Performance, History & Recovery.
- Settings lives in the sidebar footer; Hunter Mode is **Find by window** inside Applications.
- Shared token/primitive system, visible focus, dialog focus trapping, reduced-motion support,
  minimum-window layouts with no horizontal document overflow.

## Packaging and installation

- **NSIS (current user):** the 0.2.0 candidate installed in 1.56 s, launched, uninstalled in 1.25 s
  and reinstalled. The install directory and HKCU uninstall entry were created and removed
  correctly; the 60,024-file app-data tree was unchanged across uninstall and reinstall.
  Same-version repair and a 0.1.0 → 0.2.0 upgrade were not rerun for RC2.
- **MSI (per machine):** metadata and bundle structure verified. Unelevated execution returns MSI
  error 1925 and installs nothing; an elevated install/uninstall was not performed.
- **Signing:** every published asset is unsigned. The local `CN=Chryypt Organization` certificate
  is self-signed, so even a locally signed build is not publicly trusted and earns no SmartScreen
  reputation.
- Full commands, timings, MSI log excerpts and limitations are in
  [the 0.2.0 release receipt](evidence/2026-11-18-release-0.2.0.md).

## Verification

- `npm run typecheck`: exit 0.
- `npm test`: 112 passed in 14 files, including batch eligibility and spike verdict tests.
- Rust: full suite 255 passed, 4 ignored, 0 failed; focused `perf::tests` 10 passed; the live
  performance integration test passed.
- `npm run docs:check`: passed.
- `RELEASE_BUNDLES_OK` and `WINDOWS_METADATA_OK` are rechecked by the release workflow for the
  tagged revision. Signature verification is expected to fail as `NotSigned` by design.- Post-publication native verification on 2026-11-18 drove both panels through the real Tauri/WebView2 app as part of an 11/11 desktop suite run. See [the native receipt](evidence/2026-11-18-desktop-performance.md).

## Known blockers

The published artifacts are unsigned and the local certificate is self-signed. There was no
clean-VM or version-to-version upgrade test, no elevated MSI install, no real MSI/vendor/Store
removal, and elevated diagnostics, 125%/150% scaling and screen-reader paths remain unverified. The
installer must not be described as publicly trusted or warning-free.
