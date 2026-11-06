# scripts/create-signing-cert.ps1
# Manages Code Signing Certificate for Chryypt Organization in Cert:\CurrentUser\My.
# Reuses existing valid non-expired certificate with matching CN if available.

[CmdletBinding()]
param(
    [switch]$ForceNew = $false
)

$ErrorActionPreference = "Stop"

$subject = "CN=Chryypt Organization"
$friendlyName = "Chryypt Organization"
$storePath = "Cert:\CurrentUser\My"

Write-Host "=== Chryypt Organization Certificate Manager ===" -ForegroundColor Cyan

# 1. Search for existing valid certificate
if (-not $ForceNew) {
    $now = Get-Date
    $existing = Get-ChildItem -Path $storePath -CodeSigningCert -ErrorAction SilentlyContinue |
        Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt $now } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1

    if ($existing) {
        Write-Host "Reusing existing valid Chryypt Organization certificate:" -ForegroundColor Green
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
Write-Host "Generating new Chryypt Organization code-signing certificate..." -ForegroundColor Yellow

$certParams = @{
    Type = "CodeSigningCert"
    Subject = $subject
    CertStoreLocation = $storePath
    HashAlgorithm = "SHA256"
    KeyLength = 3072
    KeyExportPolicy = "NonExportable"
    NotAfter = (Get-Date).AddYears(2)
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
    Write-Error "Failed to generate Chryypt Organization code signing certificate."
    exit 1
}

Write-Host "Successfully created Chryypt Organization certificate:" -ForegroundColor Green
Write-Host "  Thumbprint : $($cert.Thumbprint)"
Write-Host "  Subject    : $($cert.Subject)"
Write-Host "  NotBefore  : $($cert.NotBefore)"
Write-Host "  NotAfter   : $($cert.NotAfter)"
Write-Host "  Store      : $storePath"
Write-Host "  Key Length : 3072 RSA"
Write-Host "  Hash Alg   : SHA-256"

Write-Output $cert.Thumbprint
exit 0
