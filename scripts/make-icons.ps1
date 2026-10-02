# Builds the program icons from the press-kit icon PNGs (the dark variant):
#   app/assets/ornatr.ico        Windows icon embedded in ORNATR.exe (16-256 px)
#   app/assets/ornatr-128.rgba   128 x 128 raw RGBA for the window and taskbar icon
# The 16/32/64/128/256 px images are used as drawn; 24 and 48 px (Explorer and the
# taskbar) are resized from the 256 px one.
# Run from the native folder:  powershell -ExecutionPolicy Bypass -File scripts\make-icons.ps1 [-Source <icons folder>]
param([string]$Source = (Join-Path $PSScriptRoot "..\..\press\ORNATR-Press-Kit\assets\icons"))
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$out = Join-Path $PSScriptRoot "..\app\assets"
New-Item -ItemType Directory -Force $out | Out-Null

function Load($size) { Clean ([System.Drawing.Bitmap]::FromFile((Resolve-Path (Join-Path $Source "ORNATR-icon-dark-$size.png")))) }

# The press-kit PNGs carry a white ring round the dark tile (left from a white background).
# From the edges inward, light pixels connected to the outside become transparent, and the
# tile's soft edge is turned from white-matted grey into black with transparency, so the
# rounded corners stay clean on any taskbar. The art inside the tile is enclosed and untouched.
function Clean($src) {
    $w = $src.Width; $h = $src.Height
    $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp); $g.DrawImage($src, 0, 0, $w, $h); $g.Dispose(); $src.Dispose()
    $rect = New-Object System.Drawing.Rectangle 0, 0, $w, $h
    $data = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadWrite, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $px = New-Object byte[] ($w * $h * 4)
    for ($y = 0; $y -lt $h; $y++) { [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]($data.Scan0.ToInt64() + $y * $data.Stride), $px, $y * $w * 4, $w * 4) }
    $light = { param($i) $px[$i + 3] -lt 128 -or (0.114 * $px[$i] + 0.587 * $px[$i + 1] + 0.299 * $px[$i + 2]) -gt 140 }
    $outside = New-Object bool[] ($w * $h)
    $queue = New-Object System.Collections.Generic.Queue[int]
    for ($x = 0; $x -lt $w; $x++) { foreach ($y in 0, ($h - 1)) { $queue.Enqueue($y * $w + $x) } }
    for ($y = 0; $y -lt $h; $y++) { foreach ($x in 0, ($w - 1)) { $queue.Enqueue($y * $w + $x) } }
    while ($queue.Count -gt 0) {
        $p = $queue.Dequeue()
        if ($outside[$p] -or -not (& $light ($p * 4))) { continue }
        $outside[$p] = $true
        $x = $p % $w; $y = [math]::Floor($p / $w)
        if ($x -gt 0) { $queue.Enqueue($p - 1) }; if ($x -lt $w - 1) { $queue.Enqueue($p + 1) }
        if ($y -gt 0) { $queue.Enqueue($p - $w) }; if ($y -lt $h - 1) { $queue.Enqueue($p + $w) }
    }
    for ($p = 0; $p -lt $w * $h; $p++) {
        $i = $p * 4
        if ($outside[$p]) { $px[$i + 3] = 0; continue }
        # the tile's edge: grey from blending with white becomes black with partial alpha
        $x = $p % $w; $y = [math]::Floor($p / $w)
        $edge = ($x -gt 0 -and $outside[$p - 1]) -or ($x -lt $w - 1 -and $outside[$p + 1]) -or ($y -gt 0 -and $outside[$p - $w]) -or ($y -lt $h - 1 -and $outside[$p + $w])
        if ($edge) {
            $luma = 0.114 * $px[$i] + 0.587 * $px[$i + 1] + 0.299 * $px[$i + 2]
            $px[$i + 3] = [byte][math]::Min($px[$i + 3], [math]::Round(255 - $luma)); $px[$i] = 0; $px[$i + 1] = 0; $px[$i + 2] = 0
        }
    }
    for ($y = 0; $y -lt $h; $y++) { [System.Runtime.InteropServices.Marshal]::Copy($px, $y * $w * 4, [IntPtr]($data.Scan0.ToInt64() + $y * $data.Stride), $w * 4) }
    $bmp.UnlockBits($data)
    $bmp
}
# the leading comma returns the byte array whole (PowerShell would otherwise unroll it)
function PngBytes($bmp) { $ms = New-Object System.IO.MemoryStream; $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); ,[byte[]]$ms.ToArray() }
function Resized($bmp, $size) {
    $r = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($r)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($bmp, 0, 0, $size, $size); $g.Dispose(); $r
}

$big = Load 256
$images = [ordered]@{}
foreach ($s in 16, 24, 32, 48, 64, 128, 256) {
    # text keys: an ordered table treats a number key as a position
    if ($s -in 24, 48) { $images["$s"] = PngBytes (Resized $big $s) } else { $b = Load $s; $images["$s"] = PngBytes $b; $b.Dispose() }
}

# ICO: a 6-byte header, a 16-byte entry per image, then the PNG data (PNG entries need Windows Vista or later)
$ico = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ico
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$images.Count)
$offset = 6 + 16 * $images.Count
foreach ($s in $images.Keys) {
    $n = $images[$s].Length; $d = if ([int]$s -ge 256) { 0 } else { [int]$s }
    $w.Write([Byte]$d); $w.Write([Byte]$d); $w.Write([Byte]0); $w.Write([Byte]0)
    $w.Write([UInt16]1); $w.Write([UInt16]32); $w.Write([UInt32]$n); $w.Write([UInt32]$offset)
    $offset += $n
}
foreach ($s in $images.Keys) { $w.Write([byte[]]$images[$s]) }
$w.Flush(); [System.IO.File]::WriteAllBytes((Join-Path $out "ornatr.ico"), $ico.ToArray())

# 128 x 128 RGBA (straight alpha) for the window icon
$b = Load 128
$rect = New-Object System.Drawing.Rectangle 0, 0, 128, 128
$data = $b.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$bgra = New-Object byte[] (128 * 128 * 4)
for ($y = 0; $y -lt 128; $y++) { [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]($data.Scan0.ToInt64() + $y * $data.Stride), $bgra, $y * 512, 512) }
$b.UnlockBits($data); $b.Dispose()
for ($i = 0; $i -lt $bgra.Length; $i += 4) { $t = $bgra[$i]; $bgra[$i] = $bgra[$i + 2]; $bgra[$i + 2] = $t }
[System.IO.File]::WriteAllBytes((Join-Path $out "ornatr-128.rgba"), $bgra)
"Wrote ornatr.ico ($($images.Count) sizes) and ornatr-128.rgba to $out"
