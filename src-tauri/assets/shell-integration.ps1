# slTerminal shell integration — PowerShell
# 注入 OSC 7 (CWD) + OSC 9;9 (ConEmu CWD) + OSC 133 (命令边界)
# 通过 -EncodedCommand 内联执行，不写磁盘

# UTF-8 编码修复（中文 Windows GBK/936 必须）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
& chcp 65001 > $null

$esc = "$([char]0x1b)"

function Write-Osc($code, $text) {
    $Host.UI.Write("$esc]$code;$text$esc\")
}

function Update-Cwd {
    $cwd = $PWD.ProviderPath
    $uri = "file://$($cwd -replace '\\','/')"
    Write-Osc 7 $uri
    Write-Osc "9;9" $cwd
}

function Send-PrePrompt { Write-Osc 133 A }
function Send-PostPrompt { Write-Osc 133 B }

function Send-ExitCode($code) {
    Write-Osc "133;D" $code
}

# 保存原始 prompt，用 OSC 133 包裹
$originalPrompt = $function:prompt

function prompt {
    # 保存 $? 和 LASTEXITCODE（调用原始 prompt 时会污染）
    $origDollarQuestion = $global:?
    $origLastExitCode = $global:LASTEXITCODE

    Send-ExitCode $origLastExitCode
    Send-PrePrompt
    Update-Cwd
    & $originalPrompt
    Send-PostPrompt

    # 恢复 $? 和 LASTEXITCODE
    $global:LASTEXITCODE = $origLastExitCode
    if ($global:? -ne $origDollarQuestion) {
        if ($origDollarQuestion) { 1+1 } else { Write-Error '' -ErrorAction Ignore }
    }
}

Update-Cwd
Send-PrePrompt
