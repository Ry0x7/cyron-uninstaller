# Competitor Comparison — Documented Behavior, Not a Benchmark

This document compares Cyron with Revo Uninstaller, Bulk Crap Uninstaller
(BCUninstaller), WizTree, TreeSize, BleachBit, Microsoft PC Manager and
Sysinternals. It is **not** a same-machine performance benchmark.

No competitor product was installed or executed for the 2026-11-17 audit. The
competitor cells below come from the vendors' public documentation and are
linked. Cyron cells come from this repository and its measured evidence. A
documented feature is not proof that the feature works correctly on this
machine, and a vendor performance claim is not a measurement.

## Reproducible scenarios

These are the scenarios a later comparative run should use. The Cyron column
records what this audit actually exercised; competitor columns remain
**NOT RUN** until someone executes the same scenario and records raw output.

| # | Scenario | Fixture / method | Cyron result in this audit | Competitors |
|---|---|---|---|---|
| S1 | Uninstall a registered app with a missing uninstaller | Disposable HKCU uninstall entry whose `UninstallString` points to a non-existent file | Broken-entry review, `.reg` backup, key removal and restore exercised by the desktop suite and Rust tests | NOT RUN |
| S2 | Uninstall a working disposable app and measure leftovers | Disposable HKCU app, install folder, marker executable, uninstaller that removes its key | Desktop suite verified the vendor command ran, key disappeared and the row left the inventory; leftover false-positive rate was not measured against a competitor | NOT RUN |
| S3 | Scan a tree with hard links and a junction | Disposable fixture, `FileIdBothDirectoryInfo` walk, reparse point never followed | Rust tests verify hard-link allocation dedup and junction refusal; scan time measured separately in `docs/BENCHMARKS.md` | NOT RUN |
| S4 | Clean a disposable cache | A single disposable file inside a known cache provider | Rust cleanup fixtures verify journal, quarantine, no-overwrite restore and purge; the desktop suite did not clean this machine's real caches | NOT RUN |
| S5 | Inspect a locked file and its owning process | Disposable file held open by a second process | Restart Manager path is implemented and unit-reviewed; no comparative lock-tool run | NOT RUN |
| S6 | Disable and re-enable a startup entry | Disposable HKCU `Run` entry | Native test verified Cancel changed nothing, Disable wrote `0x03`, Enable restored `0x02`, and the Run value survived | NOT RUN |
| S7 | Install, reinstall and uninstall the release | NSIS current-user installer; preserve `%APPDATA%\dev.cyron.uninstaller` | PASS: install 1.07 s, repair 1.02 s, uninstall 1.24 s, relaunch and reinstall; 0 app-data hash changes; quarantine sentinels preserved. MSI install is per-machine and UNVERIFIED elevated | NOT RUN |
| S8 | Sign and verify the installer | Local `CN=Chryypt Organization` self-signed certificate | FAIL for public trust: all local artifacts and the published `v0.1.0` NSIS are `NotSigned`; the local chain reports `UntrustedRoot`. Integrity negative tests PASS | NOT RUN |

## Capability matrix

Legend: **Yes** = documented or implemented; **Partial** = limited or requires
extra configuration; **No** = not the product's purpose; **?** = not verified
from the cited source. "Cyron" entries cite this repository's implementation or
tests, not a claim of superiority.

