# Cyron — Release Checklist & Engineering Guide

This document describes the release engineering steps, verification gates, installer packaging targets, signing protocols, and upgrade compatibility requirements for shipping Cyron Uninstaller.

---

## 1. Pre-Release Verification Gates

Every public release candidate must satisfy all verification gates before packaging:

- [ ] **Typecheck**: `npm run typecheck` reports 0 diagnostics.
- [ ] **Frontend Test Suite**: `npm test` passes all Vitest test suites.
- [ ] **Documentation Integrity**: `npm run docs:check` passes with 0 link or consistency errors.
- [ ] **Frontend Production Build**: `node scripts/verify-dist.mjs` outputs `DIST_OK`.
- [ ] **Rust Core Test Suite**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` passes all tests with 0 failures.
- [ ] **Destructive Safety Boundaries**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture` passes all boundary audit tests.
- [ ] **Cleanup & Recovery Stress**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture` passes all stress tests.
- [ ] **Isolated Cleanup Fixture**: `node scripts/verify-cleanup.mjs` outputs `CLEANUP_FIXTURE_OK`.
- [ ] **End-to-End Fixture**: `node scripts/verify-e2e.mjs` outputs `RC1_E2E_FIXTURE_OK`.
- [ ] **Competitive Benchmark**: `node scripts/benchmark-competitive.mjs` outputs `BENCHMARK_OK`.
- [ ] **Desktop Debug Binary**: `node scripts/verify-desktop-binary.mjs` outputs `DESKTOP_BINARY_OK`.
- [ ] **Desktop Smoke Test**: `node scripts/smoke-desktop.mjs` outputs `DESKTOP_SMOKE_OK`.
- [ ] **Release Installer Bundles**: `node scripts/verify-release-bundles.mjs` outputs `RELEASE_BUNDLES_OK`.
- [ ] **Updater Artifacts**: build with `npm run build:release` and `TAURI_SIGNING_PRIVATE_KEY` set; `node scripts/generate-updater-manifest.mjs --tag v<version>` writes `latest.json`; `npm run verify:updater` outputs `UPDATER_ARTIFACTS_OK`.
- [ ] **Windows Metadata Consistency**: `npm run verify:metadata` outputs `WINDOWS_METADATA_OK`.
- [ ] **Authenticode Signature & Negative Integrity**: `npm run verify:signatures` outputs `SIGNATURES_VERIFIED_OK` for a signed release. An `UNSIGNED_RC` build is expected to fail with `NotSigned` and must not be published as trusted.
- [ ] **NSIS Installer Exercise**: install `/S`, relaunch, repair `/S`, uninstall `/S`, reinstall `/S`; confirm the HKCU uninstall entry, install directory and `%APPDATA%\dev.cyron.uninstaller` survive/behave as expected. The 2026-11-17 run passed; see [the installer receipt](evidence/2026-11-17-installer-release.md).
- [ ] **MSI Elevation Gate**: the WiX package is per-machine. An unelevated `/qn` run correctly returns 1603 / error 1925; an elevated install/uninstall remains required before claiming MSI support.

---

## 2. Release Classifications & Code Signing Protocols

Every build artifact belongs strictly to one of two release classifications:

| Classification | Signature Status | Intended Target | Verification Command |
|---|---|---|---|
| `UNSIGNED_RC` | Unsigned | Internal private testing | `node scripts/verify-release-bundles.mjs` |
| `CHRYYPT_SIGNED` | Signed (`CN=Chryypt Organization`) | Official releases & installers | `npm run verify:signatures` |
| `UPDATER_SIGNED` | Minisign signature from the Tauri updater key | In-app update packages | `npm run verify:updater` |

### Chryypt Organization Signing (`CHRYYPT_SIGNED`)
```powershell
# Build and sign with Chryypt Organization certificate
npm run build:signed

