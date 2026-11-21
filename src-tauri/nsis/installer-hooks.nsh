; Cyron NSIS installer metadata.
;
; Tauri's NSIS template writes ProductName, FileDescription, LegalCopyright,
; FileVersion and ProductVersion into the setup executable's VERSIONINFO
; resource, but it omits CompanyName, InternalName and OriginalFilename. Without
; CompanyName, Windows shows an empty publisher for the setup file even though
; bundle.publisher is configured.
;
; Tauri includes this file before it emits its own VIAddVersionKey entries, so
; these keys complete the same version resource. NSIS copies the installer's
; version resource into the generated uninstaller, so uninstall.exe reports the
; same publisher.
;
; Keep CompanyName in sync with bundle.publisher in tauri.conf.json.
; scripts/verify-windows-metadata.ps1 fails the packaging gate when they drift.
VIAddVersionKey "CompanyName" "Chryypt"
VIAddVersionKey "InternalName" "CyronUninstallerSetup"
VIAddVersionKey "OriginalFilename" "CyronUninstallerSetup.exe"
