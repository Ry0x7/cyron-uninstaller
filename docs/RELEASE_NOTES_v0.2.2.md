# Cyron Uninstaller 0.2.2

Cyron 0.2.2 adds a read-only **Search** destination for a local Everything index and an in-app updater. Existing 0.2.1 installations need this release installed manually once; later releases can be installed from **Settings → About → Software updates**.

## Everything Search

- Search files and folders by name across the drives covered by a locally installed Everything 1.4.1 or newer.
- Cyron reads the existing Everything index over local Windows IPC. It does not bundle the SDK, install or start Everything, change its index or settings, use Everything's network interfaces, or execute a result.
- Missing, closed, older or index-loading states are shown explicitly instead of as an empty result.
- A result can be revealed through the existing validated File Explorer action.

## In-app updates

- **Settings → About → Software updates** checks the signed GitHub Releases manifest only when you click **Check for updates**. There is no background polling and no telemetry.
- The updater reports the available version and release notes, shows download progress, installs only after confirmation, and restarts Cyron when the Windows installer finishes.
- Update packages are verified against Cyron's updater public key before installation. The private key is held outside the repository; the public key is embedded in the app.
- Release builds now create the updater signature and `latest.json`, and the release workflow publishes both. The NSIS per-user setup is the update package.
- The update feed is scoped to NSIS (`windows-x86_64-nsis`). MSI installations show that they must update from the release page, so a per-machine install is never replaced by a second per-user copy.

## Packaging

- Version 0.2.2 updates the application, installers, PE metadata and About panel together.
- The release workflow now requires the updater signing secret, builds with updater artifacts, and rejects a release whose manifest, signature and installer do not agree.
- SHA-256 checksums cover the NSIS setup, MSI, standalone executable and `latest.json`.

## Trust and limitations

The Windows binaries are still Authenticode-unsigned. Windows may show an unknown-publisher or SmartScreen warning. The Tauri updater signature proves that an update came from this project's update key; it is separate from Authenticode public trust.

The updater feed cannot be exercised until v0.2.2 is published. The local build verifies the updater archive, detached signature and manifest, but a live check against a published release remains to be verified after publication.

Upgrades preserve `%APPDATA%\dev.cyron.uninstaller` (history, cleanup log and quarantine). Verify downloads against the accompanying `SHA256SUMS.txt`.
