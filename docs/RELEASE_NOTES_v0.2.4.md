# Cyron Uninstaller 0.2.4

Cyron 0.2.4 adds **System health** to Storage & Cleanup: a read-only scan for startup commands and uninstall registrations that point at programs that no longer exist, with review-first removal. Existing 0.2.3 installations can update from **Settings → About → Software updates**.

## Find dead startup entries

- **Storage & Cleanup → Cleanup → System health** scans the HKCU and HKLM Run/RunOnce values that Windows starts with your account or machine.
- An entry is reported only when its command resolves to an absolute executable (after `%VAR%` expansion) that is gone. Startup-folder shortcuts and bare command names are never guessed at.
- Each row shows the exact command, location and registry value, plus why Cyron considers it dead.
- Windows security startup entries are protected and cannot be removed. Machine-wide values that the current session cannot write are labelled **Needs elevation**.

## Find dead uninstall registrations

- The same scan checks Win32 uninstall registrations for entries whose uninstaller and install folder are both missing.
- System components and shared vendor roots are excluded, so Windows and common redistributables are not offered.
- Each row shows the registry key, publisher and recorded version, and the uninstaller or folder that is missing.

## Remove only after review

- Removal requires cleaning to be enabled, then an explicit confirmation showing the exact key and value.
- Cyron re-reads the live entry and refuses anything that changed or reappeared since the scan.
- A dead startup value is exported to a verified single-value `.reg` backup, recorded in the recovery journal and stored in quarantine before the value is deleted.
- A dead uninstall registration uses the existing recursive multi-type `.reg` backup, live fingerprint check and journal.
- Both can be restored from **History & Recovery → Recovery**. Scanning itself never changes the registry.

## Compatibility and trust

- Version 0.2.4 updates the application, installers, PE metadata and About panel together.
- Upgrading preserves `%APPDATA%\dev.cyron.uninstaller` (history, cleanup log and quarantine). A 0.2.3 → 0.2.2 downgrade and the 0.2.3 restore were exercised with history and quarantine intact (GATES G135).
- The Windows binaries are still Authenticode-unsigned. Windows may show an unknown-publisher or SmartScreen warning. The Tauri updater signature proves that an update came from this project's update key; it is separate from Authenticode public trust.
- Verify downloads against the release's `SHA256SUMS.txt`.
