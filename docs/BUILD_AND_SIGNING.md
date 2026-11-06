# Cyron — Build, Packaging & Code Signing Guide

This document describes how to compile, package, sign, and verify Cyron Uninstaller on Windows 10/11.

---

## 1. Prerequisites & Toolchain Setup

Cyron is built with **Tauri v2**, **Rust**, and **React 19 / TypeScript / Vite 7**. It runs exclusively on 64-bit Windows and invokes native Win32 APIs directly.

### Required Software:
- **Windows 10 (1809+) or Windows 11** (x64)
- **Node.js 22 LTS** and `npm`
- **Rust stable (1.77+)** targeting `x86_64-pc-windows-msvc`
- **Visual Studio 2022 Build Tools** with the *Desktop development with C++* workload
- **Windows 10/11 SDK** (provides `signtool.exe` and Win32 headers)
- **Evergreen Microsoft Edge WebView2 Runtime** (pre-installed on Windows 11 and current Windows 10)

> [!IMPORTANT]
> Git for Windows bundles a coreutils `link.exe` that shadows the Microsoft Visual C++ linker.
> To prevent linker collisions, all compilation scripts and npm build commands use `node scripts/msvc-env.mjs`, which locates Visual Studio via `vswhere` and configures the environment automatically.

---

## 2. Build & Signing Commands

Run these commands from the repository root in PowerShell:

```powershell
# 1. Install dependencies
npm install

# 2. Validate types and run test suites
npm run typecheck
npm test
npm run docs:check

# 3. Development desktop window (debug build, loads Vite dev server)
npm run dev:desktop

# 4. Standalone debug binary (no installer bundling)
npm run build:desktop

# 5. Build unsigned production release bundles (NSIS setup & WiX MSI)
npm run build:unsigned

# 6. Build and sign release bundles with local development certificate
npm run build:dev-signed

# 7. Verify Authenticode cryptographic integrity & run negative checks
npm run verify:signatures

# 8. Verify Windows PE VERSIONINFO, MSI properties, and NSIS headers
npm run verify:metadata
```

### Build Types Explained:
- **`npm run build:desktop`**: Invokes `tauri build --debug --no-bundle`. Emits `src-tauri/target/debug/cyron.exe`. Because this is a debug build, it targets Tauri's `devUrl` and expects Vite (`npm run dev`) to serve frontend assets.
- **`npm run build:unsigned`**: Builds optimized release binaries with embedded frontend assets into NSIS and WiX MSI installers under `src-tauri/target/release/bundle/`. Artifacts are unsigned (`UNSIGNED_RC`).
- **`npm run build:dev-signed`**: Builds release bundles and invokes `scripts/sign-dev-build.ps1` to sign `cyron.exe`, NSIS setup, and WiX MSI packages with the local development certificate (`DEV_TEST_SIGNED`).

---

## 3. Three-Tier Release Classification Architecture

Cyron enforces three mutually exclusive build and release classifications:

| Build Classification | Signature Status | Intended Audience | Distribution Scope |
|---|---|---|---|
| `UNSIGNED_RC` | None | Maintainers & core developers | Private testing; verify SHA-256 against `SHA256SUMS.txt`. |
| `DEV_TEST_SIGNED` | Local self-signed certificate (`CN=Cyron Uninstaller Development`) | Developers & isolated test VMs | Strictly for local evaluation; never distributed as general public releases. |
| `SIGNPATH_SIGNED` | Publicly trusted Authenticode certificate via SignPath Foundation | General public | Official public releases after SignPath Foundation approval. |

---

## 4. Local Development Signing (`DEV_TEST_SIGNED`)

For local feature development, installer lifecycle testing, and regression verification, Cyron provides an automated self-signed pipeline:

### Certificate Specification:
- **Subject**: `CN=Cyron Uninstaller Development`
- **Store Location**: `Cert:\CurrentUser\My`
- **Key Algorithm**: RSA 3072-bit, SHA-256
- **EKU**: Code Signing (`1.3.6.1.5.5.7.3.3`)
- **Key Protection**: Non-exportable private key where supported

### Lifecycle Automation:
- **`scripts/create-dev-cert.ps1`**: Idempotently finds an existing valid certificate matching the subject and thumbprint or generates a fresh one in `Cert:\CurrentUser\My`. It never touches machine root stores.
- **`scripts/sign-dev-build.ps1`**: Uses the official Windows SDK `signtool.exe` to sign `cyron.exe`, the NSIS setup `.exe`, and the WiX `.msi` package with SHA-256 digests.

### Hard Security Restrictions for Development Certificates:
1. **No Root Store Tampering**: Never automatically install development certificates into `Trusted Root Certification Authorities` or `Trusted Publishers` on developer or end-user machines.
2. **No Policy Circumvention**: Never modify Defender SmartScreen or Windows 11 Smart App Control policies to simulate signing success.
3. **No Private Key Distribution**: No private key or PFX file is ever committed to Git or uploaded to CI runners.
4. **SmartScreen Reality**: Self-signed certificates cannot eliminate Windows SmartScreen warnings on third-party computers. Reputation requires public CA telemetry and download history.

