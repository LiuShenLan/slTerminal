#Requires -Version 7
<#
.SYNOPSIS
    生成并校验文档资产预览副本。
.DESCRIPTION
    从 docs/featureSpec/assets/{icons,cli-svg} 生成 docs/featureSpec/assets/preview 下的
    暗色预览：图标在根 <svg> 标签注入 color="#E6E6E6"；CLI logo 仅处理单色/灰阶子集，
    按 README 的颜色映射替换为暗色可读色值。原文其余字节与换行风格保持不变。

    -Write 生成/覆盖预期预览并清理不属于预期集合的旧文件。
    -Check（默认）字节级比较预期集合与实际集合，差异以非零码退出。
#>
[CmdletBinding(DefaultParameterSetName = 'Check')]
param(
    [Parameter(ParameterSetName = 'Write', HelpMessage = '生成/覆盖预览并清理多余文件')]
    [switch]$Write,

    [Parameter(ParameterSetName = 'Check', HelpMessage = '校验预览与预期集合一致（默认）')]
    [switch]$Check
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$assetsRoot = Join-Path $repoRoot 'docs/featureSpec/assets'
$iconsSource = Join-Path $assetsRoot 'icons'
$cliSource = Join-Path $assetsRoot 'cli-svg'
$previewRoot = Join-Path $assetsRoot 'preview'

$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$invariantIgnoreCase = [System.StringComparer]::OrdinalIgnoreCase

# 根标签直接注入 color 的 CLI：grok、kilocode、pi
$rootColorCliNames = [System.Collections.Generic.HashSet[string]]::new($invariantIgnoreCase)
foreach ($name in @('grok', 'kilocode', 'pi')) {
    [void]$rootColorCliNames.Add($name)
}

# 预览集合只包含以下 9 个单色/灰阶 CLI；彩色 logo 不生成预览
$cliPreviewNames = @(
    'cline',
    'codex',
    'cursor',
    'github-copilot',
    'goose',
    'grok',
    'kilocode',
    'opencode',
    'pi'
)

# 非根标签注入型 CLI 的颜色字面量映射；匹配不区分大小写，且不吞掉更长色值
$cliLiteralColors = @{
    'cline'          = [ordered]@{ '#000000' = '#E6E6E6'; '#000' = '#E6E6E6' }
    'codex'          = [ordered]@{ '#000000' = '#E6E6E6'; '#000' = '#E6E6E6' }
    'github-copilot' = [ordered]@{ '#000000' = '#E6E6E6'; '#000' = '#E6E6E6' }
    'goose'          = [ordered]@{ '#101010' = '#E6E6E6' }
    'opencode'       = [ordered]@{ '#211E1E' = '#E6E6E6'; '#CFCECD' = '#3A3A3A' }
    'cursor'         = [ordered]@{
        '#43413C' = '#E6E6E6'
        '#55544F' = '#C8C8C8'
        '#72716D' = '#A8A8A8'
        '#D6D5D2' = '#5A5A5A'
        '#FFFFFF' = '#3A3A3A'
        '#FFF'    = '#3A3A3A'
    }
}

function Read-SvgText {
    param([Parameter(Mandatory)][string]$Path)

    return [System.IO.File]::ReadAllText($Path, $utf8NoBom)
}

function Find-RootSvgTag {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Path
    )

    $match = [regex]::Match($Text, '<svg\b[^>]*>', [System.Text.RegularExpressions.RegexOptions]::Singleline)
    if (-not $match.Success) {
        throw "未找到根 <svg> 标签：$Path"
    }
    return $match
}

# 在根标签结束前注入 color；已有 color 属性则只替换其值
function Set-RootColor {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Path
    )

    $root = Find-RootSvgTag -Text $Text -Path $Path
    $colorAttribute = '(?<![\w-])color\s*=\s*("[^"]*"|''[^'']*'')'
    if ([regex]::IsMatch($root.Value, $colorAttribute)) {
        $newRoot = [regex]::Replace($root.Value, $colorAttribute, 'color="#E6E6E6"')
    }
    else {
        $insertAt = $root.Length - 1
        if ($insertAt -gt 0 -and $root.Value[$insertAt - 1] -eq '/') {
            $insertAt--
        }
        $newRoot = $root.Value.Insert($insertAt, ' color="#E6E6E6"')
    }

    return $Text.Remove($root.Index, $root.Length).Insert($root.Index, $newRoot)
}

# 仅替换完整颜色字面量：后接十六进制字符时不匹配，避免 #000 误伤 #000000
function Replace-ColorLiteral {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$From,
        [Parameter(Mandatory)][string]$To
    )

    $pattern = [regex]::Escape($From) + '(?![0-9A-Fa-f])'
    return [regex]::Replace(
        $Text,
        $pattern,
        $To,
        [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    )
}

function Convert-IconPreview {
    param([Parameter(Mandatory)][string]$Path)

    $text = Read-SvgText -Path $Path
    return Set-RootColor -Text $text -Path $Path
}

