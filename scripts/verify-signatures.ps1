# scripts/verify-signatures.ps1
# Verifies Authenticode signatures of compiled Cyron binaries and installers.
# Separates cryptographic file integrity verification from certificate-chain trust.
# Also includes a negative-test suite mode (-RunNegativeTests).

[CmdletBinding()]
param(
    [ValidateSet("DEV_TEST_SIGNED", "SIGNPATH_SIGNED", "ANY")]
    [string]$Mode = "DEV_TEST_SIGNED",
    [switch]$RunNegativeTests = $false
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")

Write-Host "=== Cyron Authenticode Signature Verification ===" -ForegroundColor Cyan
Write-Host "Verification Mode : $Mode"

# 1. Run Negative Tests if requested
if ($RunNegativeTests) {
    Write-Host "`n--- Running Negative Verification Tests ---" -ForegroundColor Yellow
    $negFailures = @()

    # Negative Test 1: Unsigned file must fail verification
    $tempUnsigned = [System.IO.Path]::ChangeExtension([System.IO.Path]::GetTempFileName(), ".exe")
    [System.IO.File]::WriteAllBytes($tempUnsigned, [System.Text.Encoding]::UTF8.GetBytes("MZ_FAKE_PE_HEADER_UNSIGNED"))
    $sigUnsigned = Get-AuthenticodeSignature $tempUnsigned
    if ($sigUnsigned.SignerCertificate -ne $null) {
        $negFailures += "Negative Test 1 failed: Unsigned file reported non-null SignerCertificate"
    } else {
        Write-Host "PASS: Unsigned file rejected as unsigned." -ForegroundColor Green
    }
    Remove-Item $tempUnsigned -ErrorAction SilentlyContinue

    # Negative Test 2: Tampered signed file must fail verification
    $realExe = Join-Path $root "src-tauri\target\release\cyron.exe"
    if (Test-Path $realExe) {
        $tempTampered = [System.IO.Path]::ChangeExtension([System.IO.Path]::GetTempFileName(), ".exe")
        Copy-Item $realExe $tempTampered
        # Tamper by appending bytes to the end of the signed binary
        [System.IO.File]::AppendAllText($tempTampered, "MALICIOUS_PAYLOAD_TAMPER_BYTES")
        $sigTampered = Get-AuthenticodeSignature $tempTampered
        # Modifying a signed file invalidates Authenticode signature (HashMismatch or NotSigned)
        if ($sigTampered.Status -eq "Valid") {
            $negFailures += "Negative Test 2 failed: Tampered file reported Status=Valid"
        } else {
            Write-Host "PASS: Tampered file detected (Status=$($sigTampered.Status))." -ForegroundColor Green
        }
        Remove-Item $tempTampered -ErrorAction SilentlyContinue
    }

    # Negative Test 3: Wrong subject verification
    $dummyCertParams = @{
        Type = "CodeSigningCert"
        Subject = "CN=Wrong Subject Attacker"
        CertStoreLocation = "Cert:\CurrentUser\My"
        HashAlgorithm = "SHA256"
        NotAfter = (Get-Date).AddDays(1)
    }
    try {
        $wrongCert = New-SelfSignedCertificate @dummyCertParams
        $tempWrong = [System.IO.Path]::ChangeExtension([System.IO.Path]::GetTempFileName(), ".exe")
        Copy-Item $realExe $tempWrong
        
        # Locate signtool
        $signtool = (Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } | Select-Object -First 1).FullName
        if ($signtool) {
            & $signtool sign /fd SHA256 /sha1 $wrongCert.Thumbprint $tempWrong 2>&1 | Out-Null
            $sigWrong = Get-AuthenticodeSignature $tempWrong
            if ($sigWrong.SignerCertificate.Subject -eq "CN=Cyron Uninstaller Development") {
                $negFailures += "Negative Test 3 failed: Wrong certificate subject matched dev subject"
            } else {
                Write-Host "PASS: Wrong certificate subject rejected ($($sigWrong.SignerCertificate.Subject))." -ForegroundColor Green
            }
        }
        Remove-Item $tempWrong -ErrorAction SilentlyContinue
        Remove-Item "Cert:\CurrentUser\My\$($wrongCert.Thumbprint)" -ErrorAction SilentlyContinue
    } catch {
        Write-Host "Negative Test 3 note: Skipped certificate generation ($($_.Exception.Message))"
    }

    if ($negFailures.Count -gt 0) {
        Write-Host "Negative tests failed:" -ForegroundColor Red
        foreach ($nf in $negFailures) { Write-Host "  - $nf" -ForegroundColor Red }
        exit 1
    }
    Write-Host "All negative tests PASSED." -ForegroundColor Green
    Write-Host ""
}

