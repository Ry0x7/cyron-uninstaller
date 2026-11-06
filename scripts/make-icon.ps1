# Renders the Cyron source icon: an ink field with the offset-ring mark.
# Uses only Windows PowerShell 5.1 + System.Drawing so the asset is reproducible
# on any Windows build machine without image tooling.
[CmdletBinding()]
param(
    [string]$Output = "src-tauri/icons/source.png",
    [int]$Size = 1024
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Add-Type -AssemblyName System.Drawing

function New-RoundedPath {
    param([float]$X, [float]$Y, [float]$Width, [float]$Height, [float]$Radius)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $diameter = $Radius * 2
    $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
    $path.AddArc($X + $Width - $diameter, $Y, $diameter, $diameter, 270, 90)
    $path.AddArc($X + $Width - $diameter, $Y + $Height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($X, $Y + $Height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

$bitmap = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
$graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$graphics.Clear([System.Drawing.Color]::Transparent)

$scale = $Size / 1024.0
$ink = [System.Drawing.ColorTranslator]::FromHtml("#12171A")
$teal = [System.Drawing.ColorTranslator]::FromHtml("#45C7A6")

# Ink field: full bleed with a small transparent margin so Windows can breathe
# the icon inside its own tile at every size.
$margin = 32 * $scale
$fieldPath = New-RoundedPath -X $margin -Y $margin -Width ($Size - 2 * $margin) -Height ($Size - 2 * $margin) -Radius (220 * $scale)
$fieldBrush = New-Object System.Drawing.SolidBrush($ink)
$graphics.FillPath($fieldBrush, $fieldPath)

# Ring with a deliberate cut at the top-right: the mark reads as an open loop.
$center = $Size / 2.0
$ringRadius = 262 * $scale
$ringThickness = 86 * $scale
$ringRect = New-Object System.Drawing.RectangleF(
    ($center - $ringRadius),
    ($center - $ringRadius),
    ($ringRadius * 2),
    ($ringRadius * 2)
)
$ringPen = New-Object System.Drawing.Pen($teal, $ringThickness)
$ringPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Flat
$ringPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Flat
$ringPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Miter
$graphics.DrawArc($ringPen, $ringRect, 345, 300)

# Centered dot: the second half of the identity, clear even at 16 px.
$dotRadius = 68 * $scale
$dotBrush = New-Object System.Drawing.SolidBrush($teal)
$graphics.FillEllipse(
    $dotBrush,
    ($center - $dotRadius),
    ($center - $dotRadius),
    ($dotRadius * 2),
    ($dotRadius * 2)
)

$outputPath = [System.IO.Path]::GetFullPath(
    (Join-Path (Get-Location) $Output)
)
$outputDirectory = [System.IO.Path]::GetDirectoryName($outputPath)
if (-not (Test-Path -LiteralPath $outputDirectory)) {
    New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

$bitmap.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)

$dotBrush.Dispose()
$ringPen.Dispose()
$fieldBrush.Dispose()
$fieldPath.Dispose()
$graphics.Dispose()
$bitmap.Dispose()

Write-Output "CYRON_ICON_SOURCE_OK $outputPath ${Size}x${Size}"
