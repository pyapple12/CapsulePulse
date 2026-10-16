# 从设计源派生 Tauri 图标五件套 + 多尺寸预览（**入库脚本**，住在 core/icons/；
# 原置于不入库的 .temp/，随审计修复批迁入 —— 迁入时已修好"根目录推导"与"临时产物落址"）
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# 脚本住在 core/icons/ ⇒ 图标目录 = 脚本自身所在；项目根 = 上两级
$icons = $PSScriptRoot
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$srcPath = Join-Path $root '.reference\CapsulePulse图标设计方案.png'

$src = [System.Drawing.Image]::FromFile($srcPath)

# —— 1. 设计源入库（照 CT：source.png 2048²）——
$src.Save((Join-Path $icons 'source.png'), [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "source.png 已写入"

# —— 2. 派生各尺寸 PNG（高质量重采样；不加内边距 = 满幅缩放）——
function New-Scaled([System.Drawing.Image]$image, [int]$size, [string]$out) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)
  $g.DrawImage($image, (New-Object System.Drawing.Rectangle 0, 0, $size, $size))
  $g.Dispose()
  $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

$targets = @{ '32x32.png' = 32; '64x64.png' = 64; '128x128.png' = 128; '128x128@2x.png' = 256; 'icon.png' = 512 }
foreach ($k in $targets.Keys) {
  New-Scaled $src $targets[$k] (Join-Path $icons $k)
  Write-Output ("{0} = {1}x{1}" -f $k, $targets[$k])
}

# —— 3. 多尺寸 ICO（PNG 负载，Vista+ 原生支持；含 16/24/32/48/64/128/256）——
# **帧序有语义、勿改回升序**（PATCH013）：Tauri 编译期只取 `entries()[0]`（目录首帧）当默认
# 窗口图标，且是**单张**位图 ⇒ 首帧是 16×16 时，任务栏 24px / Alt+Tab 32px 只能放大它 ⇒ 必糊；
# 首帧取 64×64（≥ 任何界面所需尺寸）则全程只缩不放。Windows 走 exe 资源时按尺寸挑帧，帧序无关。
$icoSizes = @(64, 48, 32, 24, 16, 128, 256)
# 中间产物落 .temp/（AGENTS 临时文件纪律：仓库内不留 ico-tmp）
$tmpDir = Join-Path $root '.temp\ico-tmp'
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
$blobs = @()
foreach ($s in $icoSizes) {
  $p = Join-Path $tmpDir "icon-$s.png"
  New-Scaled $src $s $p
  $blobs += ,@{ Size = $s; Bytes = [System.IO.File]::ReadAllBytes($p) }
}

$icoPath = Join-Path $icons 'icon.ico'
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
# ICONDIR
$bw.Write([UInt16]0); $bw.Write([UInt16]1); $bw.Write([UInt16]$blobs.Count)
# ICONDIRENTRY（偏移 = 6 + 16×N）
$offset = 6 + 16 * $blobs.Count
foreach ($b in $blobs) {
  $dim = if ($b.Size -ge 256) { 0 } else { $b.Size }   # 256 记作 0
  $bw.Write([Byte]$dim); $bw.Write([Byte]$dim); $bw.Write([Byte]0); $bw.Write([Byte]0)
  $bw.Write([UInt16]1); $bw.Write([UInt16]32)
  $bw.Write([UInt32]$b.Bytes.Length); $bw.Write([UInt32]$offset)
  $offset += $b.Bytes.Length
}
foreach ($b in $blobs) { $bw.Write($b.Bytes) }
$bw.Flush(); $bw.Dispose(); $fs.Dispose()
Remove-Item $tmpDir -Recurse -Force
Write-Output ("icon.ico 已写入（{0} 帧：{1}）" -f $blobs.Count, ($icoSizes -join '/'))

# —— 4. 多尺寸预览图（浅底 / 深底实际尺寸 + 16/24/32 放大 8 倍）——
$bgLight = [System.Drawing.Color]::FromArgb(255, 245, 245, 247)
$bgDark = [System.Drawing.Color]::FromArgb(255, 29, 29, 31)
$w = 980; $h = 430
$pv = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($pv)
$g.Clear([System.Drawing.Color]::White)
$font = New-Object System.Drawing.Font 'Consolas', 11
$brushDark = [System.Drawing.Brushes]::Black
$brushLight = [System.Drawing.Brushes]::White

$showSizes = @(16, 24, 32, 48, 64, 128)
# 行 1：浅底（实际尺寸）
$g.FillRectangle((New-Object System.Drawing.SolidBrush $bgLight), 0, 0, $w, 130)
$x = 20
foreach ($s in $showSizes) {
  $img = New-Object System.Drawing.Bitmap (Join-Path $icons 'source.png')
  $g.DrawImage($img, (New-Object System.Drawing.Rectangle $x, (100 - $s), $s, $s))
  $g.DrawString("$s", $font, $brushDark, $x, 4)
  $x += [Math]::Max($s, 40) + 24
  $img.Dispose()
}
# 行 2：深底（实际尺寸）
$g.FillRectangle((New-Object System.Drawing.SolidBrush $bgDark), 0, 140, $w, 130)
$x = 20
foreach ($s in $showSizes) {
  $img = New-Object System.Drawing.Bitmap (Join-Path $icons 'source.png')
  $g.DrawImage($img, (New-Object System.Drawing.Rectangle $x, (240 - $s), $s, $s))
  $g.DrawString("$s", $font, $brushLight, $x, 144)
  $x += [Math]::Max($s, 40) + 24
  $img.Dispose()
}
# 行 3：16/24/32 放大 8 倍（最近邻，看小尺寸糊不糊）
$g.DrawString("16/24/32 放大 8x", $font, $brushDark, 20, 278)
$x = 20
foreach ($s in @(16, 24, 32)) {
  $big = New-Object System.Drawing.Bitmap $s, $s
  $gb = [System.Drawing.Graphics]::FromImage($big)
  $gb.DrawImage([System.Drawing.Image]::FromFile($srcPath), (New-Object System.Drawing.Rectangle 0, 0, $s, $s))
  $gb.Dispose()
  $zoom = New-Object System.Drawing.Bitmap ($s * 8), ($s * 8)
  $gz = [System.Drawing.Graphics]::FromImage($zoom)
  $gz.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
  $gz.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
  $gz.DrawImage($big, 0, 0, ($s * 8), ($s * 8))
  $gz.Dispose()
  $g.DrawImage($zoom, $x, 300)
  $x += ($s * 8) + 24
  $big.Dispose(); $zoom.Dispose()
}
$g.Dispose()
# 预览图落 .temp/（同上：仓库内不留产物）
$pvPath = Join-Path $root '.temp\icon-preview.png'
$pv.Save($pvPath, [System.Drawing.Imaging.ImageFormat]::Png)
$pv.Dispose()
$src.Dispose()
Write-Output "预览图 = $pvPath"

# —— 5. 校验 ——
Write-Output "`n=== core/icons 产物 ==="
Get-ChildItem $icons -File | Select-Object Name, Length | Format-Table -AutoSize
# **必须校验"目录首帧"**（= Tauri 取作窗口图标的那一帧），不要用 .NET 的 Icon 默认帧——
# .NET 按自己的默认尺寸挑帧，当初就是它报"默认帧 = 16x16"让人误判无害（PATCH013 实测）。
$bytes = [System.IO.File]::ReadAllBytes($icoPath)
$frameCount = [BitConverter]::ToUInt16($bytes, 4)
$head = $bytes[6]; if ($head -eq 0) { $head = 256 }
$order = @()
for ($i = 0; $i -lt $frameCount; $i++) { $o = 6 + $i * 16; $w = $bytes[$o]; if ($w -eq 0) { $w = 256 }; $order += $w }
Write-Output ("icon.ico 帧序 = {0}；目录首帧 = {1}x{1}（须 ≥ 48，否则任务栏会放大它）" -f ($order -join '/'), $head)
