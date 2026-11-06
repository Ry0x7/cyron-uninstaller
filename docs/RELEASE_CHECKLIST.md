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
- [ ] **Windows Metadata Consistency**: `npm run verify:metadata` outputs `WINDOWS_METADATA_OK`.
- [ ] **Authenticode Signature & Negative Integrity**: `npm run verify:signatures` outputs `SIGNATURES_VERIFIED_OK`.

---

## 2. Release Classifications & Code Signing Protocols

Every build artifact belongs strictly to one of three release classifications:

| Classification | Signature Status | Intended Target | Verification Command |
|---|---|---|---|
| `UNSIGNED_RC` | Unsigned | Internal private testing | `node scripts/verify-release-bundles.mjs` |
| `DEV_TEST_SIGNED` | Self-Signed (`CN=Cyron Uninstaller Development`) | Developer workstations & isolated test VMs | `npm run verify:signatures` |
| `SIGNPATH_SIGNED` | Publicly Trusted CA (`SignPath Foundation`) | General public releases (after approval) | `.github/workflows/signpath-release.yml` |

### A. Local Development Signing (`DEV_TEST_SIGNED`)
```powershell
# Build and sign with local self-signed certificate
npm run build:dev-signed

# Verify cryptographic file integrity & certificate thumbprint
npm run verify:signatures
```

### B. SignPath Foundation Public Signing (`SIGNPATH_SIGNED`)
Triggered manually via `.github/workflows/signpath-release.yml` once SignPath Foundation eligibility criteria ([docs/SIGNPATH_READINESS.md](SIGNPATH_READINESS.md)) are approved. SignPath submits requests through `signpath/github-action-submit-signing-request`, requires manual approval, and verifies returned Authenticode chains before drafting releases.

---

## 3. Installer Packaging & Targets

Cyron generates two distinct Windows installer packages via Tauri v2 bundler:

1. **NSIS Setup Installer (`.exe`)**:
   - Location: `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_<version>_x64-setup.exe`
   - Configured for per-user installation without mandatory elevation.
   - Embeds application metadata, uninstaller hooks, and desktop/start menu shortcuts.
2. **WiX Windows Installer Package (`.msi`)**:
   - Location: `src-tauri/target/release/bundle/msi/Cyron Uninstaller_<version>_x64_en-US.msi`
   - Configured for enterprise deployment and Windows Installer management.
   - UpgradeCode is permanently locked to `{C2BCD648-205F-5904-9E5C-85726E376EC2}` to ensure seamless in-place upgrades.
3. **Standalone Portable Executable**:
   - Location: `src-tauri/target/release/cyron.exe`

---

## 4. Checksums & Release Assets

For every release build, generate standard SHA-256 checksums using `scripts/generate-checksums.mjs`:

```powershell
node scripts/generate-checksums.mjs
```

Attach the following verified assets to the GitHub Release:
1. `Cyron Uninstaller_<version>_x64-setup.exe` (NSIS Installer)
2. `Cyron Uninstaller_<version>_x64_en-US.msi` (MSI Package)
3. `cyron.exe` (Standalone Portable Executable)
4. `SHA256SUMS.txt` (Cryptographic verification checksums)

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
