Add-Type -AssemblyName System.Drawing

$assets = Join-Path $PSScriptRoot '..\assets'
New-Item -ItemType Directory -Force -Path $assets | Out-Null
$path = Join-Path $assets 'goatpad.ico'

$bitmap = [System.Drawing.Bitmap]::new(256, 256)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.Clear([System.Drawing.Color]::FromArgb(28, 52, 40))
$brush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(107, 193, 123))
$graphics.FillRectangle($brush, 26, 22, 204, 212)
$brush.Dispose()
$pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(231, 255, 235), 13)
$graphics.DrawLine($pen, 76, 80, 180, 80)
$graphics.DrawLine($pen, 76, 126, 180, 126)
$graphics.DrawLine($pen, 76, 172, 145, 172)
$pen.Dispose()
$icon = [System.Drawing.Icon]::FromHandle($bitmap.GetHicon())
$stream = [System.IO.File]::Open($path, [System.IO.FileMode]::Create)
$icon.Save($stream)
$stream.Dispose()
$icon.Dispose()
$graphics.Dispose()
$bitmap.Dispose()
