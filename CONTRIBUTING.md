# Contributing to Cyron

Thank you for your interest in contributing to Cyron!

Cyron is built with **Tauri v2**, **Rust**, and **React 19 / TypeScript / Vite 7**. It runs exclusively on Windows 10/11 and interfaces directly with native Win32 APIs.

---

## Ways to Contribute

- **Report a bug** with the [bug report form](https://github.com/Ry0x7/cyron-uninstaller/issues/new/choose). Include your Cyron and Windows versions.
- **Suggest a feature** with the feature request form. Describe the problem first; Cyron favours review-first, recoverable designs.
- **Improve the docs or the website** in `docs/` and `landing/`.
- **Send a pull request** for a bug fix or an agreed feature. For anything larger than a small fix, open an issue first so we can agree on the approach.
- **Report a vulnerability privately** as described in [SECURITY.md](SECURITY.md), never in a public issue.

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Pull Request Process

1. Fork the repository and create a branch from `main`, for example `fix/search-export-path`.
2. Make the change and follow the safety rules below.
3. Run the checks that apply (see **Verification Suite**) and note the results in the pull request template.
4. Open the pull request against `main`. To pick up newer changes from `main`, merge it into your branch; never rebase (see below).
5. `main` is protected: every change lands through a pull request, the CI checks **Frontend (typecheck, tests, build)** and **Rust core (tests)** must pass, and the maintainer reviews it. Pull requests are merged with a merge commit or squashed.

## Support the Project

Cyron is built in the open by one maintainer. If it saved you some disk space or an afternoon of hunting leftovers, you can support its development on [Ko-fi](https://ko-fi.com/ry0x7). Starring the repository and reporting bugs help just as much.

---

## Prerequisites

1. **Windows 10 / 11** (64-bit).
2. **Visual Studio 2022** with the "Desktop development with C++" workload (MSVC toolchain).
3. **Node.js 22 LTS** and `npm`.
4. **Rust toolchain** (`stable-x86_64-pc-windows-msvc`).

> [!IMPORTANT]
> Git for Windows includes a coreutils `link.exe` that can shadow the Visual Studio linker. Always use `node scripts/msvc-env.mjs` to invoke `cargo`, `tauri`, or compilation tools if you are not running from a Visual Studio Developer Command Prompt.

---

## Development Setup

```powershell
# 1. Install frontend dependencies
npm install

# 2. Run frontend typecheck and tests
npm run typecheck
npm test

# 3. Run Rust unit tests
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml

# 4. Launch Tauri dev window
npm run dev:desktop
```

---

## Architecture & Safety Rules

All contributions must follow these strict safety principles:

1. **Read-Only by Default**:
   Scanning, duplicate detection, disk categorization, and inventory reading must remain strictly read-only.
2. **Review-First Destructive Operations**:
   - Files are moved into quarantine batches, never deleted in-place.
   - Destructive operations require server-side, single-use, expiring confirmation plans (`CleanupPlan`, `UninstallPlan`, `LeftoverPlan`).
   - The frontend never supplies executables, arguments, or raw file paths for destructive operations.
3. **Registry Protection**:
   - Deleting a registry key requires a verified recursive `.reg` backup preserving all value types and subkeys.
   - Shared vendor keys (`Microsoft`, `Google`, `Apple`, `Intel`, etc.) and system roots are strictly protected.
4. **Process Manager Safety**:
   - Processes are matched only by executable image path within the application's verified install directory.
   - Matching by process name alone is strictly forbidden.
   - Windows critical system processes (`csrss.exe`, `explorer.exe`, `services.exe`, etc.) can never be terminated.
5. **No Shell Invocations**:
   - Unregistered executables, shell interpreters (`cmd.exe`, `powershell.exe`, `pwsh`, `bash`), and scripts are rejected.
   - User input is never interpolated into command lines.
6. **No Network Code**:
   Cyron does not make network requests or collect telemetry.

---

## Verification Suite

Before submitting a pull request, run the complete verification suite:

```powershell
# Frontend checks
npm run typecheck
npm test
npm run build
node scripts/verify-dist.mjs

# Rust test suites
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test security -- --nocapture
node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml --test cleanup_stress -- --nocapture
node scripts/verify-cleanup.mjs
node scripts/verify-e2e.mjs
node scripts/benchmark-competitive.mjs

# Desktop build, smoke test, and release bundles
npm run build:desktop
node scripts/verify-desktop-binary.mjs
node scripts/smoke-desktop.mjs
node scripts/verify-release-bundles.mjs
```

---

## Documentation Integrity

Keep documentation accurate and honest:
- Never claim benchmarks, signed binaries, or verification passes that were not actually executed.
- Record any platform limitations or known constraints in `docs/ARCHITECTURE.md` and `HANDOFF.md`.

---

## Git Workflow: Strict No-Rebase Policy

> [!CAUTION]
> **Rebasing is strictly prohibited on Cyron Uninstaller.**
> All shared branches, pull requests, and releases enforce an immutable, merge-only or squash-only history.

### Why Rebasing is Forbidden:
1. **Cryptographic Release Provenance**: Cyron maintains verifiable Authenticode signatures, release hashes, and Git commit SHA records. Rebasing rewrites commit hashes, breaking published signatures, reproducible build verification, and historical audit trails.
2. **Auditability of Safety Gates**: Destructive operation safety (confirmation plans, process protections, registry backups) relies on an unbroken historical ledger.
3. **Branch Protection Enforced**: Force pushes (`git push --force`) and deletions are permanently disabled on `main` via GitHub branch protection.
4. **GitHub Merge Configuration**: Rebase merges (`allow_rebase_merge: false`) are permanently disabled in GitHub repository settings. Pull requests are integrated exclusively using **Create a merge commit** (`--no-ff`) or **Squash and merge**.

### Local Enforcement:
- The repository installs a local `pre-rebase` hook via `npm install` (`npm run prepare` / `scripts/setup-git-hooks.ps1`) that automatically intercepts and blocks `git rebase` commands.
- Local git pull behavior is set to merge (`git config pull.rebase false`).
- To integrate updates from `main` into your feature branch, use `git merge main` (never `git rebase main`).

---

## Contributors

- **Ry0x7** (Creator, Maintainer & Lead Systems Engineer)
- **Gemini** (Advanced Agentic Systems & Implementation)
- **Claude Code** (Systems Hardening & Feature Development)
- **Codex** (Code Intelligence & Implementation)

---

## License and Intellectual Property

Cyron Uninstaller is source-available software with all rights reserved on code reuse. See [LICENSE](LICENSE) for the full legal terms.

- **Public Viewing and Auditing**: You are welcome to view, study, audit, and compile the code locally for personal evaluation.
- **Strict Prohibition on Reuse**: You may **NOT** copy, extract, adapt, incorporate, or reuse any part of this codebase, design, or architecture in any other commercial or non-commercial product, application, library, or derivative project without prior written permission from Ry0x7.
- **Contributions**: Pull requests submitted to this repository grant the project owner the right to incorporate and distribute the changes under the project's license terms.
