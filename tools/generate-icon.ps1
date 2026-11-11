#requires -Version 5.1
<#
    Génère l'identité visuelle d'ARXFUND :
      assets/arxfund.png        (icône 256 px, utilisée par la fenêtre)
      assets/arxfund.ico        (icône multi-résolutions pour l'exe et l'installateur)
      installer/wizard-large.bmp (image de l'assistant Inno Setup, 164x314)
      installer/wizard-small.bmp (image de l'assistant Inno Setup, 55x55)

    Design original : carré sombre arrondi, anneau violet, « A » lumineux
    et fine forme d'onde. Aucun élément provenant d'un tiers.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root 'assets'
$installer = Join-Path $root 'installer'
New-Item -ItemType Directory -Force -Path $assets, $installer | Out-Null

function New-ArxBitmap {
    param([int]$Size, [switch]$Wizard)

    $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $s = [float]$Size
    $inset = [Math]::Max(2.0, $s * 0.04)

    # Fond sombre arrondi avec léger dégradé diagonal.
    $rect = New-Object System.Drawing.RectangleF($inset, $inset, ($s - 2 * $inset), ($s - 2 * $inset))
    $radius = $s * 0.22
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect,
        [System.Drawing.Color]::FromArgb(255, 13, 13, 22),
        [System.Drawing.Color]::FromArgb(255, 27, 24, 45),
        [System.Drawing.Drawing2D.LinearGradientMode]::ForwardDiagonal)
    $g.FillPath($bg, $path)

    # « A » stylisé : deux jambes + barre, tracé arrondi.
    $violet = [System.Drawing.Color]::FromArgb(255, 167, 139, 250)
    $cyan = [System.Drawing.Color]::FromArgb(255, 34, 211, 238)
    $penWidth = [Math]::Max(2.4, $s * 0.075)
    $pen = New-Object System.Drawing.Pen($violet, $penWidth)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round

    $apex = New-Object System.Drawing.PointF(($s * 0.50), ($s * 0.22))
    $left = New-Object System.Drawing.PointF(($s * 0.24), ($s * 0.76))
    $right = New-Object System.Drawing.PointF(($s * 0.76), ($s * 0.76))
    $g.DrawLine($pen, $apex, $left)
    $cyanPen = New-Object System.Drawing.Pen($cyan, $penWidth)
    $cyanPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $cyanPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawLine($cyanPen, $apex, $right)

    # Barre musicale brisée : signature originale lisible en petit format.
    $barPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 216, 180, 254), [Math]::Max(1.8, $s * 0.052))
    $barPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $barPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawLine($barPen, [float]($s * 0.35), [float]($s * 0.59), [float]($s * 0.46), [float]($s * 0.59))
    $g.DrawLine($barPen, [float]($s * 0.46), [float]($s * 0.59), [float]($s * 0.51), [float]($s * 0.51))
    $g.DrawLine($barPen, [float]($s * 0.51), [float]($s * 0.51), [float]($s * 0.56), [float]($s * 0.59))
    $g.DrawLine($barPen, [float]($s * 0.56), [float]($s * 0.59), [float]($s * 0.65), [float]($s * 0.59))

    $g.Dispose()
    return $bmp
}

function Save-Ico {
    param([string]$Path, [int[]]$Sizes)
    $entries = @()
    foreach ($size in $Sizes) {
        $bmp = New-ArxBitmap -Size $size
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $png = $ms.ToArray()
        $ms.Dispose()
        $bmp.Dispose()
        $entries += [pscustomobject]@{ Size = $size; Data = $png }
    }

    $fs = [System.IO.File]::Create($Path)
    $bw = New-Object System.IO.BinaryWriter($fs)
    $bw.Write([uint16]0)          # reserved
    $bw.Write([uint16]1)          # type = icon
    $bw.Write([uint16]$entries.Count)
    $offset = 6 + 16 * $entries.Count
    foreach ($e in $entries) {
        $dim = if ($e.Size -ge 256) { 0 } else { $e.Size }
        $bw.Write([byte]$dim)
        $bw.Write([byte]$dim)
        $bw.Write([byte]0)        # palette
        $bw.Write([byte]0)        # reserved
        $bw.Write([uint16]1)      # planes
        $bw.Write([uint16]32)     # bpp
        $bw.Write([uint32]$e.Data.Length)
        $bw.Write([uint32]$offset)
        $offset += $e.Data.Length
    }
    foreach ($e in $entries) { $bw.Write($e.Data) }
    $bw.Flush(); $bw.Dispose(); $fs.Dispose()
}

function Save-WizardImage {
    param([int]$Width, [int]$Height, [string]$Path)
    $bmp = New-Object System.Drawing.Bitmap($Width, $Height, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $rect = New-Object System.Drawing.Rectangle(0, 0, $Width, $Height)
    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect,
        [System.Drawing.Color]::FromArgb(255, 11, 11, 16),
        [System.Drawing.Color]::FromArgb(255, 32, 25, 58),
        [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
    $g.FillRectangle($bg, $rect)

    $iconSize = [Math]::Min($Width, [Math]::Min($Height, 120))
    $icon = New-ArxBitmap -Size 256 -Wizard
    $x = [int](($Width - $iconSize) / 2)
    $y = [int]($Height * 0.24)
    $g.DrawImage($icon, $x, $y, $iconSize, $iconSize)
    $icon.Dispose()

    if ($Width -ge 120) {
        $fontSize = [Math]::Max(10, [Math]::Min(18, $Width * 0.13))
        $font = New-Object System.Drawing.Font('Segoe UI', $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
        $fmt = New-Object System.Drawing.StringFormat
        $fmt.Alignment = [System.Drawing.StringAlignment]::Center
        $brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 237, 233, 254))
        $g.DrawString('ARXFUND', $font, $brush, (New-Object System.Drawing.RectangleF(0, ($y + $iconSize + 18), $Width, 40)), $fmt)
        $font.Dispose(); $brush.Dispose(); $fmt.Dispose()
    }

    $g.Dispose()
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}

$icon256 = New-ArxBitmap -Size 256
$icon256.Save((Join-Path $assets 'arxfund.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$icon256.Dispose()
Save-Ico -Path (Join-Path $assets 'arxfund.ico') -Sizes @(16, 24, 32, 48, 64, 128, 256)
Save-WizardImage -Width 164 -Height 314 -Path (Join-Path $installer 'wizard-large.bmp')
Save-WizardImage -Width 55 -Height 55 -Path (Join-Path $installer 'wizard-small.bmp')

Write-Host "Icône et visuels générés :"
Get-ChildItem (Join-Path $assets 'arxfund.*'), (Join-Path $installer 'wizard-*.bmp') |
    ForEach-Object { "  {0} ({1} octets)" -f $_.FullName, $_.Length }
