#Requires -Version 7
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $Version,
    [string] $Directory,
    [string] $NotePath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$repoRoot = (Get-Item (Join-Path $PSScriptRoot '..')).FullName
if ([string]::IsNullOrWhiteSpace($Directory)) {
    $Directory = Join-Path $repoRoot 'dist'
}
if ([string]::IsNullOrWhiteSpace($NotePath)) {
    $NotePath = Join-Path $repoRoot "release-notes/v$Version.md"
}
$Directory = [IO.Path]::GetFullPath($Directory)
$NotePath = [IO.Path]::GetFullPath($NotePath)

$checksumsPath = Join-Path $Directory 'SHA256SUMS'
if (-not (Test-Path -LiteralPath $checksumsPath -PathType Leaf)) {
    throw "缺少 SHA256SUMS:$checksumsPath"
}
if (-not (Test-Path -LiteralPath $NotePath -PathType Leaf)) {
    throw "缺少发布说明:$NotePath"
}

# SHA256SUMS 是唯一真值源;发布说明块只由本脚本派生,不手抄。
$entries = @(Get-Content -LiteralPath $checksumsPath | ForEach-Object {
    if ($_ -match '^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$') {
        [pscustomobject]@{ Hash = $Matches[1].ToLowerInvariant(); Name = $Matches[2] }
    }
} | Sort-Object Name)
if ($entries.Count -eq 0) {
    throw "SHA256SUMS 没有可解析条目:$checksumsPath"
}

$block = [Text.StringBuilder]::new()
[void]$block.AppendLine('**SHA256**')
[void]$block.AppendLine()
[void]$block.AppendLine('由 `scripts/verify-release.ps1` 在发布构建后生成 `SHA256SUMS` 并填入；')
[void]$block.AppendLine('禁止手抄。预期条目：')
[void]$block.AppendLine()
foreach ($entry in $entries) {
    [void]$block.AppendLine("- ``$($entry.Name)``: ``$($entry.Hash)``")
}

$text = Get-Content -Raw -LiteralPath $NotePath
$marker = '**SHA256**'
$index = $text.IndexOf($marker, [StringComparison]::Ordinal)
if ($index -lt 0) {
    throw "发布说明缺少 SHA256 块标记:$NotePath"
}
$updated = $text.Substring(0, $index) + $block.ToString()

# 原子替换:同目录临时文件 + Move-Item。
$temporary = "$NotePath.tmp-$([guid]::NewGuid().ToString('N'))"
try {
    [IO.File]::WriteAllText($temporary, $updated, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporary -Destination $NotePath -Force
} finally {
    if (Test-Path -LiteralPath $temporary) {
        Remove-Item -LiteralPath $temporary -Force
    }
}
Write-Output "sync-release-notes.ps1: OK ($($entries.Count) assets -> $NotePath)"
