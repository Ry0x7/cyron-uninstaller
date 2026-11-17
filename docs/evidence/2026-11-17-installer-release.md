# Release bundle and installer verification — 2026-11-17

This receipt records the Phase 8 packaging and installer run against the current working tree.
It is the direct evidence behind the packaging and installer rows in
[the release audit](../RELEASE_AUDIT_2026-11-17.md). It does not make the release production-ready:
the artifacts are unsigned and the MSI was not installed with elevation.

## Tested tree and machine

| Item | Value |
|---|---|
| Repository | `E:\Github\cyron-uninstaller`, branch `main` |
| Base commit | `3d9f2daddb527492afc578c43dc70c1a7a7ce6f2` plus the uncommitted Phase 8 working tree |
| Build time | 2026-11-17 09:43 local (release build), installer run 2026-11-18 00:23 +08 (`16:23Z`) |
| OS | Windows 11 Pro x64, build `10.0.26200` |
| Node.js | v24.18.0 |
| Elevation | None |
| Installer session | Real interactive user profile, not a clean VM |

## Artifacts

Built with `npm run build:unsigned` (Tauri release build, NSIS + WiX MSI + optimized EXE).

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.1.0_x64-setup.exe` | 2,271,846 | `16a378226f6162baa93373830817bb20df65c59e3f9ae589f34b15dbbdec479f` |
| `src-tauri/target/release/bundle/msi/Cyron Uninstaller_0.1.0_x64_en-US.msi` | 3,330,048 | `3f1e64a7594fba9df4fbecfb7537bde36175dd11166ac6fc346ebe88b6612630` |
| `src-tauri/target/release/cyron.exe` | 6,918,144 | `95c3f7effec7ce3ff9a922997f8bc3ccb275ead8df88167d07f7dd0268ca20d4` |

`node scripts/verify-release-bundles.mjs` → `RELEASE_BUNDLES_OK`.
`npm run verify:metadata` → `WINDOWS_METADATA_OK`: ProductName `Cyron Uninstaller`, version
`0.1.0`, publisher `Chryypt`, MSI UpgradeCode `{C2BCD648-205F-5904-9E5C-85726E376EC2}`.

## Signature result

`npm run verify:signatures`:

- Negative tests PASS: an unsigned file is rejected; appended bytes are detected; a
  wrong-subject certificate is rejected.
- All three release artifacts are **NotSigned**. The command exits 1, correctly.

The local `CN=Chryypt Organization` certificate exists (thumbprint
`D894874C68F8B794708F16D5FD96429A61614F28`, not after 2028-11-07), but its chain status is
`UntrustedRoot`. It is self-signed and does not establish public trust or SmartScreen reputation.

The published `v0.1.0` NSIS asset was also downloaded and checked:

| Check | Result |
|---|---|
| Published SHA-256 vs `SHA256SUMS.txt` | Match: `f2b64a0474b2b0d4c110f5a54d4f78c46de21fd3378f8eeb2054d968abc3928f` |
| Authenticode status | `NotSigned` |
| Signer certificate | None |

No warning-free installation or trusted-publisher claim is supported for either the published
release or the locally built bundle.

## NSIS installer run

Executed four times: initial install, same-version repair, reinstall after uninstall, and a final
uninstall. App-data preservation was checked with SHA-256 hashes of every file, plus disposable
sentinels in the app-data root and under `quarantine\installer-test-run\`.

| Step | Command | Observed |
|---|---|---|
| Install | `Cyron Uninstaller_0.1.0_x64-setup.exe /S` | Exit 0 in 1.07 s; `cyron.exe` and `uninstall.exe` in `%LOCALAPPDATA%\Cyron Uninstaller`; HKCU uninstall entry with version `0.1.0`, publisher `Chryypt`, correct InstallLocation/UninstallString |
| Repair / same-version reinstall | Same setup `/S` over the installed copy | Exit 0 in 1.02 s; app-data root and quarantine sentinels unchanged |
| Relaunch | Installed `cyron.exe` | Window titled `Cyron Uninstaller`; responding; 33.5 MB working set; clean exit code 0. Capture: [34-installed-launch.png](phase8/34-installed-launch.png) |
| Uninstall | Installed `uninstall.exe /S` | Exit 0 in 1.24 s; install directory removed; uninstall registry entry count 0; app-data hash comparison changed 0 entries; both sentinels intact; no Start Menu, Desktop or public-desktop shortcut remained |
| Reinstall after uninstall | Same setup `/S` | Exit 0; install directory present; quarantine sentinel intact |
| Final uninstall | Installed `uninstall.exe /S` | Exit 0; install directory gone; uninstall entry count 0; test sentinels removed |

This verifies the per-user NSIS install, repair, relaunch, uninstall and reinstall path, and that
Cyron app data (including a quarantine stand-in) survives install and uninstall. The silent `/S`
mode did not display the installer or uninstaller UI, so the wizard pages, progress text and finish
action were not visually inspected.

## MSI attempt

`msiexec /i "Cyron Uninstaller_0.1.0_x64_en-US.msi" /qn /norestart` was run without elevation:

- Exit code `1603`.
- MSI log: `Error 1925. You do not have sufficient privileges to complete this installation for all users of the machine. Log on as administrator and then retry this installation.`
- `InstallLocation` was not created by the MSI.

The WiX package is per-machine and requires elevation. An elevated MSI install, upgrade and
uninstall was **not** performed. The NSIS current-user package is the tested per-user path.

## Limitations

1. The run was on the audit machine, not a clean Windows VM or a second Windows version.
2. Reinstall used the same `0.1.0` package. A version-to-version upgrade, downgrade and MSI
   upgrade sequence were not executed.
3. The MSI was not installed or uninstalled with elevation; only its metadata and un-elevated
   refusal were verified.
4. The installer and uninstaller ran silently. Wizard pages, UAC/SmartScreen presentation and
   Add/Remove Programs UI were not visually reviewed.
5. App-data preservation used sentinel files and a full hash comparison, not a real quarantine
   restore after reinstall. The quarantine engine itself is covered by the Rust and cleanup fixtures.
6. The installed executable was launched, but the full desktop journey suite was run against the
   debug build, not this release build.
7. The artifacts are unsigned. Public certificate trust is a release blocker; see the audit.
