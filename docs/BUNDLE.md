# Windows Build and Bundle Notes

## 1. Build and Signing Commands

Run these commands from the repository root in PowerShell:

```powershell
npm install
npm run typecheck
npm test

# Standard debug desktop launch
npm run build:desktop

# Build unsigned release bundles
npm run build:unsigned

# Build and sign development release bundles (DEV_TEST_SIGNED)
npm run build:dev-signed

# Verify Authenticode signatures & cryptographic integrity
npm run verify:signatures

# Verify Windows PE metadata, MSI database properties, and NSIS installer headers
npm run verify:metadata
```

`npm run build:desktop` invokes Tauri in debug mode with `--no-bundle`. The executable is
`src-tauri/target/debug/cyron.exe`; this is a development build, not an installer. A debug build
targets Tauri's `devUrl`, expecting the Vite dev server (`npm run dev`) to be running. Release builds embed the frontend assets directly.

The Tauri bundle config targets **WiX MSI** and **NSIS**, with NSIS configured for a per-user
installation and MSI locked to UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`.

---

## 2. Release Bundles and Metadata Verification

Windows metadata is enforced and verified automatically by `scripts/verify-windows-metadata.ps1` (`WINDOWS_METADATA_OK`):
- **Product Name:** `Cyron Uninstaller`
- **File Description:** `Windows Application Uninstaller, Disk Analyzer & Cleaner` (VERSIONINFO / MSI property / NSIS header)
- **Version:** `0.1.0` (derived dynamically from `package.json`)
- **Internal / Original File Name:** `cyron.exe`
- **Publisher:** `Ry0x7`
- **Copyright:** `Copyright © 2026 Ry0x7`
- **Application ID:** `dev.cyron.uninstaller`
- **MSI UpgradeCode:** `{C2BCD648-205F-5904-9E5C-85726E376EC2}`

### Generated Release Bundles:
- **NSIS Setup:** `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.1.0_x64-setup.exe` (1.46 MiB)
- **WiX MSI Package:** `src-tauri/target/release/bundle/msi/Cyron Uninstaller_0.1.0_x64_en-US.msi` (2.04 MiB)

---

## 3. Two-Track Code Signing Architecture

Cyron implements two distinct code signing workflows:

1. **`DEV_TEST_SIGNED` (Active Today):**
   - Self-signed certificate (`CN=Cyron Uninstaller Development`, RSA 3072, SHA-256) generated in `Cert:\CurrentUser\My` via `scripts/create-dev-cert.ps1`.
   - Signed using official Windows SDK `signtool.exe` via `scripts/sign-dev-build.ps1`.
   - Cryptographic digest integrity verified via `scripts/verify-signatures.ps1` (`SIGNATURES_VERIFIED_OK`).
   - Strictly for local development, regression testing, and isolated test VMs.
2. **`SIGNPATH_SIGNED` (Prepared for Public Release):**
   - Free, publicly trusted Authenticode signing via [SignPath Foundation](https://signpath.org/).
   - Automated via `.github/workflows/signpath-release.yml` using `signpath/github-action-submit-signing-request`.
   - Disabled by default pending SignPath Foundation project onboarding and secret configuration.
   - Requirement-by-requirement status tracked in `docs/SIGNPATH_READINESS.md`.

---

## 4. Runtime and User Data

- **WebView2:** Requires the Evergreen Microsoft Edge WebView2 Runtime (pre-installed on Windows 11 and modern Windows 10).
- **User Data:** History log is stored at `%APPDATA%\dev.cyron.uninstaller\history.json` (capped at 100 entries). Quarantine batches and recovery journals are stored at `%LOCALAPPDATA%\dev.cyron.uninstaller\quarantine`.
- **Zero Telemetry:** The application contains no network stack and makes no external connections.
