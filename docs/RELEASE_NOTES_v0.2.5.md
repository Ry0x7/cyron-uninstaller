# Cyron Uninstaller 0.2.5

Cyron 0.2.5 brings a redesigned interface, a live Task Manager-style monitor, a Disk space map that keeps every folder visible, an in-app way to support the project, and a round of security hardening. Existing 0.2.2 or later installations can update from **Settings → About → Software updates**.

## A redesigned interface

- A calmer, Apple-style look: soft neutrals, borderless cards, translucent fills, a source-list sidebar, a translucent sticky header and segmented controls, in light and dark.

## Live performance monitor

- **Performance → Monitor** shows CPU (overall and per core), memory, disks, network and GPU with 60-second charts.
- **Performance → Processes** lists running processes grouped by app, sortable and heat-tinted by load.
- Startup and Diagnose now have their own tabs.
- Monitoring is read-only. Cyron only reads Windows performance counters; it never ends, suspends or reprioritises a process from these tabs.

## Disk space map fixes

- Folders no longer merge into one large overflow block. Large folders keep their own tiles, including on the fast NTFS (MFT) scan.
- The map is now nested: folders are framed inside their parents, and summarized folders load their contents on demand.
- Hovering, resizing and dragging the splitter no longer lag on large trees.

## Support Cyron

- A **Support Cyron** entry in the sidebar and a Support row in **Settings → About** open the project's Ko-fi page in your default browser. Cyron itself makes no network request for it, and the app still sends no telemetry.

## Security and maintenance

- Explorer is now launched from `%SystemRoot%` when you choose **Show in Explorer**, instead of being looked up on the search path.
- The Rust `url` (RUSTSEC-2026-0592) and `cookie` (RUSTSEC-2026-0594) crates are updated to patched versions; `cargo audit` reports no vulnerabilities.
- All known vulnerabilities in the npm dependency tree are resolved (`npm audit` reports 0). They were all in the desktop test tooling, which is not shipped in the app.
- CodeQL code scanning now runs on every pull request and weekly for TypeScript, Rust and the GitHub Actions workflows.
- Dependabot now checks npm, Cargo and GitHub Actions dependencies every week.
- The CI and release workflows run with read-only repository tokens by default, and the release tag is passed to scripts through environment variables.

## Project and website

- A rebuilt project website with light and dark previews of the new UI and a simulated, clearly labelled live-monitor demo.
- A refreshed README, user guide and screenshots.
- Contributor guidelines, a code of conduct, a security policy with private vulnerability reporting, issue forms and a pull request template.

## Compatibility and trust

- Version 0.2.5 updates the application, installers, PE metadata and About panel together.
- Upgrading preserves `%APPDATA%\dev.cyron.uninstaller` (history, cleanup log and quarantine).
- The Windows binaries are still Authenticode-unsigned. Windows may show an unknown-publisher or SmartScreen warning. The Tauri updater signature proves that an update came from this project's update key; it is separate from Authenticode public trust.
- Verify downloads against the release's `SHA256SUMS.txt`.
