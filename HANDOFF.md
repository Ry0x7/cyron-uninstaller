# Cyron — Active Handoff

Updated: 2026-11-06 (Phase 7 Code Signing Architecture, Release Provenance & RC1 Hardening complete)

## Current Status

Phases 1–7, Priority 0 security hardening, Release Candidate 1 (RC1) hardening, and the two-track code signing architecture are implemented and verified across the Rust backend and React desktop interface.

### Capabilities Delivered in Phase 7:
1. **Windows Metadata Normalization & Automated Audit**:
   - Enforced consistent metadata across all builds: Product Name (`Cyron Uninstaller`), Version dynamically derived from `package.json` (`0.1.0`), Publisher (`Ry0x7`), UpgradeCode (`{C2BCD648-205F-5904-9E5C-85726E376EC2}`), and Internal/Original Executable Name (`cyron.exe`).
   - Implemented `scripts/verify-windows-metadata.ps1` (`npm run verify:metadata`) to audit PE VERSIONINFO, WiX MSI database properties, NSIS installer headers, and app identity.
2. **Local Self-Signed Development Builds (`DEV_TEST_SIGNED`)**:
   - Automated certificate lifecycle via `scripts/create-dev-cert.ps1` (`CN=Cyron Uninstaller Development`, RSA 3072, SHA-256, NonExportable) in `Cert:\CurrentUser\My` with idempotent reuse.
   - Signing via official Windows SDK `signtool.exe` (`scripts/sign-dev-build.ps1`), signing `cyron.exe`, NSIS setup, and WiX MSI packages.
   - `npm run build:dev-signed` builds release packages and signs them automatically.
3. **Authenticode Signature Integrity & Automated Negative Testing**:
   - Implemented `scripts/verify-signatures.ps1` (`npm run verify:signatures`), separating cryptographic file integrity from root trust.
   - Includes automated negative testing suite asserting rejection of unsigned files, tampered binaries (hash mismatch), and rogue certificate subjects.
4. **SignPath Foundation Preparation (`SIGNPATH_SIGNED`)**:
   - Authored comprehensive eligibility assessment in `docs/SIGNPATH_READINESS.md` tracking OSI license, public repository, and project reputation prerequisites.
   - Prepared disabled-by-default GitHub Actions workflow (`.github/workflows/signpath-release.yml`) with full test suite gates, unsigned SHA-256 hash provenance, official SignPath request submission action, and Authenticode chain verification.
   - Authored `docs/CODE_SIGNING_POLICY.md` documenting SignPath attribution, maintainer/approver responsibilities, zero-telemetry privacy policy, and user verification instructions.
5. **Git History Privacy Sanitization**:
   - Executed operator directive to purge historical `.chryypt/` data from GitHub history using `git-filter-repo`.

---

## Verification Table

| Check | Command | Result |
|---|---|---|
| TypeScript Typecheck | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend Unit Tests | `npm test` | 71/71 pass across 7 files |
| Frontend Production Build | `node scripts/verify-dist.mjs` | `DIST_OK` (2 assets, 458.8 KiB) |
| Rust Unit & Lib Tests | `node scripts/msvc-env.mjs cargo test --lib` | 134/134 pass, 2 ignored |
| Security Boundaries | `... --test security -- --nocapture` | 25/25 pass |
| Cleanup Stress Suite | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Cleanup Fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| End-to-End Fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK` |
| Real System Scanner | `... --test real_system -- --nocapture` | Pass |
| Scanner Stress Suite | `... --test stress_scan -- --nocapture` | 5/5 pass, 1 ignored |
| Total Rust Test Suite | `... cargo test --manifest-path src-tauri/Cargo.toml` | **171 tests pass**, 3 ignored |
| Desktop Binary | `npm run build:desktop`, `node scripts/verify-desktop-binary.mjs` | `DESKTOP_BINARY_OK` (16.06 MiB) |
| Launch Smoke Test | `node scripts/smoke-desktop.mjs` | `DESKTOP_SMOKE_OK` ("Cyron Uninstaller") |
| Windows Metadata Audit | `npm run verify:metadata` | `WINDOWS_METADATA_OK` (5/5 passed) |
| Authenticode Signatures & Negative Checks | `npm run verify:signatures` | `SIGNATURES_VERIFIED_OK` (3/3 artifacts, 3/3 negative tests) |
| Competitive Benchmarks | `node scripts/benchmark-competitive.mjs` | `BENCHMARK_OK` |
| Release Packaging | `node scripts/verify-release-bundles.mjs` | `RELEASE_BUNDLES_OK` (NSIS: 1.46 MiB, MSI: 2.04 MiB) |
| Remote Windows CI | `.github/workflows/ci.yml` | Pass (Run #42188091108) |

---

## Key Files & Modules

- `src-tauri/src/explorer.rs`: native Windows context menu registration under `HKCU\Software\Classes`, startup routing parser, path validation.
- `src-tauri/src/providers.rs`: declarative cleanup provider system, schema validation, safe environment root checks, malicious definition tests.
- `src-tauri/src/apps.rs`: multi-source application discovery (Win32, Store packages, Steam libraries, and portable heuristics).
- `src-tauri/src/commands.rs`: 34 validated IPC commands.
- `src-tauri/src/process.rs`: process discovery, snapshot tracking, PID creation time checks (`GetProcessTimes`), critical process deny list.
- `src-tauri/src/uninstall.rs`: server-side command resolution, `UninstallPlan` & `BatchUninstallPlan` creation, safe uninstaller tokenization.
- `src-tauri/src/leftovers.rs`: server-side `LeftoverPlan`, bounded non-following directory walker, recursive multi-type `.reg` backup and restore.
- `src-tauri/src/cleanup.rs`: cleanup engine, durable write-ahead recovery journal (`sync_all`), quarantine, atomic no-overwrite restore.
- `src/features/analyzer/Analyzer.tsx`: treemap, folder tree, filter bar, direct Review for Cleanup integration.
- `src/features/uninstaller/Uninstaller.tsx`, `AppDrawer.tsx`: uninstaller UI, source filters, batch queue, process manager, residual reviewer.
- `src/features/settings/Settings.tsx`: Explorer context menu management, safety controls, theme switcher.
- `scripts/benchmark-competitive.mjs`: comparative benchmark runner.
- `docs/USER_GUIDE.md`, `docs/BENCHMARKS.md`, `docs/SECURITY.md`, `docs/ARCHITECTURE.md`: synchronized technical documentation.

---

## Known Considerations

- **Store / MSIX Lifecycle**: Store applications are managed by Windows Package Manager; Cyron displays clear guidance to use Windows Settings or `winget`, while providing read-only inventory, file walk, and leftover detection.
- **Authenticode Signing**: Binaries are unsigned release candidates. Production signing is automated in GitHub Actions once certificate secrets (`CSC_LINK` / `CSC_KEY_PASSWORD`) are supplied.
- **Git History Rewrite**: Purging historical internal session data (`62414cc`) via `git-filter-repo` is documented and awaits explicit operator authorization.
