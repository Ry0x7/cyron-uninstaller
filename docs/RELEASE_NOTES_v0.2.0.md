# Cyron Uninstaller 0.2.0

Cyron 0.2.0 is the stable Windows x64 release and supersedes 0.1.0.

- Destructive actions use confirmation plans tied to verified target identities.
- Batch uninstall excludes entries that are ineligible for unattended removal.
- Performance adds short spike captures and read-only kernel, driver, and recent device-event evidence.

Choose the NSIS setup executable for a per-user install. The release also includes a per-machine MSI and the standalone executable. Verify downloads against the accompanying SHA256SUMS.txt.

The binaries are unsigned. Windows may show an unknown-publisher or SmartScreen warning. The release audit records open verification for the elevated MSI lifecycle and some real vendor and Store uninstall paths.

[RC2 feature details](https://github.com/Ry0x7/cyron-uninstaller/releases/tag/v0.2.0-rc.2) | [Release audit](https://github.com/Ry0x7/cyron-uninstaller/blob/v0.2.0/docs/RELEASE_AUDIT_2026-11-17.md)
