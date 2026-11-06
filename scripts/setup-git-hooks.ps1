# scripts/setup-git-hooks.ps1
# Configures local repository git hooks and settings to enforce the No-Rebase policy.

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$hooksDir = Join-Path $root ".git\hooks"

if (-not (Test-Path $hooksDir)) {
    Write-Host "No .git/hooks directory found (shallow or archive checkout). Skipping hook setup."
    exit 0
}

# 1. Install pre-rebase hook (without UTF-8 BOM)
$preRebasePath = Join-Path $hooksDir "pre-rebase"
$hookContent = "#!/bin/sh`n" +
"# Cyron Uninstaller - No-Rebase Safety Hook`n" +
"# Rebasing is strictly prohibited to preserve immutable commit provenance,`n" +
"# cryptographic signature records, and verifiable release hashes.`n`n" +
"echo `"======================================================================`" >&2`n" +
"echo `"ERROR: 'git rebase' is strictly prohibited on Cyron Uninstaller.`" >&2`n" +
"echo `"`" >&2`n" +
"echo `"Rebasing rewrites commit hashes, breaking Authenticode provenance,`" >&2`n" +
"echo `"reproducible build verification, and historical audit trails.`" >&2`n" +
"echo `"`" >&2`n" +
"echo `"Please use 'git merge' instead:`" >&2`n" +
"echo `"  git merge <branch> --no-ff`" >&2`n" +
"echo `"======================================================================`" >&2`n" +
"exit 1`n"

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($preRebasePath, $hookContent, $utf8NoBom)
Write-Host "Installed git pre-rebase safety hook at: $preRebasePath" -ForegroundColor Green

# 2. Configure git pull.rebase = false
& git config pull.rebase false
Write-Host "Configured git config pull.rebase = false" -ForegroundColor Green

Write-Host "No-rebase enforcement successfully active." -ForegroundColor Cyan
exit 0
