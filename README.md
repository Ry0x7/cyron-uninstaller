# Cyron

Cyron is a Windows desktop utility that combines an application inventory, a disk-space analyzer
and a developer/AI cache inspector. It is the foundation for a Revo Uninstaller + BleachBit +
WinDirStat replacement with plain-language risk labels, built for people who are not sysadmins.

> **Read-only for scanned data.** Cyron scans, measures and explains. It does not clean user files,
> uninstall programs, stop processes or edit the registry. The Activity screen can clear Cyron's own
> history log.

## What works today

- **Application inventory** — reads the Windows uninstall registry (64-bit and 32-bit views,
  machine and current user), hides updates/patches/system components, and shows publisher,
  version, install date, architecture, estimated size and install location when the installer
  recorded them. Search, filter by scope, sort, open the install folder, or measure the real
  on-disk size of the install folder.
- **Disk analyzer** — pick a drive or folder, watch live progress, cancel, then work in three
  modes:
  - **Map** — a squarified treemap beside a persistent, resizable folder tree. Selection,
    expansion, breadcrumbs and the details rail stay in sync; the tree is virtualized, so large
    folders are not cut off at a row limit.
  - **Storage** — ten storage categories (applications, games, downloads, documents, media,
    Windows system, developer tools, AI models, caches, other) with known-vs-inferred confidence,
    age buckets and explained observations. Categories describe what consumes space; they are not
    removal recommendations.
  - **Duplicates** — size-bucket candidates verified with 4 KiB edge hashing and then full
    streaming SHA-256, grouped by content with per-file locations, wasted bytes, progress and
    cancellation. Hard links are labelled and excluded from recoverable bytes. Read-only.
  - Filters (name, minimum size, age, extension, type, sort) apply to the tree, largest files and
    treemap, and the totals show the filtered subset. Logical size and allocated size are reported
    separately.
- **Two scanners** — an NTFS MFT reader for drive roots and a streaming directory walk everywhere
  else. The chosen method and any fallback reason are shown after a scan. MFT requires administrator
  rights; without them the walk runs and the reason is reported.
- **Developer & AI cache inspector** — a catalog of 48 known locations covering Claude Code,
  Codex CLI, Gemini CLI, Cursor, Antigravity, VS Code, Ollama, LM Studio, npm, pnpm, Yarn, pip,
  Cargo, NuGet, Docker, WSL, Windows temporary files and browser caches. Every location is
  measured for real and classified as **Safe** (regenerates), **Review** (re-download or reset
  cost) or **Protected** (history, models, settings, volumes — never offered for removal).
- **Overview** — real volume capacity/free space, installed application counts, total cleanup
  potential and recent scan activity.
- **Activity** — local history of completed and cancelled disk scans (up to 100 entries).
- **Settings** — system/light/dark theme, include-hidden-files toggle, and an exclusion list that
  is sent to both scanners.

