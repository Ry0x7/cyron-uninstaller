# SignPath Foundation Code Signing Readiness Assessment

## 1. Executive Summary

This document evaluates the readiness of **Cyron Uninstaller** for the [SignPath Foundation Free Open Source Code Signing Program](https://signpath.org/apply).

SignPath Foundation provides free Authenticode code signing for open-source projects via a dedicated CA. However, acceptance is governed by strict legal, architectural, and security eligibility criteria.

**Current Overall Status: BLOCKED (Pending Operator Actions & External Application)**

Local development builds are fully functional and cryptographically verifiable today using the `DEV_TEST_SIGNED` self-signed pipeline (`scripts/sign-dev-build.ps1`). Transitioning to `SIGNPATH_SIGNED` public releases is technically prepared but blocked by repository visibility, licensing, and public release history prerequisites.

---

## 2. Requirement-by-Requirement Assessment Matrix

| # | SignPath Foundation Requirement | Cyron Implementation Status | Evaluation | Remediation / Next Steps |
|---|---|---|---|---|
| 1 | **OSI-Approved Open Source License** | Project currently has no committed `LICENSE` file. Dependencies are MIT, Apache-2.0, BSD-3-Clause. | **BLOCKED** | Operator must explicitly approve and commit an OSI license (e.g., MIT or Apache-2.0). |
| 2 | **Public Source Repository** | Repository is currently private on GitHub (`https://github.com/Ry0x7/cyron-uninstaller`). | **BLOCKED** | Operator must explicitly authorize making the GitHub repository public after git history privacy sanitization. |
| 3 | **Git History Privacy Audit** | Historical session metadata (`.chryypt/`) existed in early commits. | **COMPLETED** | Operator issued directive to purge `.chryypt` from GitHub history. |
| 4 | **Established Project Reputation & Release History** | SignPath requires projects to have an established track record, existing releases, and active users before granting free signing. Brand-new repositories are routinely rejected. | **BLOCKED (PREREQUISITE)** | Produce initial `UNSIGNED_RC` / `DEV_TEST_SIGNED` releases to establish project provenance and community visibility before submitting an application. |
| 5 | **Active Maintenance & Product Documentation** | Comprehensive architecture (`docs/ARCHITECTURE.md`), user guide (`docs/USER_GUIDE.md`), roadmap, and test suites are maintained. | **PASS** | Maintain documentation currency across all feature phases. |
| 6 | **Prohibited Software / PUP / Circumvention Policy** | Cyron is an application uninstaller and disk cleaner. SignPath strictly forbids malware, rootkits, PUPs, and security bypass tools. | **PASS** | Cyron enforces review-first execution, single-use server-held confirmation plans, crash-consistent recoverable quarantine, no raw link-following, and protection for OS/critical files. Zero network telemetry. |
| 7 | **Documented Privacy Policy & System Disclosures** | Clear disclosures regarding system modifications (registry backup, quarantine) and zero personal data collection. | **PASS** | Formally documented in `docs/CODE_SIGNING_POLICY.md` and `docs/SECURITY.md`. |
| 8 | **Build Pipeline & Reproducible Automation** | GitHub Actions build pipeline executes full test suite (71 frontend, 170+ Rust, fixtures) and builds binaries in release mode. | **PASS** | Ready for integration with `signpath/github-action-submit-signing-request`. |
| 9 | **GitHub Account Security & MFA** | SignPath requires project maintainers to enforce GitHub two-factor authentication (MFA) and branch protection on release branches. | **PASS (EXTERNAL REQUIREMENT)** | Maintainer must verify 2FA/MFA is enabled on GitHub account `Ry0x7`. |
| 10 | **Accurate Windows Metadata** | PE VERSIONINFO, WiX MSI database properties, and NSIS setup headers match project identity (`Cyron Uninstaller`, version from `package.json`, publisher `Ry0x7`, UpgradeCode locked). | **PASS** | Verified via `npm run verify:metadata` (`WINDOWS_METADATA_OK`). |
| 11 | **Public Code Signing Policy & Attribution** | SignPath terms require attribution to SignPath Foundation and clear maintainer/reviewer signing policies. | **PASS** | Documented in `docs/CODE_SIGNING_POLICY.md`. |

---

## 3. Prerequisite Roadmap Before Application Submission

The following actions must occur before submitting an application to https://signpath.org/apply:

```
[Phase A: Repository Preparation] (Completed)
  ├── Windows metadata normalized & verified (PASS)
  ├── Local dev signing & verification automated (PASS)
  ├── SignPath workflow & policy drafted (PASS)
  └── .chryypt git history purged (PASS)

[Phase B: Operator Authorization Required] (Pending)
  ├── 1. Choose and commit OSI License (e.g. MIT / Apache 2.0)
  ├── 2. Make GitHub repository public (Ry0x7/cyron-uninstaller)
  └── 3. Tag first public release candidate (v0.1.0-rc1)

[Phase C: SignPath Foundation Onboarding] (External)
  ├── 1. Submit application at https://signpath.org/apply
  ├── 2. Await SignPath Foundation review and approval
  ├── 3. Configure SignPath Organization, Project, and Signing Policy
  ├── 4. Add SIGNPATH_API_TOKEN to GitHub repository secrets
  └── 5. Enable and test .github/workflows/signpath-release.yml
```

---

## 4. SmartScreen Reputation Disclosure

Even after receiving a publicly trusted Authenticode certificate through SignPath Foundation:
- **SmartScreen Reputation is Not Instantaneous:** Windows SmartScreen reputation is tied to both the signing certificate and binary download volume/telemetry. A newly issued certificate has no initial reputation and may initially display "Windows protected your PC" until sufficient downloads accumulate without Defender telemetry warnings.
- **EV vs Standard Certificates:** Standard open-source certificates build reputation over time as users run the application. No code-signing certificate guarantees zero warnings on day one.
