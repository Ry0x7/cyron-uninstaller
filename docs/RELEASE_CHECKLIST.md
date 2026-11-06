# Cyron — Release Checklist & Release Engineering Guide

This document describes the release engineering steps, verification gates, installer generation, signing protocols, and upgrade compatibility requirements for shipping Cyron.

---

## 1. Pre-Release Verification Gates

Every public release candidate must satisfy all acceptance gates before packaging:

- [ ] **Typecheck**: `npm run typecheck` reports 0 diagnostics.
- [ ] **Frontend Tests**: `npm test` passes all 71 vitest suites across 7 files.
- [ ] **Frontend Production Build**: `node scripts/verify-dist.mjs` outputs `DIST_OK`.
- [ ] **Rust Core Unit Tests**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --lib` passes all 134 unit tests with 0 failures.
- [ ] **Destructive Safety Boundaries**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture` passes all 25 boundary audit tests.
- [ ] **Cleanup & Recovery Stress**: `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture` passes all 5 tests.
- [ ] **Isolated Cleanup Fixture**: `node scripts/verify-cleanup.mjs` outputs `CLEANUP_FIXTURE_OK`.
- [ ] **End-to-End Fixture**: `node scripts/verify-e2e.mjs` outputs `RC1_E2E_FIXTURE_OK`.
- [ ] **Competitive Benchmark**: `node scripts/benchmark-competitive.mjs` outputs `BENCHMARK_OK`.
- [ ] **Desktop Debug Binary**: `node scripts/verify-desktop-binary.mjs` outputs `DESKTOP_BINARY_OK`.
- [ ] **Desktop Smoke Test**: `node scripts/smoke-desktop.mjs` outputs `DESKTOP_SMOKE_OK`.
- [ ] **Release Installer Bundles**: `node scripts/verify-release-bundles.mjs` outputs `RELEASE_BUNDLES_OK`.
- [ ] **Windows Metadata Consistency**: `npm run verify:metadata` outputs `WINDOWS_METADATA_OK`.
- [ ] **Authenticode Signature & Negative Integrity**: `npm run verify:signatures` outputs `SIGNATURES_VERIFIED_OK`.
- [ ] **Git History Privacy Audit**: Purge historical `.chryypt/` data from repository history via `git-filter-repo`.

---

## 2. Release Classifications & Code Signing Protocols

Every build artifact belongs strictly to one of three release classifications:

| Classification | Signature Status | Intended Target | Verification Script |
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
Triggered manually via `.github/workflows/signpath-release.yml` once SignPath Foundation eligibility criteria (`docs/SIGNPATH_READINESS.md`) are approved. SignPath submits requests through `signpath/github-action-submit-signing-request`, requires manual approval, and verifies returned Authenticode chains before drafting releases.

---

## 3. Installer Packaging & Targets

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
