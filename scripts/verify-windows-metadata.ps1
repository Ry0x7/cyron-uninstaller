# scripts/verify-windows-metadata.ps1
# Verifies Windows PE VERSIONINFO, MSI database properties, NSIS installer metadata,
# versions, icons, and packaging consistency against package.json and tauri.conf.json.

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$pkgJsonPath = Join-Path $root "package.json"
$tauriConfPath = Join-Path $root "src-tauri\tauri.conf.json"

if (-not (Test-Path $pkgJsonPath)) {
    Write-Error "FAIL: package.json not found at $pkgJsonPath"
}
if (-not (Test-Path $tauriConfPath)) {
    Write-Error "FAIL: tauri.conf.json not found at $tauriConfPath"
}

$pkg = Get-Content $pkgJsonPath -Raw | ConvertFrom-Json
$tauriConf = Get-Content $tauriConfPath -Raw | ConvertFrom-Json

$expectedVersion = $pkg.version
$expectedProductName = "Cyron Uninstaller"
$expectedPublisher = "Ry0x7"
$expectedExeName = "cyron.exe"
$expectedIdentifier = "dev.cyron.uninstaller"
$expectedUpgradeCode = "{C2BCD648-205F-5904-9E5C-85726E376EC2}"

Write-Host "=== Cyron Windows Metadata Verification ===" -ForegroundColor Cyan
Write-Host "Expected Product Name : $expectedProductName"
Write-Host "Expected Version      : $expectedVersion"
Write-Host "Expected Publisher    : $expectedPublisher"
Write-Host "Expected Exe Name     : $expectedExeName"
Write-Host "Expected App ID       : $expectedIdentifier"
Write-Host ""

$failures = @()

# 1. Config Consistency
Write-Host "[1/5] Verifying Tauri Configuration..." -ForegroundColor Yellow
if ($tauriConf.productName -ne $expectedProductName) {
    $failures += "tauri.conf.json productName ('$($tauriConf.productName)') != expected '$expectedProductName'"
}
if ($tauriConf.version -ne $expectedVersion) {
    $failures += "tauri.conf.json version ('$($tauriConf.version)') != package.json version '$expectedVersion'"
}
if ($tauriConf.identifier -ne $expectedIdentifier) {
    $failures += "tauri.conf.json identifier ('$($tauriConf.identifier)') != expected '$expectedIdentifier'"
}
if ($tauriConf.bundle.publisher -ne $expectedPublisher) {
    $failures += "tauri.conf.json bundle.publisher ('$($tauriConf.bundle.publisher)') != expected '$expectedPublisher'"
}
$confUpgradeCode = $tauriConf.bundle.windows.wix.upgradeCode
if ($confUpgradeCode -and ("{$confUpgradeCode}".ToUpper() -ne $expectedUpgradeCode.ToUpper() -and $confUpgradeCode.ToUpper() -ne $expectedUpgradeCode.Trim('{}').ToUpper())) {
    $failures += "tauri.conf.json bundle.windows.wix.upgradeCode ('$confUpgradeCode') != expected '$expectedUpgradeCode'"
}

# Verify icons
foreach ($iconPath in $tauriConf.bundle.icon) {
    $resolvedIcon = Join-Path $root "src-tauri\$iconPath"
    if (-not (Test-Path $resolvedIcon)) {
        $failures += "Icon resource missing: $resolvedIcon"
    }
}
Write-Host "Config consistency checks complete." -ForegroundColor Green

# 2. Main Executable PE VERSIONINFO
Write-Host "[2/5] Inspecting cyron.exe VERSIONINFO..." -ForegroundColor Yellow
$exePath = Join-Path $root "src-tauri\target\release\$expectedExeName"
if (-not (Test-Path $exePath)) {
    $failures += "Compiled executable not found at $exePath. Run release build first."
} else {
    $vi = (Get-Item $exePath).VersionInfo
    if ($vi.ProductName -ne $expectedProductName) {
        $failures += "cyron.exe ProductName ('$($vi.ProductName)') != expected '$expectedProductName'"
    }
    if ($vi.ProductVersion -ne $expectedVersion) {
        $failures += "cyron.exe ProductVersion ('$($vi.ProductVersion)') != expected '$expectedVersion'"
    }
    if ($vi.FileVersion -ne $expectedVersion) {
        $failures += "cyron.exe FileVersion ('$($vi.FileVersion)') != expected '$expectedVersion'"
    }
    if ($vi.CompanyName -ne $expectedPublisher) {
        $failures += "cyron.exe CompanyName ('$($vi.CompanyName)') != expected '$expectedPublisher'"
    }
    if ($vi.InternalName -ne $expectedExeName) {
        $failures += "cyron.exe InternalName ('$($vi.InternalName)') != expected '$expectedExeName'"
    }
    if ($vi.OriginalFilename -ne $expectedExeName) {
        $failures += "cyron.exe OriginalFilename ('$($vi.OriginalFilename)') != expected '$expectedExeName'"
    }
    if (-not ($vi.LegalCopyright -match "Ry0x7" -and $vi.LegalCopyright -match "2026")) {
        $failures += "cyron.exe LegalCopyright ('$($vi.LegalCopyright)') does not contain 'Ry0x7' and '2026'"
    }
    Write-Host "cyron.exe metadata verified: ProductName='$($vi.ProductName)', Version='$($vi.ProductVersion)', Publisher='$($vi.CompanyName)'" -ForegroundColor Green
}

