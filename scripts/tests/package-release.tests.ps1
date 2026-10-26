#Requires -Version 7
[CmdletBinding()]
param(
    # 发布版本;缺省回读 slterm_app/Cargo.toml,与打包脚本同一真值源。
    [string] $Version,
    # 产物目录;缺省 <仓根>\dist。
    [string] $Directory,
    # 自测模式:用临时夹具验证预算判定正反例,不需要真实发布产物。
    [switch] $SelfTest
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$repoRoot = (Get-Item (Join-Path $PSScriptRoot '../..')).FullName

# 体积预算钉:校准日期 2026-10-18,全量功能态 release 实测
# (opt-level="s" + 热路径 O3):
#   setup.exe 实测 17899557 字节(17.07 MiB);未启用热路径 O3 时为 15.12 MiB
#   zip       实测 23283760 字节(22.21 MiB);未启用热路径 O3 时为 19.29 MiB
# 预算 = 实测值 × 1.15 后向上取整到字节(裕度不低于 15%):
#   setup 20584491 字节(19.63 MiB);zip 26776324 字节(25.54 MiB)
# 超预算即红;收紧预算永远合法,放宽必须在提交信息说明原因。改动
# [profile.release] 或热路径钉包清单后必须重跑本测试。
$setupBudgetBytes = 20584491
$zipBudgetBytes = 26776324

function Get-ProductVersion {
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'slterm_app/Cargo.toml')
    if ($manifest -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
        throw 'slterm_app/Cargo.toml 缺少 version 字段'
    }
    return $Matches[1]
}

function Get-TomlSection([string] $Text, [string] $Name) {
    $escaped = [regex]::Escape($Name)
    $match = [regex]::Match($Text, "(?ms)^\[$escaped\]\s*\r?\n(.*?)(?=^\[|\z)")
    if (-not $match.Success) {
        throw "根 Cargo.toml 缺少 [$Name] 节"
    }
    return $match.Groups[1].Value
}

# 体积工程配置是预算的前提:数值漂移会让预算失去意义。
function Assert-ReleaseProfile {
    if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
        throw "版本号必须是三数值段:$Version"
    }
    $manifestPath = Join-Path $repoRoot 'Cargo.toml'
    $manifest = Get-Content -Raw -LiteralPath $manifestPath
    $release = Get-TomlSection $manifest 'profile.release'

    $requiredRelease = [ordered]@{
        'lto'           = '"thin"'
        'panic'         = '"unwind"'
        'debug'         = '0'
        'strip'         = '"symbols"'
        'incremental'   = 'false'
        'codegen-units' = '1'
        'opt-level'     = '"s"'
    }
    foreach ($key in $requiredRelease.Keys) {
        $pattern = "(?m)^\s*$([regex]::Escape($key))\s*=\s*$([regex]::Escape($requiredRelease[$key]))\s*$"
        if ($release -notmatch $pattern) {
            throw "[profile.release] $key 必须为 $($requiredRelease[$key])"
        }
    }

    # 热路径钉包清单:渲染/塑形/布局/VT/壳;缺一项或降级 O3 即红。
    $hotPackages = @(
        'resvg', 'rustybuzz', 'taffy', 'ttf-parser', 'smol',
        'gpui', 'gpui-component', 'slterm_terminal', 'image'
    )
    $packages = Get-TomlSection $manifest 'profile.release.package'
    foreach ($package in $hotPackages) {
        $pattern = "(?m)^\s*$([regex]::Escape($package))\s*=\s*\{[^}]*\bopt-level\s*=\s*3\b[^}]*\}\s*$"
        if ($packages -notmatch $pattern) {
            throw "[profile.release.package] 缺少热路径 $package = opt-level 3"
        }
    }
    $mainPattern = '(?m)^\s*slterm\s*=\s*\{[^}]*\bopt-level\s*=\s*3\b[^}]*\bcodegen-units\s*=\s*16\b[^}]*\}\s*$'
    if ($packages -notmatch $mainPattern) {
        throw '[profile.release.package] slterm 必须为 opt-level 3 + codegen-units 16'
    }
}

