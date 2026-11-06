# Cyron

Cyron is a Windows desktop utility that combines an application inventory, a disk-space
analyzer and a developer/AI cache inspector. It is the foundation for a Revo Uninstaller +
BleachBit + WinDirStat replacement with plain-language risk labels, built for people who are
not sysadmins.

> **Phase 1 is read-only for scanned data.** Cyron scans, measures and explains. It does not
> clean user files, uninstall programs, stop processes or edit the registry. The Activity screen
> can clear Cyron's own history log.

## What works today

- **Application inventory** — reads the Windows uninstall registry (64-bit and 32-bit views,
  machine and current user), hides updates/patches/system components, and shows publisher,
  version, install date, architecture, estimated size and install location when the installer
  recorded them. Search, filter by scope, sort, open the install folder, or measure the real
  on-disk size of the install folder.
- **Disk analyzer** — pick a drive or folder, watch live progress, cancel, then explore a
  squarified treemap with breadcrumb drill-down, a synchronized selection panel and folder tree
  (Selection / Tree / Largest / Types in the details rail), the largest files and extension
  statistics. Logical size and allocated size are reported separately. The IPC tree is pruned and
  the Tree tab says so.
- **Developer & AI cache inspector** — a catalog of 48 known locations covering Claude Code,
  Codex CLI, Gemini CLI, Cursor, Antigravity, VS Code, Ollama, LM Studio, npm, pnpm, Yarn, pip,
  Cargo, NuGet, Docker, WSL, Windows temporary files and browser caches. Every location is
  measured for real and classified as **Safe** (regenerates), **Review** (re-download or reset
  cost) or **Protected** (history, models, settings, volumes — never offered for removal).
- **Overview** — real volume capacity/free space, installed application counts, total cleanup
  potential and recent scan activity.
- **Activity** — local history of completed and cancelled disk scans (up to 100 entries).
- **Settings** — system/light/dark theme, include-hidden-files toggle, and an exclusion list
  that is sent to the scanner.