# Verify cryptographic file integrity & certificate thumbprint
npm run verify:signatures
```

---

## 3. Installer Packaging & Targets

Cyron generates two distinct Windows installer packages via Tauri v2 bundler:

1. **NSIS Setup Installer (`.exe`)**:
   - Location: `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_<version>_x64-setup.exe`
   - Configured for per-user installation without mandatory elevation.
   - Embeds application metadata, uninstaller hooks, and desktop/start menu shortcuts.
2. **WiX Windows Installer Package (`.msi`)**:
   - Location: `src-tauri/target/release/bundle/msi/Cyron Uninstaller_<version>_x64_en-US.msi`
   - Configured for per-machine enterprise deployment and Windows Installer management; installation requires administrator elevation.
   - UpgradeCode is permanently locked to `{C2BCD648-205F-5904-9E5C-85726E376EC2}` for in-place upgrades. The `0.1.0` MSI was not installed or upgrade-tested; only its metadata and unelevated refusal were verified.
3. **Standalone Portable Executable**:
   - Location: `src-tauri/target/release/cyron.exe`

---

## 4. Checksums & Release Assets

For every release build, generate standard SHA-256 checksums using `scripts/generate-checksums.mjs`:

```powershell
node scripts/generate-checksums.mjs
```

Build the release feed with `npm run build:release`, then generate the manifest and checksums:

```powershell
node scripts/generate-updater-manifest.mjs --tag v<version>
npm run verify:updater
node scripts/generate-checksums.mjs --output src-tauri/target/release/bundle/SHA256SUMS.txt --require-updater
```

Attach the following verified assets to the GitHub Release:
1. `Cyron Uninstaller_<version>_x64-setup.exe` (NSIS Installer and updater package)
2. `Cyron Uninstaller_<version>_x64_en-US.msi` (MSI Package)
3. `cyron.exe` (Standalone Portable Executable)
4. `latest.json` (Signed Tauri updater manifest)
5. `SHA256SUMS.txt` (Cryptographic verification checksums)

Do not publish `latest.json` without the matching NSIS setup and `.sig`; the in-app updater rejects
an unsigned or mismatched package. The manifest must expose only `windows-x86_64-nsis`, never a
generic `windows-x86_64` or `windows-x86_64-msi` entry, so MSI installations are not replaced by the
per-user NSIS package. The GitHub release workflow requires `TAURI_SIGNING_PRIVATE_KEY` (and
optionally `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) in repository secrets.

---

## 5. Upgrade & App Data Preservation Verification

During in-place upgrades:
- Cyron stores persistent data in `%APPDATA%\dev.cyron.uninstaller\`:
  - `history.json` (Scan history)
  - `cleanup-log.json` (Audit log of cleanup runs)
  - `quarantine\` (Active quarantine batches and recovery journals)
- The installer must never overwrite or delete this directory during an upgrade.
- Verify upgrading from version `N` to `N+1`:
  1. Populate mock scan history and a quarantine batch in `%APPDATA%\dev.cyron.uninstaller\`.
  2. Run the `N+1` installer.
  3. Launch Cyron and verify that history, quarantine batches, and settings remain intact.
- **Current evidence:** the `0.1.0` NSIS same-version repair/reinstall preserved 0 app-data hashes and quarantine sentinels. A real `N` → `N+1` upgrade passed on 2026-11-26: a fresh 0.2.1 installation took the published 0.2.2 setup and then updated to 0.2.3 in-app with history and quarantine intact (G134); a downgrade passed on 2026-11-26 (G135), and only a clean-VM run remains an open release gate. The 0.2.4 feed is published and its downloaded setup, MSI, executable and manifest verify against `SHA256SUMS.txt` (G137); the 0.2.2 → 0.2.3 in-app step also passed on its own (G133). The in-app 0.2.3 → 0.2.4 update remains open. Existing 0.2.1 installations need one manual 0.2.2 install before future in-app updates are available.
