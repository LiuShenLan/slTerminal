[CmdletBinding()]
param(
    [string] $Version,
    [ValidateSet('release')][string] $Configuration = 'release',
    # arm64 参数形态保留(iss 已参数化),构建暂不启用。
    [ValidateSet('x64', 'arm64')][string] $Architecture = 'x64',
    # 仅限脚本自测/发布彩排;正式发布必须全新构建。
    [switch] $SkipBuild,
    [switch] $AllowStale,
    [switch] $Force,
    [switch] $ValidateOnly,
    # 夹具模式:隔离 AppId/AppName/输出名,自测不污染真实卸载项。
    [switch] $AcceptanceFixture,
    [string] $OutputDirectory,
    [string] $TargetDirectory,
    [string] $InnoCompiler
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

# dot-source 会在本脚本作用域绑定库脚本的同名参数;先保存再恢复调用方值。
$requestedConfiguration = $Configuration
$requestedTarget = $TargetDirectory
. (Join-Path $PSScriptRoot 'build-windows-product.ps1') -LibraryOnly
$Configuration = $requestedConfiguration
$TargetDirectory = $requestedTarget

# 中文翻译固定上游提交 + SHA256:内容被替换时拒绝使用。
$translationCommit = '97c4aa92235f44d0ee6617d739eb9ab0bdb404bf'
$translationSha256 = 'E6320B32D29054AA9B79FF701569323D78E91169DD7123DEB876BF5A18B5F120'
$translationUrl = "https://api.github.com/repos/jrsoftware/issrc/contents/Files/Languages/ChineseSimplified.isl?ref=$translationCommit"

if ($Architecture -ne 'x64') {
    throw 'arm64 参数形态已保留,但首发构建只支持 x64。'
}
if ([string]::IsNullOrWhiteSpace($TargetDirectory)) {
    $TargetDirectory = $script:DefaultTargetDirectory
} else {
    $TargetDirectory = [IO.Path]::GetFullPath($TargetDirectory)
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $script:RepoRoot 'dist'
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$buildRoot = Get-BuildRoot $TargetDirectory $Configuration

# 版本缺省回读 slterm_app/Cargo.toml;脚本零硬编码版本。
function Get-ProductVersion {
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $script:RepoRoot 'slterm_app/Cargo.toml')
    if ($manifest -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
        throw 'slterm_app/Cargo.toml 缺少 version 字段'
    }
    return $Matches[1]
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProductVersion
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$') {
    throw "版本号格式非法:$Version"
}

# 版本号数字段 = 四段纯数字(安装器 VersionInfoVersion)。
$numericParts = @((($Version -split '[-+]')[0]) -split '\.')
while ($numericParts.Count -lt 4) {
    $numericParts += '0'
}
$numericVersion = ($numericParts[0..3] -join '.')

# 包内置 CHANGELOG 前置断言:构建前必须完成本次版本小节。
$changelogPath = Join-Path $script:RepoRoot 'CHANGELOG.md'
if (-not (Test-Path -LiteralPath $changelogPath -PathType Leaf)) {
    throw "缺少 $changelogPath"
}
$changelog = Get-Content -Raw -LiteralPath $changelogPath
if ($changelog -notmatch "(?m)^##\s+\[?$([regex]::Escape($Version))\]?(\s|$)") {
    throw "CHANGELOG.md 缺少 $Version 小节;发布前必须完成双语说明"
}

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-windows-product.ps1') `
        -Configuration $Configuration -TargetDirectory $TargetDirectory
}

# 必填收录文件检查:与 installer.iss [Files] 同一清单真值源。
$entries = Resolve-PackageSources $buildRoot
foreach ($entry in $entries) {
    if (-not (Test-Path -LiteralPath $entry.Source -PathType Leaf)) {
        throw "缺少收录文件:$($entry.Source)"
    }
}

$binaries = @(
    (Join-Path $buildRoot 'slterm.exe'),
    (Join-Path $buildRoot 'slterm-hook.exe')
)
if ($AllowStale) {
    Write-Warning 'AllowStale:跳过新鲜度检查(仅限脚本自测/发布彩排)'
} else {
    Assert-FreshBinaries -Binaries $binaries -SourceRoots $script:SourceRoots
}

