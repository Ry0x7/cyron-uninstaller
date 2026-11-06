# Windows build and bundle notes

## Build commands

Run these commands from the repository root in PowerShell:

```powershell
npm install
npm run typecheck
npm test
npm run build:desktop
```

`npm run build:desktop` invokes Tauri in debug mode with `--no-bundle`. The executable is
`src-tauri/target/debug/cyron.exe`; this is a development build, not an installer. A debug build
targets Tauri's `devUrl`, so it expects the Vite dev server (`npm run dev`) to be running; without
it the window shows a connection error. Release builds embed the frontend assets instead.

To build the configured release installers:

```powershell
npm run tauri -- build
```

The Tauri bundle config targets **MSI** and **NSIS**, with NSIS configured for a per-user
installation. This command builds the release app and bundles installers; the first bundle run
may need to download WiX and NSIS tooling. The app requires the Visual Studio 2022 C++ Build
Tools workload and Windows SDK for the MSVC linker. Use Node.js 22+ and Rust stable with the
`x86_64-pc-windows-msvc` target. The repository's npm Tauri commands use `scripts/msvc-env.mjs`
to locate the Visual Studio toolchain when it is not already on `PATH`.

## Runtime and artifacts

Cyron uses the Microsoft Edge **WebView2 Runtime**. It is included on Windows 11 and is normally
available on supported Windows 10 installations; a machine without it needs the runtime
installed before the desktop app can render its UI.

The bundle configuration references icons in `src-tauri/icons/`; all configured icon files are
present. The debug executable was built and verified locally. **Release MSI/NSIS packaging has
not been run or verified.** No signed binary or installer has been produced; outputs are
unsigned and Windows may display a publisher warning.

## User data

Activity history is kept locally at `history.json` under Tauri's per-application data directory
(Windows convention: `%APPDATA%\dev.cyron.uninstaller\history.json`). The history log is capped
at 100 disk-scan entries. Appearance and scan settings are stored in the WebView's local storage
under `cyron.settings.v1`. Cyron has no telemetry or network service; deleting history removes
only that log and does not modify scanned files.

## Signing checklist for a future release

- Obtain and protect a code-signing certificate and its private key; do not commit either to the
  repository.
- Decide the release identity and publisher name, and configure the Windows installer metadata
  consistently.
- Sign the application executable and each installer with the approved certificate and a
  trusted timestamp service in a controlled release pipeline.
- Verify signature chains and timestamps on the produced artifacts, and test SmartScreen and
  installation behavior on clean Windows 10/11 machines.
- Publish hashes and retain the exact signed release artifacts and build provenance.

Signing infrastructure is not configured in this repository. No signing or clean-machine
installer test is claimed.
