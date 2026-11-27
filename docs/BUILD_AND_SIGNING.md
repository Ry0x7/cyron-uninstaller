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

# 5a. Build release bundles plus signed updater artifacts and latest.json
#     Requires TAURI_SIGNING_PRIVATE_KEY in the environment.
npm run build:release
node scripts/generate-updater-manifest.mjs --tag v0.2.2

# 6. Build and sign release bundles with Chryypt Organization certificate
npm run build:signed

# 7. Verify Authenticode cryptographic integrity & run negative checks
npm run verify:signatures

# 8. Verify Windows PE VERSIONINFO, MSI properties, and NSIS headers
npm run verify:metadata
```

### Build Types Explained:
- **`npm run build:desktop`**: Invokes `tauri build --debug --no-bundle`. Emits `src-tauri/target/debug/cyron.exe` with embedded production assets; it runs standalone and is the executable used by the desktop E2E suite. The debug profile is not an optimized release build.
- **`npm run build:unsigned`**: Builds optimized release binaries with embedded frontend assets into NSIS and WiX MSI installers under `src-tauri/target/release/bundle/`. Artifacts are Authenticode-unsigned (`UNSIGNED_RC`).
- **`npm run build:release`**: Builds the same bundles with `src-tauri/tauri.release.conf.json`, which enables `bundle.createUpdaterArtifacts`. With `TAURI_SIGNING_PRIVATE_KEY` set, Tauri writes the NSIS setup signature (`*-setup.exe.sig`) used by the in-app updater. This is the release-feed build.
- **`npm run build:signed`**: Builds release bundles and invokes `scripts/sign-build.ps1` to sign `cyron.exe`, NSIS setup, and WiX MSI packages with the Chryypt Organization Authenticode certificate (`CHRYYPT_SIGNED`). Authenticode signing is separate from the Tauri updater signature.

---

## 3. Release Classification Architecture

Cyron classifies build and release artifacts into:

| Build Classification | Signature Status | Intended Audience | Distribution Scope |
|---|---|---|---|
| `UNSIGNED_RC` | None | Maintainers & core developers | Private testing; verify SHA-256 against `SHA256SUMS.txt`. |
| `CHRYYPT_SIGNED` | Self-signed certificate (`CN=Chryypt Organization`) | Local or manually signed builds | Authenticode-signed binaries and installers; cryptographically intact but not publicly trusted. |
| `UPDATER_SIGNED` | Minisign signature from the Tauri updater key | In-app update packages | Verifies the NSIS setup against the committed updater public key; separate from Authenticode trust. |

---

## 4. Chryypt Organization Signing (`CHRYYPT_SIGNED`)

Cyron provides automated signing with a Chryypt Organization **self-signed** certificate. This proves artifact integrity and signer identity, not public trust. The GitHub release workflow now runs `npm run build:release` to create Tauri updater signatures, but it does not Authenticode-sign the published binaries. Published release assets remain unsigned unless a maintainer signs them locally before upload.

The updater uses a separate minisign key pair. Its public key is committed in `tauri.conf.json`; its private key is never committed. Losing the private key prevents future in-app updates for installations that trust the current public key.

### Certificate Specification:
- **Subject**: `CN=Chryypt Organization`
- **Store Location**: `Cert:\CurrentUser\My`
- **Key Algorithm**: RSA 3072-bit, SHA-256
- **EKU**: Code Signing (`1.3.6.1.5.5.7.3.3`)
- **Key Protection**: Non-exportable private key where supported

### Lifecycle Automation:
- **`scripts/create-signing-cert.ps1`**: Idempotently finds an existing valid certificate matching the Chryypt Organization subject or generates a fresh one in `Cert:\CurrentUser\My`. It never touches machine root stores.
- **`scripts/sign-build.ps1`**: Uses the official Windows SDK `signtool.exe` to sign `cyron.exe`, the NSIS setup `.exe`, and the WiX `.msi` package with SHA-256 digests under `CN=Chryypt Organization`.

### Trust Boundary:
- A self-signed signature reports an untrusted root on machines that do not import the public certificate. It does not earn SmartScreen reputation.
- The isolated-VM procedure in section 7 is for testing the publisher name and UAC presentation only; it must never be applied to a user machine.
- Release notes must state whether an artifact is unsigned, self-signed, or publicly trusted.

### Security Standards:
1. **No Root Store Tampering**: Never automatically install development or signing certificates into `Trusted Root Certification Authorities` or `Trusted Publishers` on developer or end-user machines.
2. **No Policy Circumvention**: Never modify Defender SmartScreen or Windows 11 Smart App Control policies to simulate signing success.
3. **No Private Key Distribution**: No private key or PFX file is ever committed to Git or uploaded to public CI runners.

### Updater Signing (`TAURI_SIGNING_PRIVATE_KEY`)

```powershell
# One-time key generation. Keep the private key out of Git.
npm run tauri -- signer generate -w .tauri/cyron-updater.key

