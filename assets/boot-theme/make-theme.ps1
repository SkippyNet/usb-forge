# Regenerates the USB Forge boot menu theme images in this folder (theme.txt is hand-written).
# Colours match src/theme.rs. Usage: powershell -ExecutionPolicy Bypass -File assets/boot-theme/make-theme.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function C([string]$hex, [int]$a = 255) {
    $v = [Convert]::ToInt32($hex.TrimStart('#'), 16)
    [Drawing.Color]::FromArgb($a, ($v -shr 16) -band 255, ($v -shr 8) -band 255, $v -band 255)
}
$BG = C '0f1117'; $PANEL = C '151821'; $CARD_HOVER = C '222734'; $BORDER = C '2a303e'
$TEXT = C 'e6e8ee'; $MUTED = C '8b93a7'; $ACCENT = C 'ff7a45'; $DEEP = C '0b0d12'; $THUMB = C '4a5368'

function Save([Drawing.Bitmap]$bmp, [string]$name) {
    $bmp.Save((Join-Path $here $name), [Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

function Solid([string]$name, [int]$w, [int]$h, [Drawing.Color]$color) {
    $bmp = New-Object Drawing.Bitmap $w, $h, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.Clear($color)
    $g.Dispose()
    Save $bmp $name
}

# A 9-slice box: 1px-wide border pieces (2x2 corners) around a stretched centre.
function Box([string]$prefix, [Drawing.Color]$edge, [Drawing.Color]$fill, [int]$t = 2) {
    foreach ($p in 'nw', 'ne', 'sw', 'se') { Solid "${prefix}_$p.png" $t $t $edge }
    foreach ($p in 'n', 's') { Solid "${prefix}_$p.png" 4 $t $edge }
    foreach ($p in 'w', 'e') { Solid "${prefix}_$p.png" $t 4 $edge }
    Solid "${prefix}_c.png" 4 4 $fill
}

# ---- background: dark gradient, logo + wordmark in the top quarter ----------------------
$W = 1024; $H = 768
$bmp = New-Object Drawing.Bitmap $W, $H, ([Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.InterpolationMode = 'HighQualityBicubic'
$g.TextRenderingHint = 'AntiAliasGridFit'
$grad = New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.PointF]::new(0, 0)), ([Drawing.PointF]::new(0, $H)), $PANEL, $BG
$g.FillRectangle($grad, 0, 0, $W, $H)

# Soft warm glow behind the logo.
$glow = New-Object Drawing.Drawing2D.GraphicsPath
$glow.AddEllipse(312, -140, 400, 340)
$pgb = New-Object Drawing.Drawing2D.PathGradientBrush $glow
$pgb.CenterColor = (C 'ff7a45' 38)
$pgb.SurroundColors = [Drawing.Color[]]@((C 'ff7a45' 0))
$g.FillPath($pgb, $glow)

$icon = [Drawing.Image]::FromFile((Join-Path (Split-Path -Parent $here) 'icon.png'))
$title = 'USB Forge'
$font = New-Object Drawing.Font 'Segoe UI Semibold', 40, ([Drawing.FontStyle]::Regular), ([Drawing.GraphicsUnit]::Pixel)
$size = $g.MeasureString($title, $font)
$iconSize = 76; $gap = 14
$x = [int](($W - ($iconSize + $gap + $size.Width)) / 2)
$g.DrawImage($icon, $x, 58, $iconSize, $iconSize)
$g.DrawString($title, $font, (New-Object Drawing.SolidBrush $TEXT), [float]($x + $iconSize + $gap), [float](96 - $size.Height / 2))
$icon.Dispose()

$sub = New-Object Drawing.Font 'Segoe UI', 16, ([Drawing.FontStyle]::Regular), ([Drawing.GraphicsUnit]::Pixel)
$fmt = New-Object Drawing.StringFormat
$fmt.Alignment = 'Center'
$g.DrawString('Choose an operating system or tool to start', $sub, (New-Object Drawing.SolidBrush $MUTED), [Drawing.RectangleF]::new(0, 152, $W, 24), $fmt)

# Thin accent rule under the heading.
$g.FillRectangle((New-Object Drawing.SolidBrush $ACCENT), [int]($W / 2 - 28), 186, 56, 3)
$g.Dispose()
Save $bmp 'background.png'

# ---- menu, selection, scrollbar, command-line box ---------------------------------------
Box 'menu' $BORDER $PANEL
Solid 'select_c.png' 4 4 $CARD_HOVER
Solid 'select_w.png' 4 4 $ACCENT
Solid 'slider_n.png' 10 2 $THUMB
Solid 'slider_c.png' 10 4 $THUMB
Solid 'slider_s.png' 10 2 $THUMB
Box 'terminal_box' $BORDER $DEEP
