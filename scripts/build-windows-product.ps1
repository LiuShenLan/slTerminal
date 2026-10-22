#Requires -Version 7
[CmdletBinding()]
param(
    [ValidateSet('debug', 'release')][string] $Configuration = 'release',
    # 默认 <仓根>\target;显式隔离 CARGO_TARGET_DIR,避免覆盖正在运行实例的产物。
    [string] $TargetDirectory,
    # 只加载函数与清单表(供 build-installer / package-release 复用),不构建。
    [switch] $LibraryOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$script:RepoRoot = (Get-Item (Join-Path $PSScriptRoot '..')).FullName
if ([string]::IsNullOrWhiteSpace($TargetDirectory)) {
    $TargetDirectory = Join-Path $script:RepoRoot 'target'
}
# 注意:不使用 $script:TargetDirectory——它和本脚本参数同名,dot-source 时
# 调用方恢复参数值会把它一并清空。
$script:DefaultTargetDirectory = [IO.Path]::GetFullPath($TargetDirectory)

# 新鲜度判定源码根:workspace 成员 + 随仓数据集 + 根清单。
# workspace 成员变更(根 Cargo.toml [workspace].members)时同步此表。
$script:WorkspaceMembers = @(
    'slterm_app',
    'slterm_terminal',
    'slterm_completions',
    'slterm_hook',
    'slterm_settings',
    'slterm_split',
    'slterm_runtime_protocol'
)
$script:SourceRoots = @(
    $script:WorkspaceMembers | ForEach-Object { Join-Path $script:RepoRoot $_ }
) + @(
    (Join-Path $script:RepoRoot 'slterm_agent_detection'),
    (Join-Path $script:RepoRoot 'Cargo.toml'),
    (Join-Path $script:RepoRoot 'Cargo.lock')
)

# 包内清单唯一真值源:便携 zip 的目录布局与安装器 [Files] 共同消费。
# PackagePath 是包内相对路径;BuildName 取构建目录,RepoPath 取仓库根。
# 目录条目只允许登记在 PackageDirectories,由 Resolve-PackageSources 受控
# 展开为逐个文件;展开结果仍属于本清单,安装器/便携 staging 必须全等。
$script:PackageEntries = @(
    [pscustomobject]@{ PackagePath = 'slterm.exe'; BuildName = 'slterm.exe' }
    [pscustomobject]@{ PackagePath = 'runtime/slterm-hook.exe'; BuildName = 'slterm-hook.exe' }
    [pscustomobject]@{ PackagePath = 'README.md'; RepoPath = 'README.md' }
    [pscustomobject]@{ PackagePath = 'README.zh-CN.md'; RepoPath = 'README.zh-CN.md' }
    [pscustomobject]@{ PackagePath = 'docs/CHANGELOG.md'; RepoPath = 'CHANGELOG.md' }
    [pscustomobject]@{ PackagePath = 'assets/runtime-control-api.md'; RepoPath = 'assets/runtime-control-api.md' }
    [pscustomobject]@{ PackagePath = 'assets/runtime-api-v1.schema.json'; RepoPath = 'assets/runtime-api-v1.schema.json' }
    [pscustomobject]@{ PackagePath = 'assets/app-icon.png'; RepoPath = 'assets/app-icon.png' }
    [pscustomobject]@{ PackagePath = 'assets/app-icon.ico'; RepoPath = 'assets/app-icon.ico' }
    [pscustomobject]@{ PackagePath = 'skills/slterm-runtime/SKILL.md'; RepoPath = 'assets/skills/slterm-runtime/SKILL.md' }
    [pscustomobject]@{ PackagePath = 'skills/slterm-runtime/agents/openai.yaml'; RepoPath = 'assets/skills/slterm-runtime/agents/openai.yaml' }
    [pscustomobject]@{ PackagePath = 'licenses/LICENSE'; RepoPath = 'LICENSE' }
    [pscustomobject]@{ PackagePath = 'licenses/THIRD-PARTY-NOTICES'; RepoPath = 'THIRD-PARTY-NOTICES' }
    [pscustomobject]@{ PackagePath = 'licenses/LICENSE-LATIN-MODERN-MATH'; RepoPath = 'licenses/LICENSE-LATIN-MODERN-MATH' }
)

# 目录条目:受控展开——只允许在此登记,展开后逐个进入包内清单。
# RepoDir 取仓库根,PackageDir 是包内目录,Filter 限定收录文件名模式。
$script:PackageDirectories = @(
    [pscustomobject]@{ RepoDir = 'docs/featureSpec/assets/cli-svg'; PackageDir = 'assets/cli-svg'; Filter = '*.svg' }
)

function Get-BuildRoot([string] $TargetRoot, [string] $Configuration) {
    return Join-Path ([IO.Path]::GetFullPath($TargetRoot)) $Configuration
}

function Get-NewestSourceTime([string[]] $Roots) {
    $newest = [DateTime]::MinValue
    foreach ($root in $Roots) {
        if (-not (Test-Path -LiteralPath $root)) {
            throw "新鲜度判定的源码根不存在:$root"
        }
        $item = Get-Item -LiteralPath $root
        if ($item.PSIsContainer) {
            $files = Get-ChildItem -LiteralPath $root -Recurse -File -Include '*.rs', '*.toml'
        } else {
            $files = @($item)
        }
        foreach ($file in $files) {
            if ($file.LastWriteTimeUtc -gt $newest) {
                $newest = $file.LastWriteTimeUtc
            }
        }
    }
    return $newest
}

# 源码内容摘要:全部参与构建的 .rs/.toml + 根 Cargo.lock 的路径+内容哈希,
# 汇总成单一 SHA256。判据用内容而非 mtime——仅改注释也会变 mtime,但 cargo
# 不重链接,拿时间戳比较会假红。
function Get-SourceDigest([string[]] $Roots) {
    $entries = [System.Collections.Generic.List[object]]::new()
    foreach ($root in $Roots) {
        if (-not (Test-Path -LiteralPath $root)) {
            throw "新鲜度判定的源码根不存在:$root"
        }
        $item = Get-Item -LiteralPath $root
        if ($item.PSIsContainer) {
            $files = Get-ChildItem -LiteralPath $root -Recurse -File -Include '*.rs', '*.toml'
        } else {
            $files = @($item)
        }
        foreach ($file in $files) {
            $relative = [IO.Path]::GetRelativePath($script:RepoRoot, $file.FullName).Replace('\', '/')
            $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
            $entries.Add([pscustomobject]@{ Path = $relative; Hash = $hash })
        }
    }
    $canonical = ($entries | Sort-Object Path | ForEach-Object { "$($_.Path)`0$($_.Hash)" }) -join "`n"
    $bytes = [Text.Encoding]::UTF8.GetBytes($canonical + "`n")
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return [Convert]::ToHexString($sha.ComputeHash($bytes))
    } finally {
        $sha.Dispose()
    }
}

function Get-BuildStampPath([string] $BuildRoot) {
    return Join-Path $BuildRoot '.build-stamp'
}

# 构建成功后的源码快照:记录「构建开始前」的摘要,构建期间源码再变即报红。
function Write-BuildStamp([string] $BuildRoot, [string] $Digest) {
    $stamp = Get-BuildStampPath $BuildRoot
    [IO.File]::WriteAllText($stamp, $Digest + "`n", [Text.UTF8Encoding]::new($false))
}

# 新鲜度断言:有 stamp 时比对内容摘要;旧构建目录无 stamp 时回退 mtime 判据。
function Assert-FreshBinaries([string[]] $Binaries, [string[]] $SourceRoots) {
    if ($Binaries.Count -eq 0) {
        return
    }
    $buildRoot = Split-Path -Parent $Binaries[0]
    $stamp = Get-BuildStampPath $buildRoot
    foreach ($binary in $Binaries) {
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw "缺少构建产物:$binary"
        }
    }
    if (Test-Path -LiteralPath $stamp -PathType Leaf) {
        $recorded = (Get-Content -LiteralPath $stamp -Raw).Trim()
        $current = Get-SourceDigest $SourceRoots
        if ($recorded -ne $current) {
            throw "构建产物早于源码改动(源码摘要与构建戳不一致);请重新构建"
        }
        return
    }
    # 兼容旧构建目录(无 stamp):基准取源码最新 mtime。
    $newest = Get-NewestSourceTime $SourceRoots
    foreach ($binary in $Binaries) {
        $built = (Get-Item -LiteralPath $binary).LastWriteTimeUtc
        if ($built -lt $newest) {
            throw "构建产物早于源码改动(${binary}: $built < $newest);请重新构建"
        }
    }
}