| Dimension | Cyron | Revo Uninstaller | BCUninstaller | WizTree | TreeSize | BleachBit | Microsoft PC Manager | Sysinternals |
|---|---|---|---|---|---|---|---|---|
| Primary purpose | Uninstall + storage analysis + reviewed cache cleaning + read-only performance evidence | Uninstall and leftover removal | Bulk/automated uninstall | Disk-space analysis | Disk-space analysis and reporting | Privacy/cache cleaning | General PC maintenance | Process, startup and system diagnostics |
| Registered Win32 inventory | Yes | Yes | Yes, multiple sources | No | No | No | Yes | No (Autoruns covers autostarts, not uninstall inventory) |
| Portable / unregistered app discovery | Bounded portable heuristic | Forced Uninstall can target a file/folder | Yes, portable scanning | No | No | No | ? | No |
| Broken/missing uninstaller handling | Yes, reviewed broken-entry removal with backup | Yes, Forced Uninstall | Yes, damaged/missing uninstaller handling | No | No | No | ? | No |
| Leftover file/registry scan | Yes, confidence/risk review | Yes, Safe/Moderate/Advanced scans | Yes, leftover scan with confidence | No | No | Defined cleaner paths, not app-leftover scanning | Deep cleanup | No |
| Automatic registry backup before changes | Yes, recursive multi-type `.reg` in quarantine | Yes, optional backup/Backup Manager | Junk backup in supported flows | No | No | No | ? | No |
| Recoverable file quarantine | Yes, write-ahead journal + no-overwrite restore | ? | Junk backup, not a general quarantine | No | No | Deletion/shredding is intentional and permanent | Cleanup is not presented as a quarantine | No |
| Batch uninstall | Yes, sequential with per-item outcomes and stop-after-current | Yes, Quick/Multiple Uninstall | Yes, bulk, quiet, CLI automation | No | No | No | Yes, app management | No |
| Force process termination | Yes, path-bound, creation-time-bound, critical-process deny list | Yes, forcibly stop running executables (option) | Yes, via uninstall automation | No | No | No | Yes, boost/process termination | Yes, Process Explorer can terminate processes |
| Disk treemap | Yes | No | Treemap view documented by third-party guides; not verified from the official feature page | Yes | Yes | No | Storage view | No |
| Duplicate detection | Yes, size + edge hash + SHA-256, hard-link-aware | ? | No (not its purpose) | Yes, duplicate file view | Yes, duplicate file/folder search with hash options | No | ? | No |
| Hard-link-aware allocation | Yes, 1M file-ID dedup cap | ? | ? | Yes, vendor states hard links counted once | Partial (reports allocated space; hard-link behavior not verified here) | No | ? | No |
| Direct NTFS MFT scan | Yes, elevated, volume roots | No | No | Yes, vendor states MFT scan, admin for fastest mode | ? | No | No | No |
| Cache cleaner with provider definitions | Yes, reviewed catalog and protected entries | No | Leftover cleaner, not a general cache catalog | No | No | Yes, CleanerML and winapp2.ini | Yes, Windows cleanup | No |
| Reversible startup management | Yes, `StartupApproved` 0x02/0x03, native confirmation | Yes, startup manager | Yes, Startup Manager | No | No | No | Yes, startup apps | Yes, Autoruns |
| Performance evidence | Read-only CPU, memory, PDH disk, processes, startup/tasks; no booster claims | No | No | No | No | No | Boost/health-check claims | Yes, deep process/handle/system data |
| Command-line interface | Yes (startup routing flags; no destructive CLI automation) | ? | Yes, BCU-console | CSV/MFT export flags | Yes (Professional) | Yes | ? | Yes, Autorunsc/Handle |
| Open source | No (repository license) | No | Yes | No | No | Yes | No | No |
| Zero telemetry claim | Yes, by architecture | Not assessed | Not assessed | Not assessed | Not assessed | Vendor states no telemetry | Microsoft product; not assessed | Not assessed |
| Self-signed local signing | Yes, `CN=Chryypt Organization`; not publicly trusted | Vendor-signed commercial builds | Community builds; signing not assessed | Vendor-signed | Vendor-signed | Community builds; signing not assessed | Microsoft-signed | Microsoft-signed |
| Public certificate trust | **No** — self-signed root, no SmartScreen reputation | Vendor certificate (not verified here) | Not verified here | Vendor certificate (not verified here) | Vendor certificate (not verified here) | Not verified here | Microsoft trust | Microsoft trust |
| Accessibility (screen reader, forced colors) | Keyboard/focus/reduced motion exercised; Narrator/forced colors UNVERIFIED | Not assessed | Not assessed | Not assessed | Not assessed | Not assessed | Not assessed | Console/GUI accessibility not assessed |

## What can honestly be said

1. Cyron has the broadest **combination** in this list: uninstall inventory,
   reviewed leftovers, recoverable cleanup, disk analysis and read-only
   performance evidence in one application. That is a scope statement, not a
   quality or speed ranking.
2. Revo and BCUninstaller remain the reference points for uninstall automation
   and leftover coverage. This audit did not run either against the same
   fixtures, so it cannot claim Cyron finds more or fewer leftovers.
3. WizTree is the reference point for MFT scan speed. Cyron also reads the MFT,
   but no same-machine WizTree run was performed. Cyron's own scan timings are
   in [BENCHMARKS.md](BENCHMARKS.md); they are not a comparison.
4. BleachBit is a cleaner with a much larger community cleaner catalog and
   intentional secure deletion. Cyron deliberately prefers quarantine over
   permanent deletion and does not implement shredding.
5. Microsoft PC Manager and Sysinternals are different products: PC Manager is
   a broad maintenance surface, while Sysinternals provides deep diagnostics.
   Neither is a like-for-like replacement for Cyron's reviewed uninstall flow.
6. The signing result is unambiguous: Cyron's local certificate is self-signed
   and untrusted on a standard Windows installation, and the published `v0.1.0`
   NSIS asset is `NotSigned`. No claim of warning-free installation or
   SmartScreen reputation is supported.
7. The NSIS current-user installer path is verified; the MSI is per-machine and
   was not installed with elevation. No competitor installer was run, so even the
   verified Cyron path has no comparative install-time result.

## Sources (read 2026-11-17)

- Revo Uninstaller Pro features: <https://www.revouninstaller.com/products/revo-uninstaller-pro/>
- Revo Forced Uninstall: <https://www.revouninstaller.com/how-to/use-forced-uninstall>
- Revo Hunter Mode: <https://www.revouninstaller.com/how-to/use-hunter-mode>
- Revo backup options: <https://www.revouninstaller.com/online-manual/options>
- BCUninstaller official site: <https://www.bcuninstaller.com/>
- BCUninstaller manual: <https://github.com/BCUninstaller/Bulk-Crap-Uninstaller/blob/master/doc/BCU_manual.html>
- WizTree official site: <https://diskanalyzer.com/>
- WizTree about/MFT: <https://diskanalyzer.com/about>
- WizTree download/portable: <https://diskanalyzer.com/download>
- TreeSize features: <https://www.jam-software.com/treesize/features.shtml>
- TreeSize duplicate search: <https://manuals.jam-software.com/treesize/EN/duplicate_file_search.html>
- BleachBit features: <https://www.bleachbit.org/features>
- BleachBit command line: <https://docs.bleachbit.org/doc/command-line-interface.html>
- Microsoft PC Manager: <https://pcmanager.microsoft.com/>
- Microsoft performance guidance: <https://support.microsoft.com/en-US/Windows/Experience/Performance-Optimization/tips-to-improve-pc-performance-in-windows>
- Sysinternals Process Explorer: <https://learn.microsoft.com/en-us/sysinternals/downloads/process-explorer>
- Sysinternals Autoruns: <https://learn.microsoft.com/en-us/sysinternals/downloads/autoruns>
- Sysinternals Handle: <https://learn.microsoft.com/en-us/sysinternals/downloads/handle>
