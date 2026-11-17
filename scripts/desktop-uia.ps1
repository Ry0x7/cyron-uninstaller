# Native Windows UI Automation helper for Cyron's desktop E2E suite.
# WebView content is driven by WebdriverIO. This script is only for real native
# dialogs (Tauri ask / folder picker), external windows and Explorer verbs.
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "dump", "invoke", "set-value", "pick-folder", "foreground")]
    [string]$Action,
    [string]$TitleRegex = ".*",
    [string]$NameRegex = ".*",
    [string]$Value = "",
    [int]$ProcessId = 0,
    [int]$Depth = 5
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class CyronUiaWin32 {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$root = [System.Windows.Automation.AutomationElement]::RootElement

function Get-TopWindows {
    $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($window in $children) {
        $title = $window.Current.Name
        $pidValue = $window.Current.ProcessId
        if ($ProcessId -gt 0 -and $pidValue -ne $ProcessId) { continue }
        [pscustomobject]@{
            Name = $title
            ProcessId = $pidValue
            ClassName = $window.Current.ClassName
            AutomationId = $window.Current.AutomationId
            Handle = $window.Current.NativeWindowHandle
            Element = $window
        }
    }
}

function Get-TargetWindow {
    $windows = @(Get-TopWindows)
    $match = $windows | Where-Object { $_.Name -match $TitleRegex } | Select-Object -First 1
    if (-not $match) {
        $visible = $windows | Where-Object { $_.Name } | ForEach-Object { "$($_.ProcessId):$($_.Name)" }
        throw "No window matched '$TitleRegex'. Windows: $($visible -join ' | ')"
    }
    return $match
}

function Get-Descendants($window) {
    return $window.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    )
}

function Format-Element($element, $level) {
    $indent = "  " * $level
    $name = $element.Current.Name
    $type = $element.Current.ControlType.ProgrammaticName -replace "ControlType\.", ""
    $automationId = $element.Current.AutomationId
    $class = $element.Current.ClassName
    $enabled = $element.Current.IsEnabled
    $offscreen = $element.Current.IsOffscreen
    return "$indent$type name='$name' id='$automationId' class='$class' enabled=$enabled offscreen=$offscreen"
}

function Dump-Tree($window, $maxDepth) {
    $lines = @(Format-Element $window 0)
    $elements = @(Get-Descendants $window)
    foreach ($element in $elements) {
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        $level = 0
        $parent = $walker.GetParent($element)
        while ($parent -and $level -lt $maxDepth) {
            $level += 1
            if ($parent.Current.NativeWindowHandle -eq $window.Current.NativeWindowHandle) { break }
            $parent = $walker.GetParent($parent)
        }
        if ($level -le $maxDepth) { $lines += (Format-Element $element $level) }
    }
    return $lines
}

function Invoke-Element($element) {
    try {
        $pattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        return
    } catch { }
    try {
        $point = $element.GetClickablePoint()
        [CyronUiaWin32]::SetCursorPos([int]$point.X, [int]$point.Y) | Out-Null
        [CyronUiaWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        [CyronUiaWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        return
    } catch {
        throw "Element '$($element.Current.Name)' has no InvokePattern and no clickable point: $_"
    }
}

function Set-ElementValue($element, $text) {
    $pattern = $element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $pattern.SetValue($text)
}

switch ($Action) {
    "windows" {
        Get-TopWindows | Where-Object { $_.Name } | ForEach-Object {
            "$($_.ProcessId)`t$($_.ClassName)`t$($_.Name)`t$($_.Handle)"
        }
    }
    "dump" {
        $window = Get-TargetWindow
        "WINDOW $($window.ProcessId) $($window.ClassName) $($window.Name)"
        Dump-Tree $window.Element $Depth
    }
    "foreground" {
        $window = Get-TargetWindow
        [CyronUiaWin32]::ShowWindow([IntPtr]$window.Handle, 9) | Out-Null
        [CyronUiaWin32]::SetForegroundWindow([IntPtr]$window.Handle) | Out-Null
        "FOREGROUND_OK $($window.Handle)"
    }
    "invoke" {
        $window = Get-TargetWindow
        $element = @(Get-Descendants $window.Element) |
            Where-Object {
                $_.Current.Name -match $NameRegex -and
                $_.Current.IsEnabled -and
                -not $_.Current.IsOffscreen
            } |
            Select-Object -First 1
        if (-not $element) {
            Dump-Tree $window.Element $Depth | Write-Output
            throw "No enabled element matched '$NameRegex' in '$($window.Name)'"
        }
        Invoke-Element $element
        "INVOKED '$($element.Current.Name)' in '$($window.Name)'"
    }
    "set-value" {
        $window = Get-TargetWindow
        $element = @(Get-Descendants $window.Element) |
            Where-Object {
                $_.Current.Name -match $NameRegex -and
                $_.Current.IsEnabled -and
                -not $_.Current.IsOffscreen
            } |
            Select-Object -First 1
        if (-not $element) {
            Dump-Tree $window.Element $Depth | Write-Output
            throw "No enabled value element matched '$NameRegex' in '$($window.Name)'"
        }
        Set-ElementValue $element $Value
        "SET '$Value' in '$($element.Current.Name)'"
    }
    "pick-folder" {
        $window = Get-TargetWindow
        $elements = @(Get-Descendants $window.Element)
        $edit = $elements |
            Where-Object {
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit -and
                $_.Current.IsEnabled -and -not $_.Current.IsOffscreen
            } |
            Select-Object -First 1
        if (-not $edit) {
            Dump-Tree $window.Element $Depth | Write-Output
            throw "No folder dialog edit control in '$($window.Name)'"
        }
        Set-ElementValue $edit $Value
        Start-Sleep -Milliseconds 200
        $button = $elements |
            Where-Object {
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
                $_.Current.IsEnabled -and -not $_.Current.IsOffscreen -and
                $_.Current.Name -match "^(Select Folder|Choose|Open|OK)$"
            } |
            Select-Object -First 1
        if (-not $button) {
            Dump-Tree $window.Element $Depth | Write-Output
            throw "No accept button in '$($window.Name)'"
        }
        Invoke-Element $button
        "PICKED '$Value' via '$($button.Current.Name)'"
    }
}
