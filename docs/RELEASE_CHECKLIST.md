# Cyron — Release Checklist & Release Engineering Guide

This document describes the release engineering steps, verification gates, installer generation, signing protocols, and upgrade compatibility requirements for shipping Cyron.

---

## 1. Pre-Release Verification Gates

Every public release candidate must satisfy all acceptance gates before packaging:

- [ ] **Typecheck**: `npm run typecheck` reports 0 diagnostics.
- [ ] **Frontend Tests**: `npm test` passes all 71+ vitest suites across all components and API layers.
- [ ] **Frontend Production Build**: `node scripts/verify-dist.mjs` outputs `DIST_OK`.
- [ ] **Rust Core Unit Tests**: `cargo test --manifest-path src-tauri/Cargo.toml` passes all 120+ unit tests with 0 failures.
- [ ] **Destructive Safety Boundaries**: `cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture` passes all 22 boundary audit tests.
- [ ] **Cleanup & Recovery Stress**: `cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture` passes all tests.
- [ ] **Isolated Cleanup Fixture**: `node scripts/verify-cleanup.mjs` outputs `CLEANUP_FIXTURE_OK`.
- [ ] **Desktop Debug Binary**: `node scripts/verify-desktop-binary.mjs` outputs `DESKTOP_BINARY_OK`.
- [ ] **Desktop Smoke Test**: `node scripts/smoke-desktop.mjs` outputs `DESKTOP_SMOKE_OK`.
- [ ] **Git History Privacy Audit**: Confirm no unredacted internal session data exists in pending commits.

---

## 2. Installer Packaging & Targets

Cyron ships two official installer formats for Windows:

### A. NSIS (`.exe`)
- Target: `src-tauri/target/release/bundle/nsis/Cyron_x.y.z_x64-setup.exe`
- Mode: Per-user (`currentUser`) or system-wide.
- Upgrade contract: In-place upgrades preserve `%APPDATA%\dev.cyron.uninstaller` (`history.json`, `quarantine/`, `cleanup-log.json`).
- Uninstaller contract: Removes application binaries; prompts user before removing quarantine or scan history.

### B. WiX MSI (`.msi`)
- Target: `src-tauri/target/release/bundle/msi/Cyron_x.y.z_x64_en-US.msi`
- Enterprise-ready deployment package with standard Windows Installer properties (`ADDLOCAL`, `ALLUSERS`).

### Building Bundles Locally
```powershell
node scripts/msvc-env.mjs npx tauri build --bundles nsis,msi
```

---

## 3. Authenticode Code Signing

> [!WARNING]
> An unsigned binary must NEVER be described as signed. When signing credentials are not provided, installers must be clearly documented as unsigned release candidates.

When valid code-signing credentials are available:
1. Export the Authenticode certificate to a PFX file or hardware security module (HSM).
2. Configure environment variables before triggering the release build:
   - `CSC_LINK`: Path to `.pfx` certificate.
   - `CSC_KEY_PASSWORD`: PFX passphrase.
   - `TIMESTAMP_SERVER`: `http://timestamp.digicert.com` (or RFC 3161 compliant timestamp server).
3. Verify signature:
   ```powershell
   signtool verify /pa /v "src-tauri/target/release/bundle/nsis/Cyron_x.y.z_x64-setup.exe"
   ```

---

## 4. Checksums & Release Assets

For every release build, generate standard SHA-256 checksums:

```powershell
$artifacts = Get-ChildItem -Path "src-tauri/target/release/bundle" -Recurse -Include *.exe,*.msi
$lines = foreach ($item in $artifacts) {
    $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.ToLower()
    "$hash  $($item.Name)"
}
$lines | Out-File -Encoding utf8 -FilePath "SHA256SUMS.txt"
```

Attach the following assets to the GitHub Release:
1. `Cyron_<version>_x64-setup.exe` (NSIS Installer)
2. `Cyron_<version>_x64_en-US.msi` (MSI Package)
3. `SHA256SUMS.txt` (Cryptographic verification checksums)

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
