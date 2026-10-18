#Requires -Version 7
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $Version,
    [string] $Directory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$repoRoot = (Get-Item (Join-Path $PSScriptRoot '..')).FullName
if ([string]::IsNullOrWhiteSpace($Directory)) {
    $Directory = Join-Path $repoRoot 'dist'
}
$Directory = [IO.Path]::GetFullPath($Directory)

# 发布资产合同:安装器主形态 + 便携副形态,单名无别名。
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    throw "版本号必须是三数值段:$Version"
}
$MIN_ASSET_SIZE = 1MB
$expected = @(
    "slTerminal-v$Version-windows-x64-setup.exe",
    "slTerminal-v$Version-windows-x64.zip"
)

if (-not (Test-Path -LiteralPath $Directory -PathType Container)) {
    throw "发布目录不存在:$Directory"
}

$actual = @(Get-ChildItem -LiteralPath $Directory -File |
    Where-Object { $_.Name -ne 'SHA256SUMS' } |
    ForEach-Object { $_.Name })
$difference = @(Compare-Object -ReferenceObject @($expected | Sort-Object) `
    -DifferenceObject @($actual | Sort-Object))
if ($difference.Count -ne 0) {
    $details = $difference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
    throw "发布资产名集合不匹配:`n$($details -join "`n")"
}

$checksumLines = foreach ($name in $expected) {
    $path = Join-Path $Directory $name
    $item = Get-Item -LiteralPath $path
    if ($item.Length -lt $MIN_ASSET_SIZE) {
        throw "资产小于下限 1 MiB:$name($($item.Length) 字节)"
    }
    if ($name.EndsWith('.exe', [StringComparison]::OrdinalIgnoreCase)) {
        $bytes = [IO.File]::ReadAllBytes($path)
        if ($bytes.Length -lt 2 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
            throw "资产不是 PE 文件(MZ 魔数缺失):$name"
        }
    }
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "  $name $([math]::Round($item.Length / 1MB, 2)) MB $hash"
    "$hash  $name"
}

# 生成物:进入 release notes 与更新链 digest 校验。
$checksumsPath = Join-Path $Directory 'SHA256SUMS'
$text = ($checksumLines -join "`n") + "`n"
[IO.File]::WriteAllText($checksumsPath, $text, [Text.UTF8Encoding]::new($false))
Write-Output "verify-release.ps1: OK $checksumsPath"