# Build the release feed after setting the key path or content.
$env:TAURI_SIGNING_PRIVATE_KEY = (Resolve-Path ".tauri/cyron-updater.key").Path
npm run build:release
node scripts/generate-updater-manifest.mjs --tag v<version>
npm run verify:updater
```

The generated public key is committed in `tauri.conf.json` under `plugins.updater.pubkey`. The
release workflow reads the private key and optional password from repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if the key has no password)

From a checkout that holds the generated key, set the content secret without printing it:

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY < .tauri/cyron-updater.key
# Only if the key has a password:
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

`build:release` writes the signed NSIS setup and its `.sig` next to the normal bundle. The manifest
script embeds that signature and the GitHub release asset URL in `latest.json` under
`windows-x86_64-nsis`; the workflow uploads the setup, MSI, standalone executable, manifest and
checksums. The updater downloads the NSIS setup only after the user confirms, so the per-user
installer path remains the update path. The manifest deliberately has no generic
`windows-x86_64` or `windows-x86_64-msi` entry: Tauri would otherwise hand the per-user NSIS
package to an MSI installation. The app enables the check only for an NSIS bundle; MSI
installations are directed to the release page instead.

> [!IMPORTANT]
> Back up the updater private key. Existing installations trust the public key embedded in the app;
> losing the private key means those installations cannot receive another immutable signed update.

---

## 5. Windows Metadata Standards & Audit

Windows metadata is enforced consistently across all artifacts and audited automatically by `scripts/verify-windows-metadata.ps1` (`WINDOWS_METADATA_OK`):

- **Product Name**: `Cyron Uninstaller`
- **File Description**: `Windows Application Uninstaller, Disk Analyzer & Cleaner`
- **Version**: Derived dynamically from `package.json` and `tauri.conf.json` (`0.2.5`)
- **Original Filename**: `cyron.exe`
- **Publisher**: `Chryypt`
- **Copyright**: `Copyright © 2026 Chryypt`
- **Application Identifier**: `dev.cyron.uninstaller`
- **MSI UpgradeCode**: `{C2BCD648-205F-5904-9E5C-85726E376EC2}`

### Generated Release Bundles:
- **NSIS Setup (current user)**: `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.2.5_x64-setup.exe`
- **WiX MSI Package (per machine)**: `src-tauri/target/release/bundle/msi/Cyron Uninstaller_0.2.5_x64_en-US.msi`
- **Standalone Binary**: `src-tauri/target/release/cyron.exe`
- **Updater signature**: `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.2.5_x64-setup.exe.sig`
- **Updater manifest**: `src-tauri/target/release/bundle/latest.json`

The WiX package is per-machine and its install requires administrator elevation. The NSIS package
installs per user under `%LOCALAPPDATA%\Cyron Uninstaller` and is the verified per-user path.

### Historical Verified Audit Build (2026-11-17, version 0.2.0)

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| NSIS setup | 2,271,846 | `16a378226f6162baa93373830817bb20df65c59e3f9ae589f34b15dbbdec479f` |
| WiX MSI | 3,330,048 | `3f1e64a7594fba9df4fbecfb7537bde36175dd11166ac6fc346ebe88b6612630` |
| `cyron.exe` | 6,918,144 | `95c3f7effec7ce3ff9a922997f8bc3ccb275ead8df88167d07f7dd0268ca20d4` |

`RELEASE_BUNDLES_OK` and `WINDOWS_METADATA_OK` passed. The NSIS package was installed, repaired,
relaunched, uninstalled and reinstalled with app data preserved. All three artifacts were
`NotSigned`, so `verify:signatures` exited 1 by design. See
[the installer receipt](evidence/2026-11-17-installer-release.md).

---

## 6. Cryptographic Verification & Automated Negative Tests

`scripts/verify-signatures.ps1` separates **cryptographic digest integrity** from **root certificate trust**:

1. **Digest Verification**: Confirms that the embedded Authenticode signature matches the binary's actual SHA-256 hash. If any byte is altered, verification fails immediately.
2. **Signer Identity Check**: Asserts that the signing certificate subject matches `CN=Chryypt Organization`.
3. **Automated Negative Test Suite** (`-RunNegativeTests`):
   - Proves unsigned files are rejected.
   - Proves tampered signed files fail cryptographic hash validation.
   - Proves binaries signed with mismatched certificate subjects are rejected.

An unsigned `UNSIGNED_RC` build is expected to fail artifact verification (all three release files
report `NotSigned`). The 2026-11-17 audit also confirmed that the local `CN=Chryypt Organization`
certificate chain reports `UntrustedRoot`, and that the published `v0.1.0` NSIS asset is `NotSigned`.
Passing negative tests do not establish public trust.

---

## 7. Controlled Testing in an Isolated VM

To test installer UAC dialogs, elevation prompts, and Windows SmartScreen presentation without affecting developer host workstations:

1. **Export Public Certificate** (public key only, no private key):
   ```powershell
   $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq "CN=Chryypt Organization" } | Select-Object -First 1
   Export-Certificate -Cert $cert -FilePath "$env:TEMP\chryypt-public.cer"
   ```
2. **Import into Disposable VM Root Store** (ONLY inside an isolated VM):
   ```powershell
   Import-Certificate -FilePath "chryypt-public.cer" -CertStoreLocation "Cert:\LocalMachine\Root"
   ```
3. **Verify Installer**: In the test VM, `Get-AuthenticodeSignature` will report `Status: Valid`, and UAC prompts will display `Chryypt Organization` as a verified publisher.
4. **Revert VM Snapshot**: Always revert or discard the test VM after testing.

---

## 8. Runtime Data & Storage Locations

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
- **No Telemetry**: Cyron contains no analytics, diagnostics uploads or background telemetry. The only network path is the user-initiated updater check described in section 4.
