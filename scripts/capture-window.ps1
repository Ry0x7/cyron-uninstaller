# Captures the Cyron main window to a PNG so a running build can be reviewed visually.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/capture-window.ps1 -ProcessId 1234 -Out shot.png
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Out
)

Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
public class Win32 {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
}
"@

$process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
if (-not $process) { Write-Error "no process $ProcessId"; exit 1 }

$handle = $process.MainWindowHandle
if ($handle -eq [IntPtr]::Zero) { Write-Error "process $ProcessId has no main window"; exit 1 }

[void][Win32]::ShowWindow($handle, 9)   # SW_RESTORE
[void][Win32]::SetForegroundWindow($handle)
Start-Sleep -Milliseconds 900

$rect = New-Object RECT
if (-not [Win32]::GetWindowRect($handle, [ref]$rect)) { Write-Error "GetWindowRect failed"; exit 1 }

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { Write-Error "window has no area"; exit 1 }

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
$bitmap.Save((Resolve-Path -LiteralPath (Split-Path -Parent $Out)).Path + "\" + (Split-Path -Leaf $Out), [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()

Write-Output ("CAPTURE_OK {0}x{1} -> {2}" -f $width, $height, $Out)