# Cursor 的亮度遮罩内白色保持原样，遮罩外的白色参与反相
function Replace-CursorColors {
    param([Parameter(Mandatory)][string]$Text)

    $maskPattern = '<mask\b[^>]*>.*?</mask>'
    $colors = $cliLiteralColors['cursor']
    $builder = [System.Text.StringBuilder]::new()
    $cursor = 0

    $maskMatches = [regex]::Matches(
        $Text,
        $maskPattern,
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    foreach ($mask in $maskMatches) {
        $segment = $Text.Substring($cursor, $mask.Index - $cursor)
        foreach ($from in $colors.Keys) {
            $segment = Replace-ColorLiteral -Text $segment -From $from -To $colors[$from]
        }
        [void]$builder.Append($segment)
        [void]$builder.Append($mask.Value)
        $cursor = $mask.Index + $mask.Length
    }

    $tail = $Text.Substring($cursor)
    foreach ($from in $colors.Keys) {
        $tail = Replace-ColorLiteral -Text $tail -From $from -To $colors[$from]
    }
    [void]$builder.Append($tail)

    return $builder.ToString()
}

function Convert-CliPreview {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Path
    )

    $text = Read-SvgText -Path $Path
    if ($rootColorCliNames.Contains($Name)) {
        return Set-RootColor -Text $text -Path $Path
    }
    if ($Name -eq 'cursor') {
        return Replace-CursorColors -Text $text
    }

    foreach ($from in $cliLiteralColors[$Name].Keys) {
        $text = Replace-ColorLiteral -Text $text -From $from -To $cliLiteralColors[$Name][$from]
    }
    return $text
}

# 预期集合：全部 icons + 9 个指定 CLI，键为相对 preview 的路径（正斜杠）
function Get-ExpectedPreviewFiles {
    $expected = @{}

    if (-not (Test-Path -LiteralPath $iconsSource -PathType Container)) {
        throw "图标源目录不存在：$iconsSource"
    }
    foreach ($file in Get-ChildItem -LiteralPath $iconsSource -Filter '*.svg' -File) {
        $text = Convert-IconPreview -Path $file.FullName
        $expected["icons/$($file.Name)"] = $utf8NoBom.GetBytes($text)
    }

    foreach ($name in $cliPreviewNames) {
        $sourcePath = Join-Path $cliSource "$name.svg"
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "CLI 源文件缺失：$sourcePath"
        }
        $text = Convert-CliPreview -Name $name -Path $sourcePath
        $expected["cli-svg/$name.svg"] = $utf8NoBom.GetBytes($text)
    }

    return $expected
}

function Test-BytesEqual {
    param(
        [Parameter(Mandatory)][byte[]]$Left,
        [Parameter(Mandatory)][byte[]]$Right
    )

    if ($Left.Length -ne $Right.Length) {
        return $false
    }
    for ($i = 0; $i -lt $Left.Length; $i++) {
        if ($Left[$i] -ne $Right[$i]) {
            return $false
        }
    }
    return $true
}

# 递归列出 preview 下的实际文件，键为相对路径（正斜杠）
function Get-ActualPreviewFiles {
    $actual = @{}
    if (-not (Test-Path -LiteralPath $previewRoot -PathType Container)) {
        return $actual
    }
    foreach ($file in Get-ChildItem -LiteralPath $previewRoot -File -Recurse) {
        $relative = $file.FullName.Substring($previewRoot.Length + 1).Replace('\', '/')
        $actual[$relative] = $file.FullName
    }
    return $actual
}

function Invoke-PreviewCheck {
    param([Parameter(Mandatory)][hashtable]$Expected)

    $actual = Get-ActualPreviewFiles
    $issues = [System.Collections.Generic.List[string]]::new()

    foreach ($relative in $Expected.Keys) {
        if (-not $actual.ContainsKey($relative)) {
            $issues.Add("缺失：preview/$relative")
            continue
        }
        $actualBytes = [System.IO.File]::ReadAllBytes($actual[$relative])
        if (-not (Test-BytesEqual -Left $Expected[$relative] -Right $actualBytes)) {
            $issues.Add("内容不同：preview/$relative")
        }
    }
    foreach ($relative in $actual.Keys) {
        if (-not $Expected.ContainsKey($relative)) {
            $issues.Add("多余：preview/$relative")
        }
    }

    if ($issues.Count -gt 0) {
        Write-Host '资产预览校验失败：'
        foreach ($issue in $issues) {
            Write-Host "  $issue"
        }
        Write-Host "共 $($issues.Count) 项差异；运行 scripts/generate-asset-previews.ps1 -Write 重新生成。"
        exit 1
    }

    Write-Host "资产预览校验通过：$($Expected.Count) 个文件。"
}

function Invoke-PreviewWrite {
    param([Parameter(Mandatory)][hashtable]$Expected)

    $written = 0
    foreach ($relative in $Expected.Keys) {
        $target = Join-Path $previewRoot ($relative.Replace('/', [System.IO.Path]::DirectorySeparatorChar))
        $targetDir = Split-Path -Parent $target
        if (-not (Test-Path -LiteralPath $targetDir -PathType Container)) {
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
        }
        # 内容已一致时跳过写入，避免无意义的 mtime 变动
        if (Test-Path -LiteralPath $target -PathType Leaf) {
            $existing = [System.IO.File]::ReadAllBytes($target)
            if (Test-BytesEqual -Left $Expected[$relative] -Right $existing) {
                continue
            }
        }
        [System.IO.File]::WriteAllBytes($target, $Expected[$relative])
        $written++
    }

    $removed = 0
    $actual = Get-ActualPreviewFiles
    foreach ($relative in $actual.Keys) {
        if (-not $Expected.ContainsKey($relative)) {
            Remove-Item -LiteralPath $actual[$relative] -Force
            $removed++
        }
    }

    Write-Host "资产预览已生成：预期 $($Expected.Count) 个文件，写入 $written 个，清理 $removed 个旧文件。"
}

$expected = Get-ExpectedPreviewFiles

if ($PSCmdlet.ParameterSetName -eq 'Write') {
    Invoke-PreviewWrite -Expected $expected
}
else {
    Invoke-PreviewCheck -Expected $expected
}
