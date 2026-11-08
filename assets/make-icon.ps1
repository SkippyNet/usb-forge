# Regenerates assets/icon.png (256px) and assets/icon.ico (16-256px) for USB Forge.
# Usage: powershell -ExecutionPolicy Bypass -File assets/make-icon.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function New-IconBitmap([int]$size) {
    $s = $size / 256.0
    $bmp = New-Object Drawing.Bitmap $size, $size, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.Clear([Drawing.Color]::Transparent)

    # Rounded tile with a subtle vertical gradient.
    $r = 56 * $s; $m = 8 * $s; $w = $size - 2 * $m
    $path = New-Object Drawing.Drawing2D.GraphicsPath
    $path.AddArc($m, $m, $r, $r, 180, 90)
    $path.AddArc($m + $w - $r, $m, $r, $r, 270, 90)
    $path.AddArc($m + $w - $r, $m + $w - $r, $r, $r, 0, 90)
    $path.AddArc($m, $m + $w - $r, $r, $r, 90, 90)
    $path.CloseFigure()
    $bg = New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.PointF]::new(0, $m)), ([Drawing.PointF]::new(0, $m + $w)), ([Drawing.Color]::FromArgb(255, 0x26, 0x2b, 0x3a)), ([Drawing.Color]::FromArgb(255, 0x0f, 0x11, 0x17))
    $g.FillPath($bg, $path)
    $g.DrawPath((New-Object Drawing.Pen ([Drawing.Color]::FromArgb(255, 0x3a, 0x42, 0x55)), ([float](3 * $s))), $path)

    # USB plug: metal connector on top, body below.
    $metal = New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 0xc9, 0xcf, 0xdb))
    $dark = New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 0x26, 0x2b, 0x3a))
    $g.FillRectangle($metal, [float](92 * $s), [float](40 * $s), [float](72 * $s), [float](52 * $s))
    $g.FillRectangle($dark, [float](106 * $s), [float](54 * $s), [float](14 * $s), [float](14 * $s))
    $g.FillRectangle($dark, [float](136 * $s), [float](54 * $s), [float](14 * $s), [float](14 * $s))
    $body = New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.PointF]::new(0, 92 * $s)), ([Drawing.PointF]::new(0, 216 * $s)), ([Drawing.Color]::FromArgb(255, 0xff, 0x8c, 0x5a)), ([Drawing.Color]::FromArgb(255, 0xe8, 0x55, 0x2a))
    $g.FillRectangle($body, [float](76 * $s), [float](92 * $s), [float](104 * $s), [float](124 * $s))

    # Lightning bolt cut into the body.
    $bolt = [Drawing.PointF[]]@(
        [Drawing.PointF]::new(138 * $s, 104 * $s),
        [Drawing.PointF]::new(102 * $s, 162 * $s),
        [Drawing.PointF]::new(126 * $s, 162 * $s),
        [Drawing.PointF]::new(116 * $s, 206 * $s),
        [Drawing.PointF]::new(156 * $s, 142 * $s),
        [Drawing.PointF]::new(131 * $s, 142 * $s))
    $g.FillPolygon((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 0x0f, 0x11, 0x17))), $bolt)
    $g.Dispose()
    return $bmp
}

$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = foreach ($sz in $sizes) {
    $b = New-IconBitmap $sz
    $ms = New-Object IO.MemoryStream
    $b.Save($ms, [Drawing.Imaging.ImageFormat]::Png)
    if ($sz -eq 256) { $b.Save((Join-Path $here 'icon.png'), [Drawing.Imaging.ImageFormat]::Png) }
    $b.Dispose()
    , $ms.ToArray()
}

# ICO container with PNG-compressed entries (supported since Windows Vista).
$out = New-Object IO.MemoryStream
$wr = New-Object IO.BinaryWriter $out
$wr.Write([uint16]0); $wr.Write([uint16]1); $wr.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $sz = $sizes[$i]; $len = $pngs[$i].Length
    $dim = if ($sz -ge 256) { 0 } else { $sz }
    $wr.Write([byte]$dim); $wr.Write([byte]$dim); $wr.Write([byte]0); $wr.Write([byte]0)
    $wr.Write([uint16]1); $wr.Write([uint16]32); $wr.Write([uint32]$len); $wr.Write([uint32]$offset)
    $offset += $len
}
foreach ($p in $pngs) { $wr.Write($p) }
$wr.Flush()
[IO.File]::WriteAllBytes((Join-Path $here 'icon.ico'), $out.ToArray())
"Wrote icon.png and icon.ico"