---

## 5. Public Release Signing via SignPath Foundation (`SIGNPATH_SIGNED`)

Cyron prepares for free, publicly trusted Authenticode signing through the [SignPath Foundation](https://signpath.org/):

- **Workflow Definition**: `.github/workflows/signpath-release.yml` (disabled by default until project onboarding).
- **Security Gates**: Runs full test suites, generates unsigned SHA-256 provenance hashes, submits signing requests via `signpath/github-action-submit-signing-request`, requires manual maintainer approval, and verifies returned Authenticode chains using `signtool.exe`.
- **Readiness Tracking**: Documented requirement-by-requirement in [docs/SIGNPATH_READINESS.md](SIGNPATH_READINESS.md).
- **Code Signing Policy**: Documented in [docs/CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md).

---

## 6. Windows Metadata Standards & Audit

Windows metadata is enforced consistently across all artifacts and audited automatically by `scripts/verify-windows-metadata.ps1` (`WINDOWS_METADATA_OK`):

- **Product Name**: `Cyron Uninstaller`
- **File Description**: `Windows Application Uninstaller, Disk Analyzer & Cleaner`
- **Version**: Derived dynamically from `package.json` and `tauri.conf.json` (`0.1.0`)
- **Original Filename**: `cyron.exe`
- **Publisher**: `Chryypt`
- **Copyright**: `Copyright © 2026 Chryypt`
- **Application Identifier**: `dev.cyron.uninstaller`
- **MSI UpgradeCode**: `{C2BCD648-205F-5904-9E5C-85726E376EC2}`

### Generated Release Bundles:
- **NSIS Setup**: `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.1.0_x64-setup.exe`
- **WiX MSI Package**: `src-tauri/target/release/bundle/msi/Cyron Uninstaller_0.1.0_x64_en-US.msi`
- **Standalone Binary**: `src-tauri/target/release/cyron.exe`

---

## 7. Cryptographic Verification & Automated Negative Tests

`scripts/verify-signatures.ps1` separates **cryptographic digest integrity** from **root certificate trust**:

1. **Digest Verification**: Confirms that the embedded Authenticode signature matches the binary's actual SHA-256 hash. If any byte is altered, verification fails immediately.
2. **Signer Identity Check**: Asserts that the signing certificate subject matches `CN=Cyron Uninstaller Development` (or `SignPath Foundation` for public releases).
3. **Automated Negative Test Suite** (`-RunNegativeTests`):
   - Proves unsigned files are rejected.
   - Proves tampered signed files fail cryptographic hash validation.
   - Proves binaries signed with mismatched certificate subjects are rejected.

---

## 8. Controlled Testing in an Isolated VM

To test installer UAC dialogs, elevation prompts, and Windows SmartScreen presentation without affecting developer host workstations:

1. **Export Public Certificate** (public key only, no private key):
   ```powershell
   $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq "CN=Cyron Uninstaller Development" } | Select-Object -First 1
   Export-Certificate -Cert $cert -FilePath "$env:TEMP\cyron-dev-public.cer"
   ```
2. **Import into Disposable VM Root Store** (ONLY inside an isolated VM):
   ```powershell
   Import-Certificate -FilePath "cyron-dev-public.cer" -CertStoreLocation "Cert:\LocalMachine\Root"
   ```
3. **Verify Installer**: In the test VM, `Get-AuthenticodeSignature` will report `Status: Valid`, and UAC prompts will display `Cyron Uninstaller Development` as a verified publisher.
4. **Revert VM Snapshot**: Always revert or discard the test VM after testing.

---

## 9. Runtime Data & Storage Locations

Cyron strictly separates its application state from browser rendering caches:

- **Application Data Directory (`app.path().app_data_dir()`)**:
  - Located at `%APPDATA%\dev.cyron.uninstaller\` (`FOLDERID_RoamingAppData`).
  - Contains:
    - `history.json`: Local disk scan history (capped at 100 entries).
    - `cleanup-log.json`: Audit log of cleanup and quarantine operations.
    - `quarantine\`: Active quarantine batches, manifests, and write-ahead recovery journals (`recovery_journal.jsonl`).
  - This directory is preserved across application upgrades.
- **WebView2 Browser Cache**:
  - Located at `%LOCALAPPDATA%\dev.cyron.uninstaller\EBWebView\` (`FOLDERID_LocalAppData`).
  - Managed exclusively by the Microsoft Edge WebView2 runtime for web rendering caches, GPU caches, and local storage.
- **Zero Telemetry**: Cyron contains no network communication code, analytics, or background telemetry.
