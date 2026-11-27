# Security policy

Cyron uninstalls software, moves files to quarantine and edits the Windows registry, so security reports matter a great deal. Thank you for helping keep users safe.

## Supported versions

Only the latest release receives security fixes. Update from **Settings → About → Software updates** or from the [releases page](https://github.com/Ry0x7/cyron-uninstaller/releases).

## Reporting a vulnerability

Please **do not open a public issue** for a vulnerability.

Report it privately through [GitHub's private vulnerability reporting](https://github.com/Ry0x7/cyron-uninstaller/security/advisories/new). Include:

- the Cyron version and Windows version,
- what an attacker can do and what they need first (for example a local standard user, or a crafted file or registry value),
- steps or a proof of concept, and
- any suggested fix.

You can expect an acknowledgement within a few days. Once a fix is released, the advisory is published with credit to you unless you prefer to stay anonymous.

## Scope

In scope: anything that lets Cyron delete, move, overwrite or execute something the user did not review and confirm; bypasses of the confirmation plans, quarantine, registry backups or process protections; path traversal or reparse-point tricks; and problems in the updater or installer.

The threat model and the protections Cyron relies on are described in [docs/SECURITY.md](docs/SECURITY.md).
