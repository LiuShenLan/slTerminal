<#
  ============================================================================
  gen-app-icon.ps1 —— slTerminal 应用图标母版绘制脚本（图标品牌唯一源稿）
  ============================================================================
  用途：
    Windows exe 资源图标 / 任务栏 / Alt-Tab / favicon 使用的应用图标均派生自
    本脚本产出的 1024 母版 app-icon.png——视觉复刻标题栏 app logo
    （TitleBar.tsx 的 LOGO_PATH "M5 8l6 5-6 5M13 19h7"：深蓝灰圆角色块
    内嵌亮蓝 >_ 终端提示符，配色取自 theme/schemes/linear.ts token）。

  用法（在仓库根执行）：
    1. powershell -ExecutionPolicy Bypass -File src-tauri/assets/app-icon/gen-app-icon.ps1
       → 产出 src-tauri/assets/app-icon/app-icon.png（1024 母版）
         + public/app-icon.png（64px，index.html favicon 源，与母版同源免重复维护）
    2. npx tauri icon src-tauri/assets/app-icon/app-icon.png
       → 覆盖生成 src-tauri/icons/ 全套（含 icon.ico 多尺寸，tauri-build 自动嵌入 exe）

  调参：修改下方参数区后重跑即再生成；改色时同步核对 theme token 值。
  ============================================================================
#>

# --- 参数区 ----------------------------------------------------------------
$Canvas       = 1024     # 母版边长（tauri icon 要求 >= 1024 方形）
$Inset        = 64       # 色块与画布边距（块 = Canvas - 2*Inset）
$CornerRadius = 210      # 色块圆角半径（复刻标题栏 16px 块圆角 4 的 25% 比例取整微调）
$BlockColor   = "#202633"  # 色块底：rgba(110,159,242,0.13) 叠 titlebarBg #141416 的混合取整
                            #   —— 半透明底无法入 ICO，故固化为不透明深蓝灰
$SymbolColor  = "#8fb4f5"  # 符号 >_：= theme ui.accentFg（linear.ts，改色须同步）
$GridScale    = 38       # 24 网格 → 画布像素倍率（符号实体宽 570 ≈ 色块的 64%）
$StrokeWidth  = 114      # 笔画宽（24 网格 ≈ 3.0，16px 档降采样后 ≈1.8px 保证可辨；
                          #   标题栏原 2.4/15 符号宽比 16% → 此处 ~20% 适度加粗）
# ---------------------------------------------------------------------------

Add-Type -AssemblyName System.Drawing

$scriptRoot = $PSScriptRoot                                   # src-tauri/assets/app-icon
$repoRoot   = Split-Path (Split-Path (Split-Path $scriptRoot -Parent) -Parent) -Parent
$outMaster  = Join-Path $scriptRoot "app-icon.png"            # 1024 母版（入库）
$outFavicon = Join-Path (Join-Path $repoRoot "public") "app-icon.png"  # 64px favicon 源

$BlockColorF = [System.Drawing.ColorTranslator]::FromHtml($BlockColor)
$SymbolColorF = [System.Drawing.ColorTranslator]::FromHtml($SymbolColor)

# 圆角矩形 GraphicsPath（x,y 左上 / w,h 宽高 / r 圆角半径）
function New-RoundedRectPath([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $p.AddArc($x, $y, $d, $d, 180, 90)
  $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
  $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $p.CloseFigure()
  return $p
}

# 24 网格坐标 → 画布坐标：符号视觉几何中心 (12.5, 13.5) 对齐画布中心 (512, 512)
# 坐标源 = TitleBar.tsx LOGO_PATH：chevron (5,8)->(11,13)->(5,18)，underscore (13,19)->(20,19)
function Map-GridPoint([float]$gx, [float]$gy) {
  return New-Object System.Drawing.PointF(
    (512 + ($gx - 12.5) * $GridScale),
    (512 + ($gy - 13.5) * $GridScale)
  )
}

# --- 绘制 1024 母版 ---------------------------------------------------------
$bmp = New-Object System.Drawing.Bitmap($Canvas, $Canvas, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode  = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality

# 1) 色块：不透明深蓝灰圆角方块（外缘抗锯齿平滑）
$blockSize = $Canvas - 2 * $Inset
$blockPath = New-RoundedRectPath $Inset $Inset $blockSize $blockSize $CornerRadius
$blockBrush = New-Object System.Drawing.SolidBrush($BlockColorF)
$g.FillPath($blockBrush, $blockPath)

# 2) 符号：亮蓝 >_ 终端提示符（圆头线帽 / 圆角连接，同 LOGO_PATH 造型）
$pen = New-Object System.Drawing.Pen($SymbolColorF, $StrokeWidth)
$pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$pen.EndCap   = [System.Drawing.Drawing2D.LineCap]::Round
$pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round

# chevron「>」：两条等长斜线折返
$chevron = New-Object System.Drawing.Drawing2D.GraphicsPath
$chevron.AddLines(@(
  (Map-GridPoint 5 8),
  (Map-GridPoint 11 13),
  (Map-GridPoint 5 18)
))
$g.DrawPath($pen, $chevron)

# underscore「_」：下划线
$u1 = Map-GridPoint 13 19
$u2 = Map-GridPoint 20 19
$g.DrawLine($pen, $u1, $u2)

$pen.Dispose(); $chevron.Dispose(); $blockBrush.Dispose(); $blockPath.Dispose()
$bmp.Save($outMaster, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "已生成 1024 母版: $outMaster"

# --- 派生 64px favicon（public/app-icon.png，index.html 引用）--------------
$fav = New-Object System.Drawing.Bitmap(64, 64, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g2 = [System.Drawing.Graphics]::FromImage($fav)
$g2.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g2.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
$g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g2.DrawImage($bmp, 0, 0, 64, 64)
$fav.Save($outFavicon, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "已生成 favicon: $outFavicon"

$g.Dispose(); $g2.Dispose(); $bmp.Dispose(); $fav.Dispose()
