# vendor/conpty —— 捆绑新版 ConPTY 宿主

老 Win10（build < 21376）in-box conhost 的 ConPTY 不转发鼠标 VT 序列
（microsoft/terminal#376，修复 PR #4856 只在新版 conhost）——全屏 TUI 滚轮失效。
本目录两文件为新版 ConPTY 实现，经 `include_bytes!` 嵌入 slterm_terminal，
运行时仅 Win10 提取到 `%LOCALAPPDATA%\slterm\conpty\` 并动态加载
（提取/加载失败静默回退系统 conhost，行为 = 现状）。
conpty.dll 定位 OpenConsole.exe 靠同目录查找（PR #12980），故两文件必须同目录。

## 来源

- 包：NuGet `Microsoft.Windows.Console.ConPTY` **1.24.260710001**（microsoft/terminal 官方构建，MIT）
- 提取路径（包内）：
  - `runtimes/win-x64/native/conpty.dll`
  - `build/native/runtimes/x64/OpenConsole.exe`
- 导出名（PE 导出表实测）：`CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole`
  （同表另导出 `Conpty*` 前缀别名族，指向同一实现地址）
- OpenConsole.exe 依赖全为系统 api-ms-win-*（静态 CRT，自包含）
- MD5：conpty.dll = cbd0fe7c9db8514edbf32ad70e05529a，OpenConsole.exe = 5d725f6986d630b96f880fd72db57dc5

## 更新流程

1. 下载新版本包：`https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/<版本>`
2. 按上述提取路径替换本目录两文件
3. 验证导出名（PE 导出表）+ 依赖 + MD5 记录更新
4. 更新本文件版本号；运行 L1 全量 + Win10 实机验收（滚轮 + 键盘/IME/kitty）
