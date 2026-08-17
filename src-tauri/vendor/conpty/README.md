# 捆绑 ConPTY 宿主（ADR-0005）

老版 Windows 10 内置 conhost 在 `WIN32_INPUT_MODE (0x4)` 下不转发鼠标 VT 序列——claude 等全屏 TUI 滚轮失效。本目录捆绑新版 OpenConsole（微软开源 conhost）并排加载修复，不替换系统 conhost。

## 来源

- **NuGet 包**：`Microsoft.Windows.Console.ConPTY` `1.24.260710001`（作者 Microsoft，仓库 [microsoft/terminal](https://github.com/microsoft/terminal)，MIT 许可）
- **提取路径**：`runtimes/win-x64/native/conpty.dll` + `build/native/runtimes/x64/OpenConsole.exe`
- **下载地址**：`https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/1.24.260710001/microsoft.windows.console.conpty.1.24.260710001.nupkg`
- **许可**：MIT（见 `LICENSE`，Microsoft Corporation）；支持 Windows 10.0.17763+

## 使用

- 两个文件拷贝到 `slterminal.exe` 同目录即生效（运行时检测 `conpty.dll` 存在 → `LoadLibrary` 动态解析 `Conpty*` 导出；缺失 → 静默回退系统 kernel32）。
- 导出函数名带 `Conpty` 前缀（避免与 kernel32 import 冲突，见包内 `conpty.h`）：`ConptyCreatePseudoConsole` / `ConptyClosePseudoConsole` / `ConptyResizePseudoConsole`。

## 更新指引

1. `curl "https://azuresearch-usnc.nuget.org/query?q=Microsoft.Windows.Console.ConPTY"` 查最新版本号。
2. 下载新 nupkg → 解包 → 覆盖本目录两文件 → 更新本文件版本号。
3. 更新后必须 **Win10 实机验证**：claude 滚轮 + 键盘/IME/kitty（自动化无法守卫 ConPTY 交互，见 `src-tauri/src/pty/CLAUDE.md`）。
