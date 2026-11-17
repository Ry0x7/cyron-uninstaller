# Cyron — 0.2.0 Release Candidate 1 Notes

**Status: unsigned release candidate (2026-11-18).** The 2026-11-17 audit's trust and native-path
gaps are still open; this candidate is for testing and must not be described as a publicly trusted
or warning-free release. The detailed blockers remain in
[the audit](RELEASE_AUDIT_2026-11-17.md#outstanding-issues).

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

## Diagnostics and performance

- Evidence-bound performance page: live CPU, memory, PDH physical-disk, process, startup and
  scheduled-task inventory; reversible startup toggles; before/after baselines.
- Recommendations require measured thresholds and carry evidence, expected effect, risk and
  verification. Idle physical disk + high process I/O is not reported as disk pressure.
- Elevated boot-impact and WPR paths remain opt-in and unverified in this tree.

## Interface

- Five destinations: Home, Applications, Storage & Cleanup, Performance, History & Recovery.
- Settings lives in the sidebar footer; Hunter Mode is **Find by window** inside Applications.
- Shared token/primitive system, visible focus, dialog focus trapping, reduced-motion support,
  minimum-window layouts with no horizontal document overflow.

## Packaging and installation

- **NSIS (current user):** 0.2.0 fresh install (1.56 s), launch, uninstall (1.25 s) and
  reinstall. The install directory and HKCU uninstall entry were created and removed correctly;
  the 60,024-file app-data tree was unchanged across uninstall and reinstall. Same-version repair
  and a 0.1.0 → 0.2.0 upgrade were not rerun.
- **MSI (per machine):** metadata and bundle structure verified. Unelevated execution returns MSI
  error 1925 and installs nothing; an elevated install/uninstall was not performed.
- **Artifacts from this tree:** NSIS `2,270,729` bytes / `705b63787aac117d8c660024a8dd9560f50b1f8225e89a93fa279647ccb4e616`,
  MSI `3,260,416` bytes / `313a089fca2530719d91768fd3cc1083b4024592b11ad48fa316f4ef2192fe57`,
  `cyron.exe` `6,918,144` bytes / `ca78b5757e0c4448da8bfe129314117970ba841ab2cb3a93996098d2bb6cebfa`.
- Full commands, timings, MSI log excerpts and limitations are in
  [the 0.2.0 release receipt](evidence/2026-11-18-release-0.2.0.md).

## Measurements

- Cyron-only three-run scan measurements on this machine are in [BENCHMARKS.md](BENCHMARKS.md).
- No competitor benchmark exists. [COMPETITOR_COMPARISON.md](COMPETITOR_COMPARISON.md) is a
  documentation-based feature matrix and marks competitor scenarios **NOT RUN**.

## Known blockers

The 0.2.0 RC1 artifacts are unsigned and the local certificate is self-signed; the published
`v0.1.0` NSIS asset is also `NotSigned`. There was no clean-VM or 0.1.0 → 0.2.0 upgrade test, no
elevated MSI install, no real MSI/vendor/Store removal, and elevated diagnostics, 125%/150% scaling
and screen-reader paths remain unverified. The installer must not be described as publicly trusted
or warning-free.