function Assert-AssetBudget([string] $Path, [long] $Budget, [string] $Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "缺少发布产物:$Path(体积预算钉属发布门禁,请先完成 release 构建)"
    }
    $length = (Get-Item -LiteralPath $Path).Length
    $actualMiB = [math]::Round($length / 1MB, 2)
    $budgetMiB = [math]::Round($Budget / 1MB, 2)
    if ($length -gt $Budget) {
        throw "$Label 超出体积预算:实测 $length 字节($actualMiB MiB)> 预算 $Budget 字节($budgetMiB MiB)"
    }
    Write-Host "  $Label $length 字节($actualMiB MiB)<= 预算 $budgetMiB MiB"
}

function Assert-ReleaseAssets {
    $setupName = "slTerminal-v$Version-windows-x64-setup.exe"
    $zipName = "slTerminal-v$Version-windows-x64.zip"
    Assert-AssetBudget (Join-Path $Directory $setupName) $setupBudgetBytes 'setup.exe'
    Assert-AssetBudget (Join-Path $Directory $zipName) $zipBudgetBytes 'zip'
}

function Invoke-BudgetSelfTest {
    $root = Join-Path ([IO.Path]::GetTempPath()) "slterm-package-tests-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $root | Out-Null
    try {
        $atBudget = Join-Path $root 'at-budget.bin'
        $overBudget = Join-Path $root 'over-budget.bin'
        foreach ($case in @(
            @{ Path = $atBudget; Size = 2MB },
            @{ Path = $overBudget; Size = (2MB + 1) }
        )) {
            $stream = [IO.File]::OpenWrite($case.Path)
            try { $stream.SetLength([long]$case.Size) } finally { $stream.Dispose() }
        }
        Assert-AssetBudget $atBudget (2MB) '夹具边界值'
        $rejected = $false
        try { Assert-AssetBudget $overBudget (2MB) '夹具超预算' }
        catch {
            if ($_.Exception.Message -notlike '*超出体积预算*') { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw '超预算夹具未被拒绝' }
        $missingRejected = $false
        try { Assert-AssetBudget (Join-Path $root 'missing.bin') (2MB) '夹具缺件' }
        catch {
            if ($_.Exception.Message -notlike '*缺少发布产物*') { throw }
            $missingRejected = $true
        }
        if (-not $missingRejected) { throw '缺件夹具未被拒绝' }
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force
    }
}

# 原生依赖自测:内部构建必须静态链接;dumpbin 导入表不得出现 dav1d DLL。
function Invoke-NativeBuildSelfTest {
    . (Join-Path $repoRoot 'scripts/build-windows-product.ps1') -LibraryOnly
    foreach ($name in @('Assert-NoDav1dImport', 'Assert-Dav1dStatic')) {
        if ($null -eq (Get-Command $name -ErrorAction SilentlyContinue)) {
            throw "缺少原生依赖门禁函数:$name"
        }
    }

    Assert-NoDav1dImport -Dependents @('    KERNEL32.dll', '    USER32.dll')
    foreach ($library in @('dav1d.dll', 'libdav1d-7.dll')) {
        $rejected = $false
        try { Assert-NoDav1dImport -Dependents @('    KERNEL32.dll', "    $library") }
        catch {
            if ($_.Exception.Message -notlike '*dav1d 动态依赖*') { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw "dav1d 动态依赖未被拒绝:$library" }
    }

    $configPath = Join-Path $repoRoot '.cargo/config.toml'
    if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
        throw '缺少 dav1d 可复现构建配置:.cargo/config.toml'
    }
    $config = Get-Content -Raw -LiteralPath $configPath
    foreach ($required in @(
        'SYSTEM_DEPS_DAV1D_BUILD_INTERNAL',
        'value = "always"',
        'SYSTEM_DEPS_DAV1D_LINK',
        'value = "static"'
    )) {
        if (-not $config.Contains($required)) {
            throw "dav1d 构建配置缺少:$required"
        }
    }
    if ($script:SourceRoots -notcontains $configPath) {
        throw '构建新鲜度输入缺少 .cargo/config.toml'
    }
    $vendorRoot = Join-Path $repoRoot 'vendor/dav1d-sys'
    if ($script:SourceRoots -notcontains $vendorRoot) {
        throw '构建新鲜度输入缺少 vendor/dav1d-sys'
    }
}

# 新鲜度判据自测:内容摘要 stamp 命中/拒绝、仅改注释重构建不假红、
# stamp 缺失回退 mtime。夹具放 target/ 下(已忽略),dot-source 复用库函数。
function Invoke-FreshnessSelfTest {
    $root = Join-Path $repoRoot "target/freshness-selftest-$([guid]::NewGuid().ToString('N'))"
    $sourceRoot = Join-Path $root 'src'
    $buildRoot = Join-Path $root 'build'
    New-Item -ItemType Directory -Path $sourceRoot, $buildRoot -Force | Out-Null
    try {
        . (Join-Path $repoRoot 'scripts/build-windows-product.ps1') -LibraryOnly
        $source = Join-Path $sourceRoot 'fixture.rs'
        Set-Content -LiteralPath $source -Value 'fn main() {}' -Encoding utf8NoBOM
        $binary = Join-Path $buildRoot 'slterm.exe'
        [IO.File]::WriteAllText($binary, 'stub')
        $roots = @($sourceRoot)

        Write-BuildStamp $buildRoot (Get-SourceDigest $roots)
        Assert-FreshBinaries -Binaries @($binary) -SourceRoots $roots

        Add-Content -LiteralPath $source -Value '// comment only' -Encoding utf8NoBOM
        $rejected = $false
        try { Assert-FreshBinaries -Binaries @($binary) -SourceRoots $roots }
        catch {
            if ($_.Exception.Message -notlike '*源码摘要与构建戳不一致*') { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw '源码变更未被摘要判据拒绝' }

        # 模拟重新构建:stamp 刷新为当前摘要即通过——仅改注释不重链接也不假红。
        Write-BuildStamp $buildRoot (Get-SourceDigest $roots)
        Assert-FreshBinaries -Binaries @($binary) -SourceRoots $roots

        # 旧构建目录(无 stamp)回退 mtime 判据。
        Remove-Item -LiteralPath (Get-BuildStampPath $buildRoot) -Force
        (Get-Item -LiteralPath $binary).LastWriteTimeUtc = [DateTime]::UtcNow.AddDays(-1)
        (Get-Item -LiteralPath $source).LastWriteTimeUtc = [DateTime]::UtcNow
        $rejected = $false
        try { Assert-FreshBinaries -Binaries @($binary) -SourceRoots $roots }
        catch {
            if ($_.Exception.Message -notlike '*早于源码改动*') { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw 'stamp 缺失时 mtime 回退判据失效' }
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force
    }
}

# 清单自测:workspace 成员表与根 Cargo.toml 全等;目录条目受控展开后
# 逐个进包,且与安装器 [Files] wildcard 对齐,包内路径无重复。
function Invoke-ManifestSelfTest {
    . (Join-Path $repoRoot 'scripts/build-windows-product.ps1') -LibraryOnly
    Assert-WorkspaceMembers

    $sources = @(Resolve-PackageSources (Join-Path $repoRoot 'target/manifest-selftest-build'))
    $duplicates = @($sources | Group-Object PackagePath | Where-Object { $_.Count -gt 1 })
    if ($duplicates.Count -ne 0) {
        throw "包内路径重复:$($duplicates.Name -join ', ')"
    }

    $logoDirectory = Join-Path $repoRoot 'docs/featureSpec/assets/cli-svg'
    $logoFiles = @(Get-ChildItem -LiteralPath $logoDirectory -File -Filter '*.svg' | Sort-Object Name)
    if ($logoFiles.Count -eq 0) {
        throw "CLI logo 源目录没有 *.svg:$logoDirectory"
    }
    $logoEntries = @($sources | Where-Object { $_.PackagePath -like 'assets/cli-svg/*.svg' })
    if ($logoEntries.Count -ne $logoFiles.Count) {
        throw "CLI logo 包内条目数与源目录不一致:$($logoEntries.Count) != $($logoFiles.Count)"
    }
    $expectedLogoPaths = @($logoFiles | ForEach-Object { "assets/cli-svg/$($_.Name)" })
    $actualLogoPaths = @($logoEntries | ForEach-Object { $_.PackagePath })
    $logoDifference = @(Compare-Object -ReferenceObject $expectedLogoPaths -DifferenceObject $actualLogoPaths)
    if ($logoDifference.Count -ne 0) {
        $details = $logoDifference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "CLI logo 包内路径不匹配:`n$($details -join "`n")"
    }

    $wildcard = 'Source: "{#RepoRoot}\docs\featureSpec\assets\cli-svg\*.svg"; DestDir: "{app}\assets\cli-svg"; Flags: ignoreversion'
    $installer = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'scripts/installer.iss')
    if (-not $installer.Contains($wildcard)) {
        throw "installer.iss 缺少 CLI logo 收录行:$wildcard"
    }

    $requiredNotices = @(
        [pscustomobject]@{
            PackagePath = 'licenses/THIRD-PARTY-CRATE-NOTICES'
            RepoPath = 'licenses/THIRD-PARTY-CRATE-NOTICES'
            InstallerName = 'licenses\THIRD-PARTY-CRATE-NOTICES'
        },
        [pscustomobject]@{
            PackagePath = 'licenses/LICENSE-DAV1D'
            RepoPath = 'licenses/LICENSE-DAV1D'
            InstallerName = 'licenses\LICENSE-DAV1D'
        }
    )
    foreach ($notice in $requiredNotices) {
        $expectedSource = Join-Path $repoRoot $notice.RepoPath
        $matched = @($sources | Where-Object {
            $_.PackagePath -eq $notice.PackagePath -and $_.Source -eq $expectedSource
        })
        if ($matched.Count -ne 1) {
            throw "共享清单缺少许可条目:$($notice.PackagePath)"
        }
        $line = "Source: `"{#RepoRoot}\$($notice.InstallerName)`"; DestDir: `"{app}\licenses`"; Flags: ignoreversion"
        if (-not $installer.Contains($line)) {
            throw "installer.iss 缺少许可收录行:$line"
        }
    }

    $release = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'scripts/release.ps1')
    if (-not $release.Contains('check_licenses.py') -or
        -not $release.Contains('第三方许可核验失败')) {
        throw '正式发布缺少许可核验或原生命令失败检查'
    }
    if (-not $release.Contains('& $testPath -Version $Version -Directory $OutputDirectory')) {
        throw '发布钉测试未接收版本与隔离产物目录'
    }
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProductVersion
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    throw "版本号必须是三数值段:$Version"
}

Assert-ReleaseProfile

if ($SelfTest) {
    Invoke-NativeBuildSelfTest
    Invoke-ManifestSelfTest
    Invoke-BudgetSelfTest
    Invoke-FreshnessSelfTest
    Write-Output 'package-release.tests.ps1: SELFTEST PASS (native static link, profile pin, workspace/manifest, budget boundary, missing asset, freshness digest)'
    return
}

if ([string]::IsNullOrWhiteSpace($Directory)) {
    $Directory = Join-Path $repoRoot 'dist'
}
$Directory = [IO.Path]::GetFullPath($Directory)
Assert-ReleaseAssets
Write-Output "package-release.tests.ps1: PASS (profile pin and size budget, version $Version)"
