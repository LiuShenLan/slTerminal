#Requires -Version 7
# Win32 输入矩阵回放测试(11 篇「win32 输入基线矩阵」夹具;13 篇改造节 4 承接)。
#
# 被测对象 = slterm keymap 三路径编码器端到端(13 篇锚点):PostMessage 逐键驱动
# 最小化 slterm.exe(不抢焦点——用户在别处打字时也可安全运行)→ GPUI KeyDownEvent
# → on_key_down 分发 → keymap 编码 → PTY → 侧载 OpenConsole → 探针进程
# (node reader,即 Claude Code 的输入路径)stdin 字节 → hex 拼接 → 与入库基线
# 逐字节比对。
#
#   pwsh -File scripts\win32_input_matrix.ps1           # 断言模式(常挂比对)
#   pwsh -File scripts\win32_input_matrix.ps1 -Record   # (重)建基线
#   pwsh -File scripts\win32_input_matrix.ps1 -Exe <path-to-slterm.exe>
#
# LIMITS:PostMessage 不改变真实键盘状态,修饰键 CHORD(Shift+Enter、
# Ctrl+Space……)无法无人值守测试——布局层会读到真实的(未按下)修饰键状态。
# chord 覆盖需前台 SendInput + 值守空闲机,保持人工步骤(test-exemptions 实机
# 验收点)。
#
# 基线治理(11 篇状态机):基线文件 git 追踪;语义变更时「代码 + 基线」同提交,
# 禁单独提交基线(防静默放宽);任何数值变更须在提交信息说明触发原因。
param(
    [switch]$Record,
    [string]$Exe = ''
)
$ErrorActionPreference = 'Continue'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class PM {
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, UIntPtr w, IntPtr l);
}
'@

# 仓库根 = 脚本所在目录的上一级;exe 默认 debug 构建产物(13 篇:M3 起首个
# 可驱动目标)。
$root = Split-Path -Parent $PSScriptRoot
if ($Exe -eq '') { $Exe = Join-Path $root 'target\debug\slterm.exe' }
# 运行期 log 落 .tmp/(gitignore 已覆盖,勿入库)。
$tmpDir = Join-Path $root '.tmp'
New-Item -ItemType Directory -Force $tmpDir | Out-Null
$log = Join-Path $tmpDir 'win32_input_matrix.log'
$baselineFile = Join-Path $PSScriptRoot 'win32_input_matrix.baseline.txt'
Remove-Item $log -ErrorAction SilentlyContinue

# 探针经 cmd /c 转接:SEC-01 白名单(core 唯一进程创建口)只放行三家 shell,
# node 不能作 -e 直跑程序;cmd /c 下 node 继承同一 console 输入句柄,
# stdin 字节路径与直跑等价。程序名必须写全 cmd.exe:白名单校验对非名单
# 文件名按字面名做 PATH 解析(不加 .exe),裸名 cmd 解析不出会被拒。
$p = Start-Process $Exe -WindowStyle Minimized -ArgumentList @(
    '--working-directory', $root,
    '-e','cmd.exe','/c','node', (Join-Path $PSScriptRoot 'win32_input_matrix_probe.cjs'), $log
) -PassThru
$ready = $false
# 负载裕度,非断言放宽:30x1s = GPUI 首帧 + ConPTY 侧载握手 + node 冷启动的
# 真机上界;探针不 ready 即链路断裂,与耗时无关。
for ($i = 0; $i -lt 30; $i++) {
    Start-Sleep -Seconds 1
    if ((Test-Path $log) -and (Get-Content $log -Raw) -match 'ready') { $ready = $true; break }
}
if (-not $ready) { Write-Output 'FAIL: probe never ready'; if (-not $p.HasExited) { $p.Kill() }; exit 1 }
$p.Refresh()
$h = $p.MainWindowHandle
Start-Sleep -Seconds 1

$WM_KEYDOWN = 0x100; $WM_KEYUP = 0x101
function Send-Key([IntPtr]$hwnd, [int]$vk, [int]$scan, [bool]$ext = $false) {
    $extBit = if ($ext) { [int64]1 -shl 24 } else { 0 }
    $down = [IntPtr]([int64]($scan -shl 16) -bor 1 -bor $extBit)
    $up   = [IntPtr]([int64]0xC0000000 + ($scan -shl 16) + 1 + $extBit)
    [void][PM]::PostMessageW($hwnd, $WM_KEYDOWN, [UIntPtr][uint32]$vk, $down)
    # 负载裕度,非断言放宽:60ms/240ms 让 GPUI 消息循环与 PTY 泵消化逐键,
    # 字节内容断言不因此改变——链路断裂时该红仍红。
    Start-Sleep -Milliseconds 60
    [void][PM]::PostMessageW($hwnd, $WM_KEYUP, [UIntPtr][uint32]$vk, $up)
    Start-Sleep -Milliseconds 240
}

# 矩阵:名称, vk, scan, extended。仅无修饰键(chord 见 LIMITS)。
$matrix = @(
    @('a',         0x41, 0x1E, $false),
    @('Esc',       0x1B, 0x01, $false),
    @('Enter',     0x0D, 0x1C, $false),
    @('Tab',       0x09, 0x0F, $false),
    @('Backspace', 0x08, 0x0E, $false),
    @('Up',        0x26, 0x48, $true),
    @('Down',      0x28, 0x50, $true),
    @('Home',      0x24, 0x47, $true),
    @('Delete',    0x2E, 0x53, $true),
    @('F1',        0x70, 0x3B, $false),
    @('F5',        0x74, 0x3F, $false),
    @('ShiftTap',  0x10, 0x2A, $false),   # 裸修饰键:必须零字节
    @('b',         0x42, 0x30, $false)    # 静默修饰键后的哨兵键
)
foreach ($k in $matrix) { Send-Key $h $k[1] $k[2] $k[3] }
Send-Key $h 0x51 0x10   # q -> 探针退出
Start-Sleep -Seconds 2
if (-not $p.HasExited) { $p.Kill() }

# 顺序拼接所有 rx 块为一条 hex 串。
$rx = (Select-String -Path $log -Pattern 'rx ([0-9a-f]+)' -AllMatches).Matches |
    ForEach-Object { $_.Groups[1].Value }
$sequence = ($rx -join '')

if ($Record) {
    Set-Content -Path $baselineFile -Value $sequence
    Write-Output "RECORDED baseline: $sequence"
    exit 0
}
if (-not (Test-Path $baselineFile)) {
    Write-Output "FAIL: no baseline (run with -Record first). Got: $sequence"
    exit 1
}
$baseline = (Get-Content $baselineFile -Raw).Trim()
if ($sequence -eq $baseline) {
    Write-Output "PASS ($($rx.Count) chunks): $sequence"
    exit 0
} else {
    Write-Output "FAIL"
    Write-Output "  expected: $baseline"
    Write-Output "  got:      $sequence"
    exit 1
}