# workspace 成员表必须与根 Cargo.toml 全等,防新鲜度判据漏掉新增成员。
function Assert-WorkspaceMembers {
    $manifestPath = Join-Path $script:RepoRoot 'Cargo.toml'
    $manifest = Get-Content -Raw -LiteralPath $manifestPath
    $workspace = [regex]::Match($manifest, '(?ms)^\[workspace\]\s*\r?\n(.*?)(?=^\[|\z)')
    if (-not $workspace.Success) {
        throw '根 Cargo.toml 缺少 [workspace] 节'
    }
    $members = [regex]::Match($workspace.Groups[1].Value, '(?ms)^\s*members\s*=\s*\[(.*?)\]')
    if (-not $members.Success) {
        throw '根 Cargo.toml [workspace] 缺少 members 数组'
    }
    $declared = @([regex]::Matches($members.Groups[1].Value, '"([^"]+)"') |
        ForEach-Object { $_.Groups[1].Value })
    $difference = @(Compare-Object -ReferenceObject @($script:WorkspaceMembers | Sort-Object) `
        -DifferenceObject @($declared | Sort-Object))
    if ($difference.Count -ne 0) {
        $details = $difference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "Workspace 成员表与根 Cargo.toml 不一致(脚本表为 Reference):`n$($details -join "`n")"
    }
}