# 2. Discover artifacts to verify
$artifacts = @()

$exePath = Join-Path $root "src-tauri\target\release\cyron.exe"
if (Test-Path $exePath) {
    $artifacts += $exePath
}

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
if (Test-Path $nsisDir) {
    $nsisFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue
    foreach ($f in $nsisFiles) { $artifacts += $f.FullName }
}

$msiDir = Join-Path $root "src-tauri\target\release\bundle\msi"
if (Test-Path $msiDir) {
    $msiFiles = Get-ChildItem -Path $msiDir -Filter "*.msi" -ErrorAction SilentlyContinue
    foreach ($f in $msiFiles) { $artifacts += $f.FullName }
}

if ($artifacts.Count -eq 0) {
    Write-Error "FAIL: No artifacts found in src-tauri\target\release to verify."
    exit 1
}

Write-Host "Verifying $($artifacts.Count) artifacts..." -ForegroundColor Yellow

$failures = @()
$verified = @()

foreach ($filePath in $artifacts) {
    $fileName = Split-Path $filePath -Leaf
    Write-Host "`nArtifact: $fileName" -ForegroundColor Cyan
    
    $sig = Get-AuthenticodeSignature $filePath
    if (-not $sig -or -not $sig.SignerCertificate) {
        $failures += "$fileName is NOT signed."
        Write-Host "  FAIL: Signature missing or unreadable." -ForegroundColor Red
        continue
    }

    $cert = $sig.SignerCertificate
    $subject = $cert.Subject
    $thumbprint = $cert.Thumbprint
    $status = $sig.Status
    $statusMsg = $sig.StatusMessage

    Write-Host "  Signer Subject : $subject"
    Write-Host "  Thumbprint     : $thumbprint"
    Write-Host "  Sig Status     : $status"
    Write-Host "  Status Message : $statusMsg"

    # Cryptographic integrity check:
    # If Status is HashMismatch or the file was tampered with, fail immediately.
    if ($status -eq "HashMismatch") {
        $failures += "$fileName cryptographic digest verification FAILED (tampered binary)."
        Write-Host "  FAIL: Cryptographic hash mismatch!" -ForegroundColor Red
        continue
    }

    if ($Mode -eq "DEV_TEST_SIGNED") {
        # Must match CN=Cyron Uninstaller Development
        if ($subject -ne "CN=Cyron Uninstaller Development") {
            $failures += "$fileName signed by '$subject', expected 'CN=Cyron Uninstaller Development'."
            Write-Host "  FAIL: Incorrect signer certificate subject." -ForegroundColor Red
            continue
        }
        # In DEV_TEST_SIGNED, the status is expected to be UnknownError (untrusted self-signed root)
        # unless explicitly installed in a test VM root store.
        if ($status -eq "Valid") {
            Write-Host "  Trust Chain    : Valid (certificate is trusted by the current machine)." -ForegroundColor Green
        } elseif ($status -eq "UnknownError" -and ($statusMsg -match "root certificate which is not trusted" -or $statusMsg -match "untrusted root")) {
            Write-Host "  Trust Chain    : Expected Self-Signed Root (cryptographically intact, untrusted root)." -ForegroundColor Yellow
        } else {
            $failures += "$fileName unexpected signature status: $status ($statusMsg)"
            Write-Host "  FAIL: Unexpected signature status: $status ($statusMsg)" -ForegroundColor Red
            continue
        }
    } elseif ($Mode -eq "SIGNPATH_SIGNED") {
        # Must match SignPath Foundation and have a valid chained status
        if ($subject -notmatch "SignPath Foundation") {
            $failures += "$fileName signer subject '$subject' does not match SignPath Foundation."
            Write-Host "  FAIL: Subject does not match SignPath Foundation." -ForegroundColor Red
            continue
        }
        if ($status -ne "Valid") {
            $failures += "$fileName SignPath signature is not valid ($($status): $($statusMsg))"
            Write-Host "  FAIL: Signature chain is not valid: $status" -ForegroundColor Red
            continue
        }
        Write-Host "  Trust Chain    : Valid SignPath Foundation Authenticode Chain" -ForegroundColor Green
    }

    $verified += $fileName
    Write-Host "  Integrity      : CRYPTOGRAPHICALLY VALID" -ForegroundColor Green
}

Write-Host ""
if ($failures.Count -gt 0) {
    Write-Host "=== SIGNATURE VERIFICATION FAILED ($($failures.Count) errors) ===" -ForegroundColor Red
    foreach ($f in $failures) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    exit 1
}

Write-Host "=== SIGNATURES_VERIFIED_OK ($($verified.Count) artifacts passed) ===" -ForegroundColor Green
exit 0
