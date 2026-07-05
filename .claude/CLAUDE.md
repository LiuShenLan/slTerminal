# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目性质

**slTerminal** — 面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。

定位约束（贯穿全程，不可违背）：
- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）。

## 架构（两进程模型）

**Rust 后端拥有一切 OS 访问；Web 前端只做 UI，经 IPC 调用后端。**

- 外壳 Tauri 2（系统 WebView2，不打包 Chromium）。
- 前端 React + TypeScript + Vite，状态用 Zustand。
- 终端 xterm.js（`@xterm/addon-webgl` + `addon-fit`）；编辑器 CodeMirror 6；布局 Dockview。
- 后端 portable-pty（Windows→ConPTY）、git2、notify。

规划目录结构（实现时落到既定位置，不另起炉灶）：
- 前端 `src/`：`ipc/`（唯一通信层）、`types/`、`stores/`、`workspace/`（布局 + titleManager）、`panels/`（terminal/editor）、`features/`、`theme/`、`lib/`。
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
- **watcher 重建开销**：`notify` 在 Windows 上调用 `ReadDirectoryChangesW` 递归注册目录树，大目录（如 `target/` 26K 文件）耗时约 2s。不要频繁 `stop()` + `start()` watcher——用 `LruWatcherPool`（`notify/pool.rs`）缓存 + pause/resume 切换。详见 @../src-tauri/src/notify/CLAUDE.md

## 命令

- 后端测试：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- 前端测试：`npm test`（Vitest + `@tauri-apps/api/mocks`）
- 终端渲染测试（L3）：`npm run test:l3`
- 静态检查：`npx tsc --noEmit` + `npx eslint src/` + `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- 开发运行：`npm run tauri dev`
- 构建：`npx tauri build --debug --no-bundle`

> 全量自动化测试用例清单（727 用例：Rust 125 + 前端 602）→ @.claude/test-inventory.md

### 发布打包

**一键打包**：`.\.claude\package.ps1 -Version "0.1.0"`（release 模式，单文件 15.9MB exe → 5.7MB zip）
加 `-Debug` 用 debug 模式（exe + dll 两个文件，~20MB）。

手动步骤：
1. `npx tauri build --no-bundle` → `src-tauri/target/release/slterminal.exe`（单文件自包含）
2. `Compress-Archive src-tauri/target/release/slterminal.exe slterminal-v0.1.0-x64.zip`
3. GitHub Releases → 创建 Tag `v0.1.0` → 上传 zip

> **给他人分享**：zip 解压到任意目录，双击 `slterminal.exe` 即可运行。不写注册表、不写 C 盘。
> 首次运行 Windows SmartScreen 会提示"Windows protected your PC"→ 点"更多信息"→"仍要运行"。

## 模块

### 简单模块

| 模块 | 职责 | 入口 | 详情 |
|------|------|------|------|
| src/ipc | IPC 通信层，前端 invoke 唯一入口 | src/ipc/index.ts | @../src/ipc/CLAUDE.md |
| src/panels | Dockview 面板系统（terminal + editor） | src/panels/index.ts | @../src/panels/CLAUDE.md |
| src/stores | Zustand 状态管理，会话单点 | src/stores/index.ts | @../src/stores/CLAUDE.md |
| src/workspace | 工作区布局管理（Dockview serde + 面板注册 + titleManager） | src/workspace/Workspace.tsx | @../src/workspace/CLAUDE.md |
| src/lib | 通用工具（路径函数 `basename`/`isChildOf`/`relativePath`） | src/lib/index.ts | @../src/lib/CLAUDE.md |
| src/features/explorer | 文件浏览器（FileTree + useFileTree gen 取消） | src/features/explorer/ExplorerPanel.tsx | @../src/features/explorer/CLAUDE.md |
| src-tauri/src/pty | PTY 管理，Windows ConPTY 核心 | src-tauri/src/pty/mod.rs | @../src-tauri/src/pty/CLAUDE.md |
| src-tauri/src/notify | 文件系统监听（LruWatcherPool 缓存 + pause/resume 切换） | src-tauri/src/notify/mod.rs | @../src-tauri/src/notify/CLAUDE.md |
| e2e-tests | WDIO E2E 端到端测试 | e2e-tests/wdio.conf.ts | @../e2e-tests/CLAUDE.md |
