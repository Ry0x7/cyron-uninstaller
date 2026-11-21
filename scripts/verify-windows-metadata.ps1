# scripts/verify-windows-metadata.ps1
# Verifies Windows PE VERSIONINFO, MSI database properties, NSIS installer metadata,
# versions, icons, and packaging consistency against package.json and tauri.conf.json.

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

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
$expectedPublisher = "Chryypt"
$expectedExeName = "cyron.exe"
$expectedIdentifier = "dev.cyron.uninstaller"
$expectedUpgradeCode = "{C2BCD648-205F-5904-9E5C-85726E376EC2}"
$expectedHomepage = "https://github.com/Ry0x7/cyron-uninstaller"
$expectedInstallerInternalName = "CyronUninstallerSetup"

$installerIconPath = Join-Path $root "src-tauri\icons\icon.ico"
$installerHooksPath = Join-Path $root "src-tauri\nsis\installer-hooks.nsh"

Write-Host "=== Cyron Windows Metadata Verification ===" -ForegroundColor Cyan
Write-Host "Expected Product Name : $expectedProductName"
Write-Host "Expected Version      : $expectedVersion"
Write-Host "Expected Publisher    : $expectedPublisher"
Write-Host "Expected Exe Name     : $expectedExeName"
Write-Host "Expected App ID       : $expectedIdentifier"
Write-Host "Expected Homepage     : $expectedHomepage"
Write-Host ""

$failures = @()

function Get-IconPixelHash([System.Drawing.Icon]$Icon) {
    $bitmap = $Icon.ToBitmap()
    $stream = New-Object System.IO.MemoryStream
    try {
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        $sha = [System.Security.Cryptography.SHA256]::Create()
        try {
            return ([BitConverter]::ToString($sha.ComputeHash($stream.ToArray())) -replace "-", "").ToLowerInvariant()
        } finally {
            $sha.Dispose()
        }
    } finally {
        $stream.Dispose()
        $bitmap.Dispose()
    }
}

# 1. Config Consistency
Write-Host "[1/6] Verifying Tauri Configuration..." -ForegroundColor Yellow
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
if ($tauriConf.bundle.homepage -ne $expectedHomepage) {
    $failures += "tauri.conf.json bundle.homepage ('$($tauriConf.bundle.homepage)') != expected '$expectedHomepage'"
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

# The NSIS installer, uninstaller and MSI ARP entry must all use the branded icon.
$nsisConfig = $tauriConf.bundle.windows.nsis
foreach ($key in @("installerIcon", "uninstallerIcon")) {
    $iconValue = $nsisConfig.$key
    if ([string]::IsNullOrWhiteSpace($iconValue)) {
        $failures += "tauri.conf.json bundle.windows.nsis.$key is not configured"
    } elseif (-not (Test-Path (Join-Path $root "src-tauri\$iconValue"))) {
        $failures += "tauri.conf.json bundle.windows.nsis.$key points at a missing file: $iconValue"
    }
}
if (-not (Test-Path $installerIconPath)) {
    $failures += "Installer icon not found at $installerIconPath"
} else {
    # A single-size .ico renders as a blank or blurry installer icon in Explorer,
    # the taskbar and Add/Remove Programs, so require the standard size ladder.
    $iconBytes = [System.IO.File]::ReadAllBytes($installerIconPath)
    $iconEntryCount = [BitConverter]::ToUInt16($iconBytes, 4)
    $iconSizes = @()
    for ($i = 0; $i -lt $iconEntryCount; $i++) {
        $offset = 6 + ($i * 16)
        $width = [int]$iconBytes[$offset]
        if ($width -eq 0) { $width = 256 }
        $iconSizes += $width
    }
    foreach ($requiredSize in @(16, 32, 48, 256)) {
        if (-not ($iconSizes -contains $requiredSize)) {
            $failures += "Installer icon lacks a ${requiredSize}x${requiredSize} image (found: $($iconSizes -join ', '))"
        }
    }
}

# Tauri's NSIS template does not emit CompanyName itself; the installer hook does.
if ([string]::IsNullOrWhiteSpace($nsisConfig.installerHooks)) {
    $failures += "tauri.conf.json bundle.windows.nsis.installerHooks is not configured"
} elseif (-not (Test-Path $installerHooksPath)) {
    $failures += "NSIS installer hooks file not found at $installerHooksPath"
} else {
    $hooksText = Get-Content $installerHooksPath -Raw
    if ($hooksText -notmatch 'VIAddVersionKey\s+"CompanyName"\s+"' + [regex]::Escape($expectedPublisher) + '"') {
        $failures += "NSIS installer hooks do not declare CompanyName '$expectedPublisher'"
    }
}
Write-Host "Config consistency checks complete." -ForegroundColor Green

# 2. Main Executable PE VERSIONINFO
Write-Host "[2/6] Inspecting cyron.exe VERSIONINFO..." -ForegroundColor Yellow
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
    if (-not ($vi.LegalCopyright -match "Chryypt" -and $vi.LegalCopyright -match "2026")) {
        $failures += "cyron.exe LegalCopyright ('$($vi.LegalCopyright)') does not contain 'Chryypt' and '2026'"
    }
    Write-Host "cyron.exe metadata verified: ProductName='$($vi.ProductName)', Version='$($vi.ProductVersion)', Publisher='$($vi.CompanyName)'" -ForegroundColor Green
}