# 清单 → 源文件绝对路径;两脚本同表派生,防多一件少一件。
# 目录条目按文件名排序展开;包内路径重复即拒绝。
function Resolve-PackageSources([string] $BuildRoot) {
    $resolved = [System.Collections.Generic.List[object]]::new()
    foreach ($entry in $script:PackageEntries) {
        $source = if ($entry.PSObject.Properties['BuildName']) {
            Join-Path $BuildRoot $entry.BuildName
        } else {
            Join-Path $script:RepoRoot $entry.RepoPath
        }
        $resolved.Add([pscustomobject]@{ PackagePath = $entry.PackagePath; Source = $source })
    }
    foreach ($directory in $script:PackageDirectories) {
        $sourceDirectory = Join-Path $script:RepoRoot $directory.RepoDir
        if (-not (Test-Path -LiteralPath $sourceDirectory -PathType Container)) {
            throw "目录清单源不存在:$sourceDirectory"
        }
        $files = @(Get-ChildItem -LiteralPath $sourceDirectory -File -Filter $directory.Filter |
            Sort-Object Name)
        foreach ($file in $files) {
            $resolved.Add([pscustomobject]@{
                PackagePath = "$($directory.PackageDir)/$($file.Name)"
                Source      = $file.FullName
            })
        }
    }
    $duplicates = @($resolved | Group-Object PackagePath | Where-Object Count -gt 1 |
        ForEach-Object Name)
    if ($duplicates.Count -ne 0) {
        throw "包内路径重复:$($duplicates -join ', ')"
    }
    return @($resolved)
}

# 加载即校验:脚本内成员表是新鲜度与哈希判据的输入,漂移必须立即报红。
Assert-WorkspaceMembers

if ($LibraryOnly) {
    return
}

$buildRoot = Get-BuildRoot $script:DefaultTargetDirectory $Configuration
$buildArgs = @(
    'build', '--locked', '--timings',
    '-p', 'slterm', '--bin', 'slterm',
    '-p', 'slterm_hook', '--bin', 'slterm-hook'
)
if ($Configuration -eq 'release') {
    $buildArgs += '--release'
}

Write-Host "=== 构建 slterm 产品($Configuration)===" -ForegroundColor Cyan
Write-Host "  CARGO_TARGET_DIR: $($script:DefaultTargetDirectory)"
# 构建前抓源码摘要;构建成功后写入构建目录,供后续打包阶段比对。
$sourceDigest = Get-SourceDigest $script:SourceRoots
$previousTarget = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = $script:DefaultTargetDirectory
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build 失败(exit $LASTEXITCODE)"
    }
} finally {
    $env:CARGO_TARGET_DIR = $previousTarget
}

foreach ($name in @('slterm.exe', 'slterm-hook.exe')) {
    $path = Join-Path $buildRoot $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "构建完成但缺少产物:$path"
    }
    Write-Host "  $name $([math]::Round((Get-Item -LiteralPath $path).Length / 1MB, 2)) MB"
}
Write-BuildStamp $buildRoot $sourceDigest
Write-Output "build-windows-product.ps1: OK ($Configuration -> $buildRoot)"
