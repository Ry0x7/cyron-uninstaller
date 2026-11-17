param(
    [Parameter(Mandatory = $true)][int]$Width,
    [Parameter(Mandatory = $true)][int]$Height,
    [string]$ProcessName = "cyron"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct CyronRect { public int Left; public int Top; public int Right; public int Bottom; }
public class CyronWindow {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out CyronRect rect);
    [DllImport("user32.dll")]
    public static extern bool GetClientRect(IntPtr hWnd, out CyronRect rect);
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
    Select-Object -First 1
if (-not $process) {
    Write-Error "no visible $ProcessName window"
    exit 1
}

$handle = $process.MainWindowHandle
$outer = New-Object CyronRect
$client = New-Object CyronRect
if (-not [CyronWindow]::GetWindowRect($handle, [ref]$outer) -or -not [CyronWindow]::GetClientRect($handle, [ref]$client)) {
    Write-Error "could not read window/client bounds"
    exit 1
}

# The requested size is the webview client area, matching WebDriver's
# setWindowRect semantics. GetWindowRect includes the frame and title bar.
$frameWidth = ($outer.Right - $outer.Left) - ($client.Right - $client.Left)
$frameHeight = ($outer.Bottom - $outer.Top) - ($client.Bottom - $client.Top)
$targetWidth = $Width + [Math]::Max(0, $frameWidth)
$targetHeight = $Height + [Math]::Max(0, $frameHeight)

# SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE: resize only, leave focus and z-order alone.
$flags = 0x0002 -bor 0x0004 -bor 0x0010
$ok = [CyronWindow]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, $targetWidth, $targetHeight, $flags)
if (-not $ok) {
    Write-Error ("SetWindowPos failed: " + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
    exit 1
}
Start-Sleep -Milliseconds 400
Write-Output "WINDOW_SET client=${Width}x${Height} outer=${targetWidth}x${targetHeight}"