# 3. NSIS Installer Metadata
Write-Host "[3/5] Inspecting NSIS Installer Metadata..." -ForegroundColor Yellow
$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$nsisExe = Get-ChildItem -Path $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "*$expectedVersion*" } |
    Select-Object -First 1

if (-not $nsisExe) {
    $failures += "NSIS setup executable not found in $nsisDir for version $expectedVersion"
} else {
    $nvi = $nsisExe.VersionInfo
    if ($nvi.ProductName -ne $expectedProductName) {
        $failures += "NSIS installer ProductName ('$($nvi.ProductName)') != expected '$expectedProductName'"
    }
    if ($nvi.ProductVersion -ne $expectedVersion) {
        $failures += "NSIS installer ProductVersion ('$($nvi.ProductVersion)') != expected '$expectedVersion'"
    }
    if (-not ($nvi.LegalCopyright -match "Ry0x7" -and $nvi.LegalCopyright -match "2026")) {
        $failures += "NSIS installer LegalCopyright ('$($nvi.LegalCopyright)') does not contain 'Ry0x7' and '2026'"
    }
    Write-Host "NSIS installer metadata verified: Name='$($nsisExe.Name)', Version='$($nvi.ProductVersion)'" -ForegroundColor Green
}

# 4. WiX MSI Database Properties
Write-Host "[4/5] Inspecting WiX MSI Database Properties..." -ForegroundColor Yellow
$msiDir = Join-Path $root "src-tauri\target\release\bundle\msi"
$msiFile = Get-ChildItem -Path $msiDir -Filter "*.msi" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "*$expectedVersion*" } |
    Select-Object -First 1

if (-not $msiFile) {
    $failures += "WiX MSI package not found in $msiDir for version $expectedVersion"
} else {
    try {
        $com = New-Object -ComObject WindowsInstaller.Installer
        $db = $com.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $com, @($msiFile.FullName, 0))
        $view = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @("SELECT Property, Value FROM Property"))
        $view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null)
        $record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
        $props = @{}
        while ($record -ne $null) {
            $pName = $record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, @(1))
            $pVal = $record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, @(2))
            $props[$pName] = $pVal
            $record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
        }

        if ($props["ProductName"] -ne $expectedProductName) {
            $failures += "MSI Property ProductName ('$($props["ProductName"])') != expected '$expectedProductName'"
        }
        if ($props["ProductVersion"] -ne $expectedVersion) {
            $failures += "MSI Property ProductVersion ('$($props["ProductVersion"])') != expected '$expectedVersion'"
        }
        if ($props["Manufacturer"] -ne $expectedPublisher) {
            $failures += "MSI Property Manufacturer ('$($props["Manufacturer"])') != expected '$expectedPublisher'"
        }
        if ($props["UpgradeCode"].ToUpper() -ne $expectedUpgradeCode.ToUpper()) {
            $failures += "MSI Property UpgradeCode ('$($props["UpgradeCode"])') != expected '$expectedUpgradeCode'"
        }
        Write-Host "MSI properties verified: ProductName='$($props["ProductName"])', Manufacturer='$($props["Manufacturer"])', UpgradeCode='$($props["UpgradeCode"])'" -ForegroundColor Green
    } catch {
        $failures += "Failed to open or query MSI database: $($_.Exception.Message)"
    }
}

# 5. Summary
Write-Host ""
if ($failures.Count -gt 0) {
    Write-Host "=== METADATA VERIFICATION FAILED ($($failures.Count) errors) ===" -ForegroundColor Red
    foreach ($f in $failures) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    exit 1
}

Write-Host "=== WINDOWS_METADATA_OK ===" -ForegroundColor Green
exit 0
