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

# 4. 钉测试(M11 随迁件;存在时强制执行,失败即停)。
foreach ($test in @('installer.tests.ps1', 'package-release.tests.ps1')) {
    $testPath = Join-Path $PSScriptRoot "tests/$test"
    if (Test-Path -LiteralPath $testPath -PathType Leaf) {
        Write-Host "=== 钉测试 $test ===" -ForegroundColor Cyan
        & $testPath
    }
}

# 5. 发布核验:资产集合 + 尺寸 + PE 魔数 + SHA256SUMS。
& (Join-Path $PSScriptRoot 'verify-release.ps1') -Version $Version -Directory $OutputDirectory

Write-Host "=== 完成: $OutputDirectory ===" -ForegroundColor Green
Get-ChildItem -LiteralPath $OutputDirectory -File | ForEach-Object {
    Write-Host "  $($_.Name) $([math]::Round($_.Length / 1MB, 2)) MB"
}
