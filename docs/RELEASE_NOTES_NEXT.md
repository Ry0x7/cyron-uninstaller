# Cyron — Next Release Notes (unreleased)

**Status: not released. Version number to be assigned.** The 2026-11-17 audit blocks a public
release until the items in [the audit](RELEASE_AUDIT_2026-11-17.md#outstanding-issues) are closed.
Nothing here authorizes publishing the current tree.

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

- **NSIS (current user):** verified install (1.07 s), same-version repair (1.02 s), relaunch,
  uninstall (1.24 s) and reinstall. The install directory and HKCU uninstall entry are created and
  removed correctly; 0 app-data hash entries changed and quarantine sentinels survived.
- **MSI (per machine):** metadata and bundle structure verified. Unelevated execution returns MSI
  error 1925 and installs nothing; an elevated install/uninstall was not performed.
- **Artifacts from this tree:** NSIS `2,271,846` bytes / `16a378226f6162baa93373830817bb20df65c59e3f9ae589f34b15dbbdec479f`,
  MSI `3,330,048` bytes / `3f1e64a7594fba9df4fbecfb7537bde36175dd11166ac6fc346ebe88b6612630`,
  `cyron.exe` `6,918,144` bytes / `95c3f7effec7ce3ff9a922997f8bc3ccb275ead8df88167d07f7dd0268ca20d4`.
- Full commands, timings, MSI log excerpts and limitations are in
  [the installer receipt](evidence/2026-11-17-installer-release.md).

## Measurements

- Cyron-only three-run scan measurements on this machine are in [BENCHMARKS.md](BENCHMARKS.md).
- No competitor benchmark exists. [COMPETITOR_COMPARISON.md](COMPETITOR_COMPARISON.md) is a
  documentation-based feature matrix and marks competitor scenarios **NOT RUN**.

## Known blockers

Unsigned and self-signed installers (the published `v0.1.0` NSIS asset is also `NotSigned`), no
clean-VM or version-to-version upgrade test, no elevated MSI install, no real MSI/vendor/Store
removal, unverified elevated diagnostics, unverified 125%/150% scaling and screen-reader paths.
The installer must not be described as publicly trusted or warning-free.
