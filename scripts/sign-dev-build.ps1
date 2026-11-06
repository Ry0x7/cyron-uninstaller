# scripts/sign-dev-build.ps1
# Compatibility wrapper delegating to sign-build.ps1 (Chryypt Organization Code Signing).

[CmdletBinding()]
param(
    [string]$TargetDir = "",
    [switch]$SkipBuild = $false
)

$targetScript = Join-Path $PSScriptRoot "sign-build.ps1"
& powershell -ExecutionPolicy Bypass -File $targetScript
exit $LASTEXITCODE