Phase 1 does not clean user files, uninstall programs, terminate processes or write registry values.
The `clear_history` command removes only Cyron's own activity log; history temp-file maintenance
also removes only Cyron-created temporary files. See [Safety guarantees](#safety-guarantees).

## Not in Phase 1

Uninstalling, leftover-file removal, cache deletion, process termination, registry backup and
rollback, duplicate-file detection, Windows Store/MSIX enumeration, and a persistent tree column
beside the treemap (the Tree rail tab exists) are not implemented. The **Cleaner** page is
explicitly a preview.

## Stack

| Layer | Technology |
| --- | --- |
| Desktop shell | Tauri v2 |
| Backend | Rust (MSVC toolchain), `windows-sys`, `winreg` |
| Frontend | React 19, TypeScript, Vite 7 |
| Charts | Custom squarified treemap (no charting dependency) |
| Tests | Vitest (frontend), `cargo test` (Rust) |
| CI | GitHub Actions, `windows-latest` |

## Prerequisites

- Windows 10 (1809+) or Windows 11 with the **WebView2 runtime** (included with Windows 11).
- **Node.js 22+** and npm.
- **Rust stable** with the `x86_64-pc-windows-msvc` target.
- **Visual Studio 2022 Build Tools** with the *Desktop development with C++* workload and a
  Windows 10/11 SDK. Tauri links native Windows libraries, so a linker and SDK are mandatory.
- On machines where the MSVC tools are not on `PATH`, this repository includes
  [scripts/msvc-env.mjs](scripts/msvc-env.mjs), which discovers the toolchain through
  `vswhere` and runs a command with the right `PATH`, `LIB` and `INCLUDE`. The npm scripts
  below use it automatically.

## Getting started

```powershell
npm install
npm run dev:desktop        # Vite + Tauri dev window
```

Build a debug desktop executable without installers:

```powershell
npm run build:desktop      # src-tauri/target/debug/cyron.exe
```

Release installers (MSI/NSIS) are configured in
[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) and can be produced with
`npm run tauri -- build`. Bundling downloads WiX/NSIS tooling on first use; it has not been
verified in this Phase 1 pass.

## Verification

```powershell
npm run typecheck                                      # TypeScript, project references
npm test                                               # 38 Vitest tests
npm run build                                          # production frontend bundle
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test real_system -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test read_only -- --nocapture
node scripts/verify-dist.mjs                           # build + bundle assertions
node scripts/verify-desktop-binary.mjs                 # debug binary exists
node scripts/smoke-desktop.mjs                         # launches the app, checks its window, exits
```

The Rust suite includes unit tests for every backend module, a real-system integration test that
reads the live registry, measures real cache folders and scans a temporary fixture tree, and a
read-only safety suite that audits the backend for destructive APIs and proves junction skipping
and hard-link dedupe. The debug build and launch have been verified on Windows
(`DESKTOP_BINARY_OK`, `DESKTOP_SMOKE_OK`); release installers and a native visual pass have not.
Current status, known gaps and the latest verification results are tracked in
[docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md).

## Project layout

```text
src/                       React application
  components/              shared primitives (buttons, badges, panels, drawer, icons)
  features/
    overview/              storage, apps and cleanup summary
    uninstaller/           read-only application inventory + detail drawer
    analyzer/              scan controls, treemap, selection rail
    cleaner/               cache/risk inspector (preview only)
    activity/              local scan history
    settings/              appearance, scan scope, exclusions
  lib/                     typed IPC client, formatting, treemap math, risk model, settings
  styles/                  design tokens and shared layout CSS
src-tauri/                 Tauri + Rust backend
  src/apps.rs              Windows uninstall-registry discovery
  src/fs.rs                native directory enumeration (FileIdBothDirectoryInfo)
  src/scan.rs              bounded scanner, progress, cancellation, aggregation
  src/caches.rs            developer/AI/Windows cache catalog and measurement
  src/volumes.rs           local volume capacity
  src/history.rs           app-data scan history (max 100 entries)
  src/commands.rs          validated Tauri command layer
  src/lib.rs               app state, dialog plugin, command registration
  tests/real_system.rs     real registry/cache/scan integration test
  tests/read_only.rs       destructive-API audit, junction and hard-link safety tests
scripts/                   MSVC environment shim, build/smoke checks
docs/                      architecture and implementation status
```

## Safety guarantees

- **No user-data cleanup or uninstall path.** There is no command to delete arbitrary files,
  uninstall software, terminate processes or write registry values. `clear_history` removes only
  Cyron's own `history.json`; temp-file cleanup is limited to files created by Cyron. The Cleaner
  renders findings without a removal action.
- **No arbitrary paths.** Scan and reveal commands require a non-empty absolute path that
  exists (and is a directory for scans), then canonicalize it before use.
- **Junctions and symlinks are never followed.** Reparse points are counted and skipped, so a
  scan cannot loop or escape its root.
- **Hard links are deduplicated for allocated space** when native directory enumeration supplies
  file ids. The `std::fs` fallback cannot deduplicate links, and after one million tracked ids new
  file ids are no longer retained, so a later duplicate may count twice.
- **Capped outputs and work controls.** Per-directory retained file nodes, IPC tree depth/child
  count, returned extension stats, largest files, errors, and tracked hard-link ids are capped;
  cancellation, depth and exclusions are supported. The scanner still materializes a directory's
  entries and subdirectory nodes before pruning, and its extension aggregation map can grow with
  the input, so strict memory bounds for arbitrarily large scans are not guaranteed.
- **Local only.** No network calls, no telemetry, no uploads. Scan results stay in memory;
  activity history and settings stay in this Windows profile.
- **No elevation.** Launching without administrator rights is supported; unreadable folders are
  reported rather than crashing the scan.

## Known limitations

- **Win32 registry inventory only.** Windows Store/MSIX apps and portable programs are not
  enumerated yet. Sizes are the installer's `EstimatedSize` when present, otherwise unknown;
  they are never fabricated. Use *Measure folder* for the real size of an install directory.
- **No MFT scan.** Enumeration uses `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`
  with a `std::fs` fallback. No WizTree-level benchmark has been run or is claimed.
- **The IPC tree is pruned.** At most 200 children per folder, 8 levels deep, and up to 40
  individual file nodes per folder plus aggregate groups are returned. Totals cover the full walk;
  extension stats return the top 24 and largest files the top 100. Per-directory entry and child
  vectors, and the extension aggregation map, can grow with input; arbitrary scans are not strictly
  memory-bounded.
- **Network and optical drives are skipped** when listing volumes, so an offline mapped drive
  cannot freeze the overview.
- **Fixed cache catalog.** Locations are discovered only from the built-in catalog; it does not
  learn new tool paths. Individual projects and custom install locations are not scanned.
- **No user-data cleanup.** Cleaner risk levels are descriptive; Phase 3 is planned to add
  previews and controlled cleanup. The current `clear_history` action only removes Cyron's own log.
- **Activity history covers disk scans only.** App and cache inventory lookups are not logged;
  the per-profile file is `<Tauri app_data_dir>/history.json`, capped at 100 entries. Settings use
  `localStorage["cyron.settings.v1"]`.
- **Windows only.** The Rust backend calls Win32 APIs unconditionally.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — components, scanner pipeline, IPC surface and
  data flows.
- [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md) — Phase 1 acceptance status,
  evidence and limits.
- [DESIGN.md](DESIGN.md) — tokens, themes, component anatomy, treemap palette, motion and
  accessibility rules.
- [docs/SECURITY.md](docs/SECURITY.md) — Phase 1 threat model, scanner guarantees and the gates
  required before destructive features.
- [docs/BUNDLE.md](docs/BUNDLE.md) — build, installer targets, signing status and app-data paths.
- [ROADMAP.md](ROADMAP.md) — phased plan to a complete uninstaller/cleaner.
- [AGENTS.md](AGENTS.md) — canonical repository memory for contributors and agents.

## Project status

Phase 1 foundation and read-only scanning is implemented, including a synchronized folder tree in
the analyzer rail. The debug desktop build and launch are verified; a native visual pass and
release packaging are not. Phase 2 is disk intelligence;
Phase 3 adds safe cleaning, Phase 4 the uninstall engine and Phase 5 packaging and hardening.
No license file has been added to the repository yet.
