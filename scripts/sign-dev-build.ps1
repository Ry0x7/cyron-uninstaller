# scripts/sign-dev-build.ps1
# Signs compiled Cyron executables and installer artifacts with the local self-signed
# development certificate using the official Windows SDK signtool.exe.
# Artifacts signed by this script are classified as DEV_TEST_SIGNED.

[CmdletBinding()]
param(
    [string]$TargetDir = "",
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")

Write-Host "=== Cyron Development Code Signing (DEV_TEST_SIGNED) ===" -ForegroundColor Cyan

# 1. Locate SignTool
function Find-SignTool {
    if ($env:SIGNTOOL_PATH -and (Test-Path $env:SIGNTOOL_PATH)) {
        return (Resolve-Path $env:SIGNTOOL_PATH).Path
    }
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }
    $kitRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "$env:ProgramFiles\Windows Kits\10\bin"
    )
    foreach ($kr in $kitRoots) {
        if (Test-Path $kr) {
            $tools = Get-ChildItem -Path $kr -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } |
                Sort-Object FullName -Descending
            if ($tools.Count -gt 0) {
                return $tools[0].FullName
            }
        }
    }
    return $null
}

$signtool = Find-SignTool
if (-not $signtool) {
    Write-Error "FAIL: signtool.exe not found in Windows Kits or PATH. Install the Windows 10/11 SDK."
    exit 1
}
Write-Host "Using SignTool : $signtool" -ForegroundColor Green

# 2. Retrieve or create development certificate
$certScript = Join-Path $PSScriptRoot "create-dev-cert.ps1"
$certOutput = & powershell -ExecutionPolicy Bypass -File $certScript
$thumbprint = ($certOutput | Select-Object -Last 1).Trim()

if (-not $thumbprint -or $thumbprint.Length -lt 40) {
    Write-Error "FAIL: Could not obtain valid certificate thumbprint from create-dev-cert.ps1"
    exit 1
}

$cert = Get-Item "Cert:\CurrentUser\My\$thumbprint" -ErrorAction SilentlyContinue
if (-not $cert) {
    Write-Error "FAIL: Certificate with thumbprint $thumbprint not found in Cert:\CurrentUser\My"
    exit 1
}
if ($cert.Subject -ne "CN=Cyron Uninstaller Development") {
    Write-Error "FAIL: Refusing to use certificate with subject '$($cert.Subject)'. Expected 'CN=Cyron Uninstaller Development'."
    exit 1
}
Write-Host "Using Certificate : $($cert.Subject) [$thumbprint]" -ForegroundColor Green

# 3. Discover targets to sign
$targets = @()

$exePath = Join-Path $root "src-tauri\target\release\cyron.exe"
if (Test-Path $exePath) {
    $targets += $exePath
}

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
if (Test-Path $nsisDir) {
    $nsisFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue
    foreach ($f in $nsisFiles) {
        $targets += $f.FullName
    }
}

$msiDir = Join-Path $root "src-tauri\target\release\bundle\msi"
if (Test-Path $msiDir) {
    $msiFiles = Get-ChildItem -Path $msiDir -Filter "*.msi" -ErrorAction SilentlyContinue
    foreach ($f in $msiFiles) {
        $targets += $f.FullName
    }
}

if ($targets.Count -eq 0) {
    Write-Error "FAIL: No build artifacts found to sign. Run build first."
    exit 1
}

Write-Host "`nDiscovered $($targets.Count) artifacts to sign:" -ForegroundColor Yellow
foreach ($t in $targets) {
    Write-Host "  - $t"
}

# 4. Sign each target
$failed = @()
$signed = @()

foreach ($target in $targets) {
    Write-Host "`nSigning: $target..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $signtool -ArgumentList "sign", "/fd", "SHA256", "/sha1", $thumbprint, "/d", "Cyron Uninstaller [DEV-TEST-SIGNED]", "`"$target`"" -NoNewWindow -PassThru -Wait
    if ($proc.ExitCode -ne 0) {
        $failed += "$target (signtool exit code $($proc.ExitCode))"
    } else {
        $signed += $target
        Write-Host "Signed successfully." -ForegroundColor Green
    }
}

Write-Host ""
if ($failed.Count -gt 0) {
    Write-Host "=== SIGNING FAILED ($($failed.Count) artifacts failed) ===" -ForegroundColor Red
    foreach ($f in $failed) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    exit 1
}

Write-Host "=== DEV_TEST_SIGNED_OK ($($signed.Count) artifacts signed) ===" -ForegroundColor Green
exit 0
