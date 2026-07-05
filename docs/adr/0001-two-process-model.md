# 两进程模型（Rust 后端 + Web 前端）

Status: Accepted

slTerminal 采用 Rust 后端（拥有所有 OS 访问）与 Web 前端（仅 UI 渲染）的分离架构，通过 Tauri 2 IPC 通信。

**为什么选 Tauri 而非 Electron？** Electron 打包 Chromium（~150MB），Tauri 使用系统 WebView2（二进制 ~16MB）。Electron 的 Node.js 后端通过 `child_process` 管理 PTY，而 Rust 原生调用 Windows ConPTY API，能精确控制 Job Object、CPR 注入、`CreateProcessW` 等底层行为——这对稳定终端模拟器至关重要。Web 前端保留 xterm.js + CodeMirror 6 等成熟 Web 生态。

**备选方案**：WSL 模式（仅 Linux 用户可用）；纯 Rust GUI（egui/tauri-egui 缺乏 xterm.js 和 CodeMirror 级别的成熟控件）。

**后果**：所有 OS 调用必须经过 `src/ipc/` 层；前端和后端 DTO 必须双边维护；数据处理用 Rust（`snake_case`），展示用 JS（`camelCase`），serde 自动转换。
