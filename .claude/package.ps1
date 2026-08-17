# slTerminal 便携版打包脚本
# 用法: .\.claude\package.ps1 [-Version "0.2.0"] [-Debug] [-CopyConpty]
# 产物: slterminal-v{version}-x64.zip（项目根目录；release 包恒含捆绑 ConPTY 两文件）
# -CopyConpty: 构建后把 vendor/conpty 两文件拷贝到 target 目录（debug 手动部署用，ADR-0005）

param(
    [string]$Version = "0.2.0",
    [switch]$Debug,
    [switch]$CopyConpty
)

$ErrorActionPreference = "Stop"
$projectRoot = (Get-Item "$PSScriptRoot\..").FullName
# ADR-0005: 捆绑 ConPTY 宿主（老 Win10 鼠标转发修复，见 src-tauri/vendor/conpty/README.md）
$conptyVendor = "$projectRoot\src-tauri\vendor\conpty"

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

# 2b. -CopyConpty: 拷贝捆绑 ConPTY 到产物目录（debug 手动部署用；运行时缺失则静默回退系统）
if ($CopyConpty) {
    Write-Host "  拷贝捆绑 ConPTY (conpty.dll + OpenConsole.exe) 到 $targetDir ..." -ForegroundColor Yellow
    Copy-Item "$conptyVendor\conpty.dll", "$conptyVendor\OpenConsole.exe" $targetDir -Force
}

# 3. 打包 zip（release 包恒含捆绑 ConPTY；debug 包也纳入，与 exe 同目录生效）
$zipName = "slterminal-v$Version-x64.zip"
$zipPath = "$projectRoot\$zipName"
Write-Host "`n[3/3] 打包 $zipName ..." -ForegroundColor Yellow
Remove-Item $zipPath -ErrorAction SilentlyContinue
$dllPath = "$conptyVendor\conpty.dll"
$openConsolePath = "$conptyVendor\OpenConsole.exe"
if (-not (Test-Path $dllPath)) { throw "找不到捆绑 ConPTY: $dllPath" }
Compress-Archive -Path $exePath, $dllPath, $openConsolePath -DestinationPath $zipPath -CompressionLevel Optimal
$zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 2)

Write-Host "`n=== 完成 ===" -ForegroundColor Green
Write-Host "  $zipPath ($zipSizeMB MB)"

# 4. 验证
Write-Host "`n验证 zip 内容:" -ForegroundColor Yellow
[System.IO.Compression.ZipFile]::OpenRead($zipPath).Entries | ForEach-Object {
    Write-Host "  $($_.Name) ($([math]::Round($_.Length/1MB,2)) MB)"
}
