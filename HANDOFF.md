# Cyron — Active Handoff

Updated: 2026-11-07 (Milestone 7.2: Release-Blocking Correctness & Desktop Validation Phase)

## 1. Current Verified State

- **Branch / Commit:** `main` (incorporating Milestone 7.2 release-blocking correctness)
- **Current Version:** `0.1.0` (Release Candidate 1)
- **Tag / Published Prerelease:** `v0.1.0` at `https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.1.0`
- **Published Release Assets:**
  - `Cyron Uninstaller_0.1.0_x64-setup.exe` (NSIS Installer)
  - `Cyron Uninstaller_0.1.0_x64_en-US.msi` (WiX MSI Installer)
  - `cyron.exe` (Standalone Portable Executable)
  - `SHA256SUMS.txt` (Cryptographic verification manifest)
- **IPC Surface:** 45 registered commands in `src-tauri/src/lib.rs` and `commands.rs` (including `restore_registry_backup`).
- **Git History Audit:** Complete. Clean commit tree verified with zero `.chryypt` session files in Git history. `.chryypt/` is permanently excluded via `.gitignore`. Strict No-Rebase policy enforced via local hooks and GitHub settings.

## 2. Recent Test Evidence

| Verification Target | Command | Result |
|---|---|---|
| TypeScript Typecheck | `npm run typecheck` | Pass (0 diagnostics) |
| Frontend Unit Tests | `npm test` | 78/78 pass across 8 files |
| Frontend Production Build | `node scripts/verify-dist.mjs` | `DIST_OK` |
| Rust Core & Unit Tests | `node scripts/msvc-env.mjs cargo test --lib` | 145/145 pass (2 ignored) |
| Security Boundaries | `... --test security -- --nocapture` | 26/26 pass (includes snapshot isolation) |
| Cleanup Stress Tests | `... --test cleanup_stress -- --nocapture` | 5/5 pass |
| Isolated Cleanup Fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Real External HWND E2E Fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK` (8/8 pass) |
| Hunter Benchmark & Stability | `node scripts/verify-hunter-perf.mjs` | `HUNTER_BENCHMARK_OK` (p50: 737 µs, 0 KB leak) |
| Scanner Stress Suite | `... --test stress_scan -- --nocapture` | 5/5 pass (1 ignored) |
| Total Rust Test Suite | `... cargo test --manifest-path src-tauri/Cargo.toml` | 182 passed, 3 ignored |
| Windows Metadata Audit | `npm run verify:metadata` | `WINDOWS_METADATA_OK` (5/5 pass) |
| Signature Integrity & Negative Tests | `npm run verify:signatures` | `SIGNATURES_VERIFIED_OK` (3/3 pass, 3/3 negative pass) |
| Documentation Integrity | `npm run docs:check` | `DOCS_CHECK: PASSED` |

## 3. Active Policy Blockers & Required Operator Decisions

1. **Prerelease Distribution Conflict:** The public v0.1.0 prerelease on GitHub contains `DEV_TEST_SIGNED` binaries (signed with local `CN=Cyron Uninstaller Development` cert). This conflicts with `docs/BUILD_AND_SIGNING.md` and `docs/CODE_SIGNING_POLICY.md`, which state self-signed builds are strictly for local/VM testing and must not be distributed publicly. Operator decision needed: keep published as an explicitly labeled testing prerelease or unpublish until SignPath signing is secured.
2. **SignPath Foundation Licensing:** Standard SignPath free open-source code signing requires an OSI-approved license. Cyron currently uses a custom Source-Available / No-Reuse license (`LICENSE`). Submitting an application requires either requesting a SignPath exception or an operator decision regarding licensing terms.

## 4. Next Engineering Tasks

1. Maintain documentation synchronization via automated checks (`scripts/verify-docs.mjs` / `npm run docs:check`).
2. Await operator evaluation of the published v0.1.0 installer bundles on Windows 10/11 VMs.