# 版本回读:防「装进去的是旧 exe」。
$reported = (& ($binaries[0]) '--version' 2>&1 | Out-String).Trim()
if ($reported -notmatch [regex]::Escape($Version)) {
    throw "slterm.exe --version 输出与目标版本不符: $reported"
}
Write-Host "  版本回读: $reported"

# isl pin:缺失/校验失败才下载,下载后再校验;仍不符则拒绝。
$toolsDirectory = Join-Path $TargetDirectory 'installer-tools'
$islPath = Join-Path $toolsDirectory 'ChineseSimplified.isl'
function Test-PinnedFile([string] $Path) {
    return (Test-Path -LiteralPath $Path -PathType Leaf) -and
        ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $translationSha256)
}
if ($Force -or -not (Test-PinnedFile $islPath)) {
    New-Item -ItemType Directory -Path $toolsDirectory -Force | Out-Null
    $temporary = Join-Path $toolsDirectory "ChineseSimplified.$([guid]::NewGuid().ToString('N')).tmp"
    try {
        Invoke-WebRequest -Uri $translationUrl -OutFile $temporary -UseBasicParsing -TimeoutSec 120 `
            -Headers @{ 'User-Agent' = 'slterm'; 'Accept' = 'application/vnd.github.raw' }
        if ((Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash -ne $translationSha256) {
            throw '下载的 ChineseSimplified.isl SHA256 与固定值不符,拒绝使用'
        }
        Move-Item -LiteralPath $temporary -Destination $islPath -Force
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}
Write-Host "  isl pin: $islPath"

# ISCC 探测序:显式参数 → $env:ISCC_PATH → 用户级安装 → 系统级候选。
function Find-InnoCompiler {
    $candidates = @($InnoCompiler, $env:ISCC_PATH)
    if ($env:LOCALAPPDATA) {
        $candidates += Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6/ISCC.exe'
    }
    if ($env:ProgramFiles) {
        $candidates += Join-Path $env:ProgramFiles 'Inno Setup 6/ISCC.exe'
    }
    $programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
    if (-not [string]::IsNullOrWhiteSpace($programFilesX86)) {
        $candidates += Join-Path $programFilesX86 'Inno Setup 6/ISCC.exe'
    }
    foreach ($candidate in $candidates) {
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and
            (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return (Get-Item -LiteralPath $candidate).FullName
        }
    }
    throw '未找到 Inno Setup 6 的 ISCC.exe;请安装或用 -InnoCompiler 指定'
}

$iscc = Find-InnoCompiler
Write-Host "  ISCC: $iscc"

if ($ValidateOnly) {
    Write-Output "build-installer.ps1: VALIDATE OK ($Version, $Architecture)"
    return
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$suffix = if ($AcceptanceFixture) { '-acceptance' } else { '' }
$setupName = "slTerminal-v$Version-windows-$Architecture$suffix-setup.exe"
$setupPath = Join-Path $OutputDirectory $setupName
if ((Test-Path -LiteralPath $setupPath) -and -not $Force) {
    throw "安装器已存在:$setupPath;使用 -Force 覆盖"
}

$isccArguments = @(
    "/DAppVersion=$Version",
    "/DNumericVersion=$numericVersion",
    "/DConfiguration=$Configuration",
    "/DArchitecture=$Architecture",
    "/DRepoRoot=$($script:RepoRoot)",
    "/DBuildRoot=$buildRoot",
    "/DOutputDir=$OutputDirectory",
    "/DIslFile=$islPath",
    (Join-Path $PSScriptRoot 'installer.iss')
)
if ($AcceptanceFixture) {
    $isccArguments = @('/DAcceptanceFixture') + $isccArguments
}

& $iscc @isccArguments
if ($LASTEXITCODE -ne 0) {
    throw "ISCC 编译失败(exit $LASTEXITCODE)"
}
if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf)) {
    throw "ISCC 完成但缺少产物:$setupPath"
}
$hash = (Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash
Write-Output "build-installer.ps1: OK $setupPath"
Write-Output "SHA256 $hash"
