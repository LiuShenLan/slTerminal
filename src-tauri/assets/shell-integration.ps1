# slTerminal shell integration — PowerShell
# 注入 OSC 7 (CWD) + OSC 9;9 (ConEmu CWD) + OSC 133 (命令边界)
# 要求: UTF-8 BOM 编码 (PS 5.1 硬性)
# 嵌套检测留待 Phase 5：当前无 SLTERM_PANE_ID 环境变量，始终注入

# UTF-8 编码修复 (中文 Windows GBK/936 必须)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
& chcp 65001 > $null

# Escape 字符 (用 [char]0x1b 兼容 PS 5.1, 不用 `e)
$esc = "$([char]0x1b)"

# ---- OSC 辅助函数 ----
function Write-Osc($code, $text) {
    $Host.UI.Write("$esc]$code;$text$esc\")
}

# 发送 CWD (OSC 7 + OSC 9;9)
function Update-Cwd {
    $cwd = $PWD.ProviderPath
    # OSC 7: file:// URI (跨平台事实标准)
    $uri = "file://$($cwd -replace '\\','/')"
    Write-Osc 7 $uri
    # OSC 9;9: Windows 原生路径 (ConEmu 起源, Windows Terminal 默认)
    Write-Osc "9;9" $cwd
}

# OSC 133 A: 提示符前 (PrePrompt)
function Send-PrePrompt {
    Write-Osc 133 A
}

# OSC 133 B: 命令输入开始 (PostPrompt)
function Send-PostPrompt {
    Write-Osc 133 B
}

# OSC 133 D: 命令退出码
function Send-ExitCode($code) {
    if ($code -ne 0) {
        Write-Osc "133;D" $code
    }
}

# ---- Prompt 装饰器 ----
# 保存原始 prompt 函数
$originalPrompt = $function:prompt

# 新 prompt 函数: 在用户 prompt 前后包裹 OSC 序列
function prompt {
    $lastExit = $global:LASTEXITCODE
    Send-ExitCode $lastExit
    Send-PrePrompt
    Update-Cwd
    # 调用原始 prompt (用户自定义或默认)
    & $originalPrompt
    Send-PostPrompt
}

# OSC 133 C: 命令执行前 (PreCommandLookupAction)
# 在 PSReadLine 命令接受前发送
if (Get-Module -ListAvailable -Name PSReadLine) {
    try {
        Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
            Write-Osc 133 C
            [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
        } -ErrorAction SilentlyContinue
    } catch { }
}

# 启动时立即发送一次 CWD
Update-Cwd
Send-PrePrompt