Cyron does not clean user files, uninstall programs, terminate processes or write registry values.
The `clear_history` command removes only Cyron's own activity log; history temp-file maintenance
also removes only Cyron-created temporary files. See [Safety guarantees](#safety-guarantees).

## Not implemented yet

Uninstalling, leftover-file removal, cache deletion, process termination, registry backup and
rollback, duplicate management (Phase 2 reports duplicates but does not act on them), Windows
Store/MSIX enumeration, and a standalone release installer. The **Cleaner** page is explicitly a
preview.

## Stack

| Layer | Technology |
| --- | --- |
| Desktop shell | Tauri v2 |
| Backend | Rust (MSVC toolchain), `windows-sys`, `winreg`, `sha2` |
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

The debug executable loads the Vite dev server, so keep `npm run dev` (or `npm run dev:desktop`)
running when you launch it. Release installers (MSI/NSIS) are configured in
[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) and can be produced with
`npm run tauri -- build`. Bundling downloads WiX/NSIS tooling on first use; it has not been
verified in this pass.

## Verification

```powershell
npm run typecheck                                      # TypeScript, project references
npm test                                               # 51 Vitest tests
npm run build                                          # production frontend bundle
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test real_system -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test read_only -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test stress_scan -- --nocapture
node scripts/verify-dist.mjs                           # build + bundle assertions
node scripts/verify-desktop-binary.mjs                 # debug binary exists
node scripts/smoke-desktop.mjs                         # launches the app, checks its window, exits
```

Scanner benchmarks (the MFT half needs an Administrator terminal):

```powershell
node scripts/bench-scan.mjs --path C:\ --method directory --runs 3
node scripts/bench-scan.mjs --path C:\ --method both --runs 3 --write
```

Measured results and their caveats are in [docs/BENCHMARKS.md](docs/BENCHMARKS.md). The latest
whole-volume numbers on the development machine are 14.52 s with MFT and 155.24 s with the
directory walk, warm cache, single machine — not a WizTree comparison.

The Rust suite includes unit tests for every backend module, a real-system integration test that
reads the live registry, measures real cache folders and scans a temporary fixture tree, a
read-only safety suite that audits the backend for destructive APIs and proves junction skipping
and hard-link dedupe, and a stress suite for wide/deep trees and access-denied folders.
Current status, known gaps and the latest verification results are tracked in
[docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md).

## Project layout

```text
src/                       React application
  components/              shared primitives (buttons, badges, panels, drawer, icons)
  features/
    overview/              storage, apps and cleanup summary
    uninstaller/           read-only application inventory + detail drawer
    analyzer/              scan controls, treemap, tree, filters, duplicates, storage
    cleaner/               cache/risk inspector (preview only)
    activity/              local scan history
    settings/              appearance, scan scope, exclusions
  lib/                     typed IPC client, formatting, treemap math, filters, risk model
  styles/                  design tokens and shared layout CSS
src-tauri/                 Tauri + Rust backend
  src/apps.rs              Windows uninstall-registry discovery
  src/fs.rs                native directory enumeration (FileIdBothDirectoryInfo)
  src/mft.rs               NTFS MFT reader with a bounded record budget
  src/scan.rs              streaming walker, progress, cancellation, aggregation
  src/duplicates.rs        size buckets, partial + full SHA-256 verification
  src/storage.rs           category, age-bucket and insight classification
  src/caches.rs            developer/AI/Windows cache catalog and measurement
  src/volumes.rs           local volume capacity
  src/history.rs           app-data scan history (max 100 entries)
  src/commands.rs          validated Tauri command layer
  src/lib.rs               app state, dialog plugin, command registration
  tests/real_system.rs     real registry/cache/scan integration test
  tests/read_only.rs       destructive-API audit, junction and hard-link safety tests
  tests/stress_scan.rs     wide/deep/denied/cancellation stress tests
  examples/scan_bench.rs   benchmark binary
scripts/                   MSVC environment shim, build/smoke/benchmark checks
docs/                      architecture, status and benchmark documents
```

## Safety guarantees

- **No user-data cleanup or uninstall path.** There is no command to delete arbitrary files,
  uninstall software, terminate processes or write registry values. `clear_history` removes only
  Cyron's own `history.json`; temp-file cleanup is limited to files created by Cyron. The Cleaner
  renders findings without a removal action.
- **No arbitrary paths.** Scan, duplicate-scan and reveal commands require a non-empty absolute
  path that exists (and is a directory for scans), then canonicalize it before use. Job ids are
  restricted to `[A-Za-z0-9_-]` and a running id cannot be replaced.
- **Junctions and symlinks are never followed.** Reparse points are counted and skipped, so a scan
  cannot loop or escape its root.
- **Hard links are deduplicated for allocated space** when native enumeration supplies file ids.
  The walk tracks up to one million ids and says so when it stops retaining new ones; the MFT
  reader counts each record once.
- **Bounded scanning.** The walk streams directory entries, retains at most 200 children per folder,
  100,000 tree nodes, 40 file nodes per folder, 4,096 extension keys, 1,000,000 hard-link ids and
  the top 100 largest files. The MFT reader rejects a volume above its 768 MiB record budget
  (4,910,404 records) before allocating and polls cancellation while parsing. Cancellation, depth
  and exclusions are supported. These are structural bounds; the reported totals still cover the
  whole walk.
- **Local only.** No network calls, no telemetry, no uploads. Scan results stay in memory;
  activity history and settings stay in this Windows profile.
- **No elevation requested.** Launching without administrator rights is supported and uses the
  directory walk; unreadable folders are reported rather than crashing the scan.

## Known limitations

- **Win32 registry inventory only.** Windows Store/MSIX apps and portable programs are not
  enumerated yet. Sizes are the installer's `EstimatedSize` when present, otherwise unknown; they
  are never fabricated. Use *Measure folder* for the real size of an install directory.
- **MFT needs elevation, NTFS and a drive root to pay off.** Without administrator rights the
  scanner reports the reason and uses the directory walk. MFT reads the whole volume's record set,
  so a subfolder scan is faster with the walk; Cyron only auto-selects MFT for drive roots.
- **MFT and walk totals differ by counting rule.** MFT counts one entry per file record; the walk
  counts paths and dedupes hard links while its id set has room. A file with several hard links is
  one file to MFT and several paths to the walk.
- **The IPC tree is pruned.** At most 200 children per folder, 8 levels deep, and up to 40
  individual file nodes per folder plus aggregate groups are returned. Totals cover the full walk;
  extension stats return the top 24 and largest files the top 100.
- **Duplicate results are capped and candidate-based.** Default minimum size 1 MiB, at most
  1,000,000 candidates, 200 groups and 50 files per group; the report is marked `truncated` when a
  cap is hit. Files that change during hashing are reported rather than trusted.
- **Storage categories are heuristics.** Known folders are marked `known`, extension-based
  classification is marked `inferred`, and a category with any inferred bytes is not fully known.
- **Benchmarks are warm-cache and single-machine.** Cold-cache timing, CPU time, throughput and
  comparison with other products are not measured.
- **Network and optical drives are skipped** when listing volumes, so an offline mapped drive
  cannot freeze the overview.
- **Fixed cache catalog.** Locations are discovered only from the built-in catalog; it does not
  learn new tool paths. Individual projects and custom install locations are not scanned.
- **No duplicate management or user-data cleanup.** Phase 2 reports; it does not act. The current
  `clear_history` action only removes Cyron's own log.
- **Activity history covers disk scans only.** App, cache and duplicate inventory lookups are not
  logged; the per-profile file is `<Tauri app_data_dir>/history.json`, capped at 100 entries.
  Settings use `localStorage["cyron.settings.v1"]`.
- **Windows only.** The Rust backend calls Win32 APIs unconditionally.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — components, scanner pipeline, IPC surface and
  data flows.
- [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md) — Phase 1 and Phase 2 acceptance
  status, evidence and limits.
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md) — measured scan results and caveats.
- [DESIGN.md](DESIGN.md) — tokens, themes, component anatomy, treemap palette, motion and
  accessibility rules.
- [docs/SECURITY.md](docs/SECURITY.md) — threat model, scanner guarantees and the gates required
  before destructive features.
- [docs/BUNDLE.md](docs/BUNDLE.md) — build, installer targets, signing status and app-data paths.
- [ROADMAP.md](ROADMAP.md) — phased plan to a complete uninstaller/cleaner.
- [AGENTS.md](AGENTS.md) — canonical repository memory for contributors and agents.

## Project status

Phase 1 and Phase 2 are implemented. The debug desktop build and launch are verified; a native
visual pass across all six views in both themes has been performed against the dev-server-backed
window. Release packaging and a standalone (non-dev-server) build are unverified. Phase 3 adds safe
cleaning, Phase 4 the uninstall engine and Phase 5 packaging and hardening. No license file has
been added to the repository yet.
