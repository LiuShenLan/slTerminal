#Requires -Version 7
[CmdletBinding()]
param(
    [string] $Version,
    [ValidateSet('release')][string] $Configuration = 'release',
    [ValidateSet('x64', 'arm64')][string] $Architecture = 'x64',
    [switch] $Force,
    [string] $OutputDirectory,
    [string] $TargetDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$repoRoot = (Get-Item (Join-Path $PSScriptRoot '..')).FullName
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'dist'
}

# 版本缺省回读 slterm_app/Cargo.toml;正式发布由调用方显式传值。
if ([string]::IsNullOrWhiteSpace($Version)) {
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'slterm_app/Cargo.toml')
    if ($manifest -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
        throw 'slterm_app/Cargo.toml 缺少 version 字段'
    }
    $Version = $Matches[1]
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    throw "发布版本号必须是三数值段:$Version"
}

Write-Host "=== slTerminal $Version 发布链 ===" -ForegroundColor Cyan

Write-Host '=== 核验第三方生产依赖许可 ===' -ForegroundColor Cyan
& python (Join-Path $PSScriptRoot 'check_licenses.py')
if ($LASTEXITCODE -ne 0) {
    throw "第三方许可核验失败(exit $LASTEXITCODE)"
}

# 随包 ConPTY 运行时必须与 CI 使用同一固定哈希与 x64 架构。
Write-Host '=== 核验随包 ConPTY 运行时 ===' -ForegroundColor Cyan
& python (Join-Path $PSScriptRoot 'check_bundled_runtime.py')
if ($LASTEXITCODE -ne 0) {
    throw "随包 ConPTY 运行时核验失败(exit $LASTEXITCODE)"
}

# Runtime schema 资产是随包合同;发布前必须与生成器逐字节一致。
Write-Host '=== 核验 Runtime 线缆合同 ===' -ForegroundColor Cyan
& cargo test --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
    -p slterm_runtime_protocol --features schema --test lib_tests -- --test-threads=1
if ($LASTEXITCODE -ne 0) {
    throw "Runtime 线缆合同核验失败(exit $LASTEXITCODE)"
}

# 发布自测:清单/新鲜度/许可/原生静态链接在构建前先跑一遍。
Write-Host '=== 发布自测（清单与新鲜度） ===' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'tests/package-release.tests.ps1') -SelfTest
if ($LASTEXITCODE -ne 0) {
    throw "发布自测失败(exit $LASTEXITCODE)"
}

# 1. 共享构建:安装器与 zip 同源同 feature 图。
& (Join-Path $PSScriptRoot 'build-windows-product.ps1') `
    -Configuration $Configuration -TargetDirectory $TargetDirectory

# 2. 安装器(主形态);3. 便携 zip(副形态)。共享构建已就绪。
& (Join-Path $PSScriptRoot 'build-installer.ps1') -Version $Version `
    -Configuration $Configuration -Architecture $Architecture `
    -SkipBuild -Force:$Force -OutputDirectory $OutputDirectory -TargetDirectory $TargetDirectory

& (Join-Path $PSScriptRoot 'package-release.ps1') -Version $Version `
    -Configuration $Configuration -Architecture $Architecture `
    -SkipBuild -Force:$Force -OutputDirectory $OutputDirectory -TargetDirectory $TargetDirectory

# 4. 产物钉测试:profile、体积预算与缺件检查（失败即停）。
Write-Host '=== 钉测试 package-release.tests.ps1 ===' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'tests/package-release.tests.ps1') `
    -Version $Version -Directory $OutputDirectory
if ($LASTEXITCODE -ne 0) {
    throw "产物钉测试失败(exit $LASTEXITCODE)"
}

# 5. 发布核验:资产集合 + 尺寸 + PE 魔数 + SHA256SUMS。
& (Join-Path $PSScriptRoot 'verify-release.ps1') -Version $Version -Directory $OutputDirectory
if ($LASTEXITCODE -ne 0) {
    throw "发布核验失败(exit $LASTEXITCODE)"
}

# 6. 发布说明 SHA256 块由同一 SHA256SUMS 派生,不手抄。
& (Join-Path $PSScriptRoot 'sync-release-notes.ps1') -Version $Version -Directory $OutputDirectory
if ($LASTEXITCODE -ne 0) {
    throw "发布说明哈希同步失败(exit $LASTEXITCODE)"
}

Write-Host "=== 完成: $OutputDirectory ===" -ForegroundColor Green
Get-ChildItem -LiteralPath $OutputDirectory -File | ForEach-Object {
    Write-Host "  $($_.Name) $([math]::Round($_.Length / 1MB, 2)) MB"
}
