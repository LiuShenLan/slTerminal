# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目性质

**slTerminal** — 面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。当前仓库**只有规划文档，尚无代码**：实现按 `plan/` 中的分阶段计划逐步落地。开工前先读对应阶段文档，不要凭空臆造结构。

定位约束（贯穿全程，不可违背）：
- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）。

## 规划文档（实现前必读）

| 文档 | 内容 |
|------|------|
| `plan/1. 需求与分阶段计划.md` | 需求规格 + M1–M6 里程碑 + JetBrains 暗色配色表 |
| `plan/2. 详细方案与最小demo.md` | 技术选型 + IPC 契约 + 最小 demo + Windows 关键坑 |
| `plan/3. 技术架构设计文档.md` | **架构 + 模块路径划分 + 硬性开发约束（第七节）** |
| `plan/4. 完整开发计划.md` | 完整开发方法学 |
| `plan/5.0–5.6 phase-*.md` | 每个 phase 的开发文档与验收标准（5.0 = 工程与测试基建） |

## 架构（两进程模型）

**Rust 后端拥有一切 OS 访问；Web 前端只做 UI，经 IPC 调用后端。**

- 外壳 Tauri 2（系统 WebView2，不打包 Chromium）。
- 前端 React + TypeScript + Vite，状态用 Zustand。
- 终端 xterm.js（`@xterm/addon-webgl` + `addon-fit`）；编辑器 CodeMirror 6；布局 Dockview。
- 后端 portable-pty（Windows→ConPTY）、git2、notify。

规划目录结构（实现时落到既定位置，不另起炉灶）：
- 前端 `src/`：`ipc/`（唯一通信层）、`types/`、`stores/`、`workspace/`、`panels/`（terminal/editor）、`features/`、`theme/`、`lib/`。
- 后端 `src-tauri/src/`：`lib.rs`（注册命令/State/run）、`error.rs`、`state.rs`，功能模块 `pty/ fs/ git/ claude/ notify/`。

## 硬性开发约束（来自架构文档第七节，新增功能必须遵守）

1. **前端绝不直接碰 OS/文件/进程**：`invoke` 只允许出现在 `src/ipc/`；其它文件只调用 `ipc/` 暴露的领域函数。
2. **后端按功能分模块**（pty/fs/git/claude/notify）：模块间不互相穿透，共享只经 `state.rs` 的 `AppState`。
3. **命令统一注册**于 `lib.rs` 的 `generate_handler!`；一律返回 `Result<_, AppError>`；阻塞 I/O 用 `spawn_blocking`。
4. **DTO 双边对应**：`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。
5. **面板封闭**：Dockview 面板只能是 `panels/` 下注册过的类型；新增类型 = 加目录 + 在 `panelRegistry.ts` 注册。
6. **配色单点**：JetBrains git 配色（文件名色、行内 diff 色）只在 `theme/colors.ts` 定义；组件引用 token，禁止硬编码颜色。
7. **布局单点**：操作页面布局只经 `workspace/layoutSerde.ts` 用 Dockview `toJSON/fromJSON` 存取。
8. **会话单点**：会话真值（id/活动状态/cwd）只在 `stores/sessions.ts`；面板只订阅，不自存。
9. **平台分支收敛**：`#[cfg(windows)]` 只允许出现在 `pty/spawn.rs`、`pty/shell.rs` 等明确处，业务逻辑不撒 cfg。
10. **权限最小化**：新增命令必须在 `capabilities/` 显式放行，不用通配 `*`。

## Windows 关键坑（PTY 实现时必踩）

- **spawn 串行化**：并发 spawn 会卡死 ConPTY 输出管道 → 用一把 `SPAWN_LOCK` 把 spawn 串起来（`pty/spawn.rs`）。
- **cwd 反斜杠**：传给 ConPTY 前把 cwd 规范化成 `\`（`CreateProcessW` 对 `/` 行为异常）。
- **cwd / 命令边界跟踪**：portable-pty 在 Windows 不返回 cwd → 注入 PowerShell profile 发 OSC 7（cwd）+ OSC 133 A/B/C/D（提示符边界 + 退出码），宿主据此跟踪，不解析提示符。
- **键盘 / IME**：Shift+Tab、Ctrl 组合键用 xterm.js `attachCustomKeyEventHandler` 接管；中文 IME 合成要尽早测。
- E2E 测试在 Tauri 用不了 Playwright（非 Chromium）。Phase 0 起用 embedded driver（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver` → `webview2-com` 驱动 ICoreWebView2 COM），零 msedgedriver 依赖。

## 命令（项目脚手架尚未创建，以下为规划值，见 phase 0）

- 后端测试：`cargo test`（在 `src-tauri/`）。
- 前端测试：`npm test`（Vitest + `@tauri-apps/api/mocks`）。
- 终端渲染测试（L3）：`@xterm/headless` + serialize 断言。
- 构建：`tauri build --debug`。
- 开发运行：`npm run tauri dev`。

> 脚手架由 `npm create tauri-app`（React+TS+Vite）初始化；CI 走 GitHub Actions `windows-latest`。实际命令以 phase 0 落地后的 `package.json` / `Cargo.toml` 为准。
