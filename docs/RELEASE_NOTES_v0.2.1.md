# Cyron Uninstaller 0.2.1

Cyron 0.2.1 is a packaging release. Application behavior is unchanged from 0.2.0; the installers now carry complete Windows metadata and branding.

- The NSIS setup executable and the generated uninstaller report **Chryypt** as `CompanyName`, so Windows file properties no longer show an empty publisher. They also declare `InternalName` and `OriginalFilename`.
- The NSIS installer and uninstaller use the Cyron icon instead of the generic NSIS icon. The MSI Add/Remove Programs entry keeps the same branded `ProductIcon`.
- `bundle.homepage` is configured, so NSIS and MSI register Help and Update links (`URLInfoAbout`, `ARPHELPLINK`, `ARPURLUPDATEINFO`) pointing at the project page.
- The in-app About panel reports version 0.2.1.

Choose the NSIS setup executable for a per-user install. The release also includes a per-machine MSI and the standalone executable. Verify downloads against the accompanying SHA256SUMS.txt.

The binaries are unsigned. Windows may show an unknown-publisher or SmartScreen warning.
