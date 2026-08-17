# vendor/conpty —— 捆绑新版 ConPTY 宿主（ADR-0005）

老 Win10（build < 21376）in-box conhost 不转发鼠标 VT 序列（microsoft/terminal#376，
修复 PR #4856 只在新版 conhost）——claude 等全屏 TUI 滚轮失效。本目录两文件为
新版 ConPTY 实现，运行时嵌入 slterminal_lib.dll 并由 `pty/conpty_api.rs` 提取到
`%LOCALAPPDATA%\slterminal\conpty\` 加载（仅 Win10 启用；失败静默回退系统 conhost）。

## 来源

- 包：NuGet `Microsoft.Windows.Console.ConPTY` **1.24.260710001**（microsoft/terminal 官方构建，MIT）
- 提取路径（包内）：
  - `runtimes/win-x64/native/conpty.dll`
  - `build/native/runtimes/x64/OpenConsole.exe`
- 导出名（dumpbin 验证）：`ConptyCreatePseudoConsole` / `ConptyClosePseudoConsole` / `ConptyResizePseudoConsole`（带 Conpty 前缀）
- OpenConsole.exe 依赖全为系统 api-ms-win-*（静态 CRT，自包含）

## 更新流程

1. 下载新版本包：`https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/<版本>`
2. 按上述提取路径替换本目录两文件
3. 验证导出名（dumpbin /exports）+ 依赖（dumpbin /dependents）
4. 更新本文件版本号；运行 L1 全量 + Win10 实机验收（滚轮 + 键盘/IME/kitty）