# 3. NSIS Installer Metadata
Write-Host "[3/6] Inspecting NSIS Installer Metadata..." -ForegroundColor Yellow
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
    if ($nvi.CompanyName -ne $expectedPublisher) {
        $failures += "NSIS installer CompanyName ('$($nvi.CompanyName)') != expected '$expectedPublisher'"
    }
    if ([string]::IsNullOrWhiteSpace($nvi.FileDescription)) {
        $failures += "NSIS installer FileDescription is empty"
    }
    if ($nvi.InternalName -ne $expectedInstallerInternalName) {
        $failures += "NSIS installer InternalName ('$($nvi.InternalName)') != expected '$expectedInstallerInternalName'"
    }
    if ([string]::IsNullOrWhiteSpace($nvi.OriginalFilename)) {
        $failures += "NSIS installer OriginalFilename is empty"
    }
    if (-not ($nvi.LegalCopyright -match "Chryypt" -and $nvi.LegalCopyright -match "2026")) {
        $failures += "NSIS installer LegalCopyright ('$($nvi.LegalCopyright)') does not contain 'Chryypt' and '2026'"
    }
    Write-Host "NSIS installer metadata verified: Name='$($nsisExe.Name)', Version='$($nvi.ProductVersion)', Publisher='$($nvi.CompanyName)'" -ForegroundColor Green
}

# 4. WiX MSI Database Properties
Write-Host "[4/6] Inspecting WiX MSI Database Properties..." -ForegroundColor Yellow
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
        if (-not $props["UpgradeCode"] -or $props["UpgradeCode"].ToUpper() -ne $expectedUpgradeCode.ToUpper()) {
            $failures += "MSI Property UpgradeCode ('$($props["UpgradeCode"])') != expected '$expectedUpgradeCode'"
        }
        if ($props["ARPPRODUCTICON"] -ne "ProductIcon") {
            $failures += "MSI Property ARPPRODUCTICON ('$($props["ARPPRODUCTICON"])') is not ProductIcon"
        }
        if ($props["ARPURLINFOABOUT"] -ne $expectedHomepage) {
            $failures += "MSI Property ARPURLINFOABOUT ('$($props["ARPURLINFOABOUT"])') != expected '$expectedHomepage'"
        }

        $summary = $com.SummaryInformation($msiFile.FullName, 0)
        if ($summary -ne $null) {
            $author = $summary.Property(4)
            if ($author -ne $expectedPublisher) {
                $failures += "MSI Summary Information Author ('$author') != expected '$expectedPublisher'"
            }
        } else {
            $failures += "MSI Summary Information could not be read"
        }

        # ARP renders the ProductIcon from the Icon table; it must be the branded .ico, not a placeholder.
        $iconView = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @("SELECT Name, Data FROM Icon"))
        $iconView.GetType().InvokeMember("Execute", "InvokeMethod", $null, $iconView, $null)
        $iconRecord = $iconView.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $iconView, $null)
        $productIconSize = -1
        while ($iconRecord -ne $null) {
            $iconName = $iconRecord.GetType().InvokeMember("StringData", "GetProperty", $null, $iconRecord, @(1))
            if ($iconName -eq "ProductIcon") {
                $productIconSize = $iconRecord.GetType().InvokeMember("DataSize", "GetProperty", $null, $iconRecord, @(2))
            }
            $iconRecord = $iconView.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $iconView, $null)
        }
        if ($productIconSize -lt 0) {
            $failures += "MSI Icon table has no ProductIcon stream"
        } elseif ((Test-Path $installerIconPath) -and $productIconSize -ne (Get-Item $installerIconPath).Length) {
            $failures += "MSI ProductIcon stream ($productIconSize bytes) is not the configured installer icon ($((Get-Item $installerIconPath).Length) bytes)"
        }

        Write-Host "MSI properties verified: ProductName='$($props["ProductName"])', Manufacturer='$($props["Manufacturer"])', UpgradeCode='$($props["UpgradeCode"])'" -ForegroundColor Green
    } catch {
        $failures += "Failed to open or query MSI database: $($_.Exception.Message)"
    }
}

# 5. Artifact Icons
Write-Host "[5/6] Verifying installer and application icons..." -ForegroundColor Yellow
if ((Test-Path $exePath) -and $nsisExe) {
    try {
        $appIconHash = Get-IconPixelHash ([System.Drawing.Icon]::ExtractAssociatedIcon($exePath))
        $setupIconHash = Get-IconPixelHash ([System.Drawing.Icon]::ExtractAssociatedIcon($nsisExe.FullName))
        if ($appIconHash -ne $setupIconHash) {
            $failures += "NSIS setup icon does not match the application icon (setup=$setupIconHash, app=$appIconHash)"
        } else {
            Write-Host "Installer icon matches the Cyron application icon." -ForegroundColor Green
        }
    } catch {
        $failures += "Failed to compare installer and application icons: $($_.Exception.Message)"
    }
} else {
    $failures += "Cannot compare installer and application icons without both release artifacts"
}

# 6. Summary
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
