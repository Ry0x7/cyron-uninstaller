# scripts/create-dev-cert.ps1
# Creates or reuses a local self-signed Code Signing Certificate in Cert:\CurrentUser\My
# strictly for development and controlled testing.
# Reuses existing valid non-expired certificate with matching CN if available.

[CmdletBinding()]
param(
    [switch]$ForceNew = $false
)

$ErrorActionPreference = "Stop"

$subject = "CN=Cyron Uninstaller Development"
$friendlyName = "Cyron Uninstaller Development"
$storePath = "Cert:\CurrentUser\My"

Write-Host "=== Cyron Development Certificate Manager ===" -ForegroundColor Cyan

# 1. Search for existing valid certificate
if (-not $ForceNew) {
    $now = Get-Date
    $existing = Get-ChildItem -Path $storePath -CodeSigningCert -ErrorAction SilentlyContinue |
        Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt $now } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1

    if ($existing) {
        Write-Host "Reusing existing valid development certificate:" -ForegroundColor Green
        Write-Host "  Thumbprint : $($existing.Thumbprint)"
        Write-Host "  Subject    : $($existing.Subject)"
        Write-Host "  NotBefore  : $($existing.NotBefore)"
        Write-Host "  NotAfter   : $($existing.NotAfter)"
        Write-Host "  Store      : $storePath"
        Write-Output $existing.Thumbprint
        exit 0
    }
}

# 2. Create new certificate
Write-Host "Generating new self-signed development code-signing certificate..." -ForegroundColor Yellow

$certParams = @{
    Type = "CodeSigningCert"
    Subject = $subject
    CertStoreLocation = $storePath
    HashAlgorithm = "SHA256"
    KeyLength = 3072
    KeyExportPolicy = "NonExportable"
    NotAfter = (Get-Date).AddYears(1)
    FriendlyName = $friendlyName
}

try {
    $cert = New-SelfSignedCertificate @certParams
} catch {
    Write-Warning "NonExportable key creation failed ($($_.Exception.Message)); retrying with default export policy..."
    $certParams.Remove("KeyExportPolicy")
    $cert = New-SelfSignedCertificate @certParams
}

if (-not $cert) {
    Write-Error "Failed to generate self-signed code signing certificate."
    exit 1
}

Write-Host "Successfully created development certificate:" -ForegroundColor Green
Write-Host "  Thumbprint : $($cert.Thumbprint)"
Write-Host "  Subject    : $($cert.Subject)"
Write-Host "  NotBefore  : $($cert.NotBefore)"
Write-Host "  NotAfter   : $($cert.NotAfter)"
Write-Host "  Store      : $storePath"
Write-Host "  Key Length : 3072 RSA"
Write-Host "  Hash Alg   : SHA-256"

Write-Output $cert.Thumbprint
exit 0
