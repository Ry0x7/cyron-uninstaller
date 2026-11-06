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
present. The debug executable was built and verified locally (`DESKTOP_BINARY_OK`, `DESKTOP_SMOKE_OK`).

**Release MSI and NSIS packaging has been verified locally (`RELEASE_BUNDLES_OK`):**
- **NSIS Setup**: `src-tauri/target/release/bundle/nsis/Cyron_0.1.0_x64-setup.exe` (1.42 MiB, SHA-256: `29d884334a871c310047bc5191c9d2c2b9a1410b7450967943635c419bdcc0e0`).
- **WiX MSI Package**: `src-tauri/target/release/bundle/msi/Cyron_0.1.0_x64_en-US.msi` (2.00 MiB, SHA-256: `29453c42694a87723fe30b4827d4e9a92e52413038e9daded7592fd277271dc8`).

Outputs are currently unsigned release binaries; Windows SmartScreen will display an untrusted publisher warning until an Authenticode certificate is applied.

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
