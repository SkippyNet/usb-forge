# Renders a preview of the USB Forge boot menu from the real theme files in assets/boot-theme
# (background, 9-slice menu box, selection bar, colours and layout from theme.txt).
# It is an approximation for docs: the boot loader's own font and hint labels are not drawn.
# Usage: powershell -ExecutionPolicy Bypass -File assets/screenshots/make-boot-preview.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$theme = Join-Path (Split-Path -Parent $here) 'boot-theme'
function Img([string]$n) { [Drawing.Image]::FromFile((Join-Path $theme $n)) }
function C([string]$hex) {
    $v = [Convert]::ToInt32($hex.TrimStart('#'), 16)
    [Drawing.Color]::FromArgb(255, ($v -shr 16) -band 255, ($v -shr 8) -band 255, $v -band 255)
}

# Draw a 9-slice pixmap style ("menu_*.png") into a rectangle.
function NineSlice($g, [string]$prefix, [int]$x, [int]$y, [int]$w, [int]$h) {
    $nw = Img "${prefix}_nw.png"; $t = $nw.Height; $l = $nw.Width
    $g.DrawImage((Img "${prefix}_c.png"), $x + $l, $y + $t, $w - 2 * $l, $h - 2 * $t)
    $g.DrawImage((Img "${prefix}_n.png"), $x + $l, $y, $w - 2 * $l, $t)
    $g.DrawImage((Img "${prefix}_s.png"), $x + $l, $y + $h - $t, $w - 2 * $l, $t)
    $g.DrawImage((Img "${prefix}_w.png"), $x, $y + $t, $l, $h - 2 * $t)
    $g.DrawImage((Img "${prefix}_e.png"), $x + $w - $l, $y + $t, $l, $h - 2 * $t)
    $g.DrawImage($nw, $x, $y, $l, $t)
    $g.DrawImage((Img "${prefix}_ne.png"), $x + $w - $l, $y, $l, $t)
    $g.DrawImage((Img "${prefix}_sw.png"), $x, $y + $h - $t, $l, $t)
    $g.DrawImage((Img "${prefix}_se.png"), $x + $w - $l, $y + $h - $t, $l, $t)
}

$W = 1024; $H = 768
$bmp = New-Object Drawing.Bitmap $W, $H
$g = [Drawing.Graphics]::FromImage($bmp)
$g.InterpolationMode = 'NearestNeighbor'
$g.PixelOffsetMode = 'Half'
$g.TextRenderingHint = 'AntiAliasGridFit'
$g.DrawImage((Img 'background.png'), 0, 0, $W, $H)

# boot_menu: left 15%, width 70%, top 30%, height 52%; items 32px high, 2px apart, 6px padding.
$mx = [int]($W * 0.15); $mw = [int]($W * 0.70); $my = [int]($H * 0.30); $mh = [int]($H * 0.52)
NineSlice $g 'menu' $mx $my $mw $mh

$items = @(
    'clonezilla-live-3.3.3-37-amd64.iso', 'debian-13.7.0-amd64-netinst.iso',
    'Fedora-Workstation-Live-45-1.4.x86_64.iso', 'gparted-live-1.8.1-6-amd64.iso',
    'HBCD_PE_x64.iso', 'linuxmint-22.3-cinnamon-64bit.iso', 'mt86plus_9.00_x86_64.iso',
    'rescuezilla-2.6.2-64bit.resolute.iso', 'systemrescue-13.03-amd64.iso',
    'ubuntu-26.04.1-desktop-amd64.iso', 'ubuntu-26.04.1-live-server-amd64.iso'
)
$font = New-Object Drawing.Font 'Segoe UI', 16, ([Drawing.FontStyle]::Regular), ([Drawing.GraphicsUnit]::Pixel)
$pad = 6; $ih = 32; $sp = 2
$ix = $mx + 2 + $pad; $iw = $mw - 4 - 2 * $pad
$y = $my + 2 + $pad
for ($i = 0; $i -lt $items.Count; $i++) {
    $selected = $i -eq 4
    if ($selected) {
        $sw = Img 'select_w.png'
        $g.DrawImage((Img 'select_c.png'), $ix + $sw.Width, $y, $iw - $sw.Width, $ih)
        $g.DrawImage($sw, $ix, $y, $sw.Width, $ih)
        $color = C '#ff7a45'
    } else { $color = C '#e6e8ee' }
    $sz = $g.MeasureString($items[$i], $font)
    $g.DrawString($items[$i], $font, (New-Object Drawing.SolidBrush $color), [float]($ix + 12), [float]($y + ($ih - $sz.Height) / 2))
    $y += $ih + $sp
}

$bmp.Save((Join-Path $here 'boot-menu.png'), [Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
