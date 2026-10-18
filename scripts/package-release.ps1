[CmdletBinding()]
param(
    [string] $Version = 'unreleased',
    [ValidateSet('release')][string] $Configuration = 'release',
    [ValidateSet('x64', 'arm64')][string] $Architecture = 'x64',
    # 仅限脚本自测/发布彩排;正式发布必须全新构建。
    [switch] $SkipBuild,
    [switch] $AllowStale,
    [switch] $Force,
    [string] $OutputDirectory,
    [string] $TargetDirectory
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

if ($Architecture -ne 'x64') {
    throw 'arm64 参数形态已保留,但首发构建只支持 x64。'
}
if ($Version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+-]*$') {
    throw "版本号含非法字符:$Version"
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
$zipName = "slTerminal-v$Version-windows-$Architecture.zip"
$zipPath = Join-Path $OutputDirectory $zipName

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-windows-product.ps1') `
        -Configuration $Configuration -TargetDirectory $TargetDirectory
}

# 清单 → 源文件;任一缺失即红。
$entries = Resolve-PackageSources $buildRoot
foreach ($entry in $entries) {
    if (-not (Test-Path -LiteralPath $entry.Source -PathType Leaf)) {
        throw "缺少收录文件:$($entry.Source)"
    }
}
$expected = @($entries | ForEach-Object { $_.PackagePath })

$binaries = @(
    (Join-Path $buildRoot 'slterm.exe'),
    (Join-Path $buildRoot 'slterm-hook.exe')
)
if ($AllowStale) {
    Write-Warning 'AllowStale:跳过新鲜度检查(仅限脚本自测/发布彩排)'
} else {
    Assert-FreshBinaries -Binaries $binaries -SourceRoots $script:SourceRoots
}

# staging 实际文件集必须与清单全等;条目路径禁逃逸 staging 根。
function Assert-Manifest([string] $Root, [string[]] $Expected) {
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $actual = Get-ChildItem -LiteralPath $rootFull -Recurse -File | ForEach-Object {
        $relative = [IO.Path]::GetRelativePath($rootFull, $_.FullName).Replace('\', '/')
        if ([IO.Path]::IsPathRooted($relative) -or $relative -match '(^|/)\.\.(/|$)') {
            throw "staging 条目逃逸根目录:$relative"
        }
        $relative
    }
    $difference = @(Compare-Object -ReferenceObject @($Expected | Sort-Object) `
        -DifferenceObject @($actual | Sort-Object))
    if ($difference.Count -ne 0) {
        $details = $difference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "staging 清单不匹配:`n$($details -join "`n")"
    }
}

# 只允许删除输出根下的 `.stage-` 前缀路径。
function Remove-StageSafely([string] $Path) {
    $rootFull = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\', '/')
    $targetFull = [IO.Path]::GetFullPath($Path)
    if (-not $targetFull.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "拒绝清理输出根之外的路径:$targetFull"
    }
    if (-not ([IO.Path]::GetFileName($targetFull).StartsWith('.stage-', [StringComparison]::Ordinal))) {
        throw "拒绝清理非 staging 路径:$targetFull"
    }
    if (Test-Path -LiteralPath $targetFull) {
        Remove-Item -LiteralPath $targetFull -Recurse -Force
    }
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
if ((Test-Path -LiteralPath $zipPath) -and -not $Force) {
    throw "便携包已存在:$zipPath;使用 -Force 覆盖"
}

$stageName = ".stage-$([guid]::NewGuid().ToString('N'))"
$stage = Join-Path $OutputDirectory $stageName
$temporaryZip = Join-Path $OutputDirectory "$stageName.zip"
try {
    New-Item -ItemType Directory -Path $stage | Out-Null
    foreach ($entry in $entries) {
        $destination = Join-Path $stage $entry.PackagePath
        $directory = Split-Path -Parent $destination
        if ($directory) {
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }
        Copy-Item -LiteralPath $entry.Source -Destination $destination
    }
    Assert-Manifest -Root $stage -Expected $expected

    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $temporaryZip -CompressionLevel Optimal
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($temporaryZip)
    try {
        $zipEntries = @($archive.Entries | ForEach-Object { $_.FullName.Replace('\', '/') })
    } finally {
        $archive.Dispose()
    }
    $zipDifference = @(Compare-Object -ReferenceObject @($expected | Sort-Object) `
        -DifferenceObject @($zipEntries | Sort-Object))
    if ($zipDifference.Count -ne 0) {
        $details = $zipDifference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "zip 二次读回与清单不匹配:`n$($details -join "`n")"
    }

    # 同卷重命名 = 原子替换;绝不让半包出现在最终路径。
    [IO.File]::Move($temporaryZip, $zipPath, $true)
} finally {
    Remove-StageSafely $stage
    if (Test-Path -LiteralPath $temporaryZip) {
        Remove-StageSafely $temporaryZip
    }
}

$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash
Write-Output "package-release.ps1: OK $zipPath"
Write-Output "SHA256 $hash"
