# slTerminal 便携版打包脚本
# 用法: .\.claude\package.ps1 [-Version "0.1.0"] [-Debug]
# 产物: slterminal-v{version}-x64.zip（项目根目录）

param(
    [string]$Version = "0.1.0",
    [switch]$Debug
)

$ErrorActionPreference = "Stop"
$projectRoot = (Get-Item "$PSScriptRoot\..").FullName

Write-Host "=== slTerminal 打包 ===" -ForegroundColor Cyan
Write-Host "  版本: $Version"
Write-Host "  模式: $(if ($Debug) { 'debug' } else { 'release' })"

# 1. 构建
Write-Host "`n[1/3] 构建 Tauri 应用..." -ForegroundColor Yellow
$buildArgs = @("tauri", "build", "--no-bundle")
if ($Debug) { $buildArgs += "--debug" }
Push-Location $projectRoot
try {
    npx @buildArgs
    if ($LASTEXITCODE -ne 0) { throw "构建失败" }
} finally {
    Pop-Location
}

# 2. 定位产物
$targetDir = if ($Debug) { "$projectRoot\src-tauri\target\debug" } else { "$projectRoot\src-tauri\target\release" }
$exePath = "$targetDir\slterminal.exe"
if (-not (Test-Path $exePath)) { throw "找不到产物: $exePath" }
$sizeMB = [math]::Round((Get-Item $exePath).Length / 1MB, 2)
Write-Host "  产物: slterminal.exe ($sizeMB MB)"

# 3. 打包 zip
$zipName = "slterminal-v$Version-x64.zip"
$zipPath = "$projectRoot\$zipName"
Write-Host "`n[3/3] 打包 $zipName ..." -ForegroundColor Yellow
Remove-Item $zipPath -ErrorAction SilentlyContinue
Compress-Archive -Path $exePath -DestinationPath $zipPath -CompressionLevel Optimal
$zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 2)

Write-Host "`n=== 完成 ===" -ForegroundColor Green
Write-Host "  $zipPath ($zipSizeMB MB)"

# 4. 验证
Write-Host "`n验证 zip 内容:" -ForegroundColor Yellow
[System.IO.Compression.ZipFile]::OpenRead($zipPath).Entries | ForEach-Object {
    Write-Host "  $($_.Name) ($([math]::Round($_.Length/1MB,2)) MB)"
}
