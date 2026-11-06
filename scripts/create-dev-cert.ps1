# scripts/create-dev-cert.ps1
# Delegates to create-signing-cert.ps1 for Chryypt Organization code signing.

[CmdletBinding()]
param(
    [switch]$ForceNew = $false
)

$targetScript = Join-Path $PSScriptRoot "create-signing-cert.ps1"
if ($ForceNew) {
    & powershell -ExecutionPolicy Bypass -File $targetScript -ForceNew
} else {
    & powershell -ExecutionPolicy Bypass -File $targetScript
}
exit $LASTEXITCODE
