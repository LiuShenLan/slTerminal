# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目性质

**slTerminal** — 面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。

定位约束（贯穿全程，不可违背）：
- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）。

## 文档规范

所有非代码文档（CLAUDE.md、设计文档、测试清单等）遵循渐进式披露原则：

- **收录判定**：跨 ≥2 个模块适用、或每次会话必需的指令才入根文件；只在触碰某模块时才需要的细节，归该模块子路径的 `CLAUDE.md`
- **子文件创建**：新建模块目录时同步创建该路径的 `CLAUDE.md`，并登记下方模块索引
- **子文件模板**：职责 → 架构决策（关键约束）→ 文件表 → 测试模式（可选）
- 修改或新增代码时，同步更新所属子路径的 CLAUDE.md，不在根文件展开细节

## 架构（两进程模型）

**Rust 后端拥有一切 OS 访问；Web 前端只做 UI，经 IPC 调用后端。**

- 外壳 Tauri 2（系统 WebView2，不打包 Chromium）。
- 前端 React + TypeScript + Vite，状态用 Zustand。
- 终端 xterm.js（`@xterm/addon-webgl` + `addon-fit`）；编辑器 CodeMirror 6；布局 Dockview。
- 后端 portable-pty（Windows→ConPTY）、git2、notify。

目录结构原则：实现落到既定分层，不另起炉灶。
- 前端 `src/`：`ipc/`（唯一通信层）、`types/`、`stores/`、`workspace/`、`panels/`、`features/`、`theme/`、`lib/`。
- 后端 `src-tauri/src/`：`lib.rs`（注册命令/State/run）、`error.rs`、`state.rs` + 功能模块。

现行模块清单以下方「模块索引」为准。

## 硬性开发约束（新增功能必须遵守）

1. **前端绝不直接碰 OS/文件/进程**：`invoke` 只允许出现在 `src/ipc/`；其它文件只调用 `ipc/` 暴露的领域函数。
2. **后端按功能分模块**（现行清单见模块索引）：模块间不互相穿透，共享只经 `state.rs` 的 `AppState`。
3. **命令统一注册**于 `lib.rs` 的 `generate_handler!`；一律返回 `Result<_, AppError>`；阻塞 I/O 用 `spawn_blocking`。
4. **DTO 双边对应**：`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。
5. **面板封闭**：Dockview 面板只能是 `panels/` 下注册过的类型；新增类型 = 加目录 + 在 `panelRegistry.ts` 注册。
6. **配色单点**：所有颜色只在 `theme/colors.ts` 定义为 token；组件引用 token，禁止硬编码颜色（既定例外：xterm.js 终端主题在 `panels/terminal/theme.ts`）。
7. **布局单点**：操作页面布局只经 `workspace/layoutSerde.ts` 用 Dockview `toJSON/fromJSON` 存取。
8. **会话元数据单点**：PTY 进程映射仅在 `panels/terminal/TerminalRegistry`（模块级 Map）管理，前端会话元数据已合并。面板只订阅，不自存。
9. **平台分支收敛**：`#[cfg(windows)]` 只允许出现在 `pty/spawn.rs`、`pty/shell.rs` 等明确处，业务逻辑不撒 cfg。
10. **权限最小化**：Tauri 2 自定义命令默认放行，`capabilities/` 只管插件权限；不追加通配 `*`。

## Windows 关键坑

每条只留红线规则，机制与背景在链接的子文件。

- **spawn 串行化**：并发 spawn 会卡死 ConPTY 输出管道——`pty_spawn` 必须握 `SPAWN_LOCK`。详见 @../src-tauri/src/pty/CLAUDE.md
- **ConPTY flags 固定 0x7**：PASSTHROUGH_MODE (0x8) 吞全屏 TUI 鼠标滚轮输入；自动化测试无法守卫（假阴性），改 flags 必须实测真实 claude 滚轮。详见 @../src-tauri/src/pty/CLAUDE.md
- **cwd 反斜杠**：传给 ConPTY 前把 cwd 规范化成 `\`（`CreateProcessW` 对 `/` 行为异常）。详见 @../src-tauri/src/pty/CLAUDE.md
- **cwd / 命令边界跟踪**：portable-pty 在 Windows 不返回 cwd——注入 PowerShell 集成脚本发 OSC 7 + OSC 133，宿主据此跟踪，不解析提示符。详见 @../src-tauri/src/pty/CLAUDE.md
- **键盘 / IME**：Shift+Tab、Ctrl 组合键用 xterm.js `attachCustomKeyEventHandler` 接管；中文 IME 合成要尽早实测。详见 @../src/panels/CLAUDE.md
- **E2E 用不了 Playwright**（Tauri 非 Chromium）：用 embedded driver（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver`），零 msedgedriver 依赖。详见 @../e2e-tests/CLAUDE.md
- **watcher 不频繁重建**：`notify` 递归注册大目录（如 `target/`）耗时约 2s——用 `LruWatcherPool` 缓存 + pause/resume 切换，禁止 stop/start 轮换。详见 @../src-tauri/src/notify/CLAUDE.md
- **HTML 预览 iframe sandbox**：sandboxed iframe 中 `#fragment`/`:target` 彻底失效，`allow-same-origin` 会致 Tauri 向 iframe 注入 App JS——固定 `sandbox="allow-scripts"` + 注入脚本拦截锚点点击。详见 @../src/panels/CLAUDE.md
- **测试 tempdir 8.3 短名**：CI runner 的 `%TEMP%` 是 8.3 短名——Rust 测试路径比较前用 `dunce::canonicalize` 统一长名（`dunce::simplified` 不解短名），否则 CI 失败而本地不复现。详见 @../src-tauri/src/git/CLAUDE.md

## 命令

- 开发运行：`npm run tauri dev`
- 构建：`npx tauri build --debug --no-bundle`

## 测试策略

四级测试金字塔，按执行速度和隔离度分层。完整用例清单 → `@.claude/test-inventory.md`。

| 层级 | 名称 | 技术栈 | 运行命令 | 用例数 |
|------|------|--------|----------|--------|
| L1 | Rust 单元/集成 | `cargo test`、`tempfile` 隔离 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | 见 test-inventory |
| L2 | 前端单元/集成 | Vitest + jsdom | `npm test` | 见 test-inventory |
| L3 | 终端 headless 渲染 | Vitest + `@xterm/headless` | `npm run test:l3` | 见 test-inventory |
| L4 | 端到端 (E2E) | WDIO + embedded driver | `npm run e2e`（= `build:e2e` + `wdio`） | 见 test-inventory |

核心原则：
- **隔离优先**：L1 用 `tempfile::tempdir()` 隔离文件系统、`SPAWN_LOCK` 串行化 PTY；L2 用 `vi.mock()` 隔离 IPC/终端库；L4 用 embedded driver 隔离浏览器依赖
- **L1/L2 覆盖所有 PR**，L3/L4 覆盖关键路径变更
- **bugfix 须附防复发测试**：修复缺陷时同步提交常驻回归用例，防同一缺陷重现
- **用例清单同步**：新增/修改/删除用例须同步更新 `.claude/test-inventory.md`
- **L1 必须 `--test-threads=1`**：ConPTY 并发 spawn 会死锁
- **L4 必须 `VITE_E2E=1` 构建**（用 `npm run e2e`/`build:e2e`）：E2E helper 由 `E2E_ENABLED` 门控，`tauri build` 前端恒为 production `vite build`（`DEV=false`），不设开关则 helper 被 tree-shake、wdio 全部卡"Workspace 未就绪"
- **模块测试模式见各子路径 CLAUDE.md**，不在根文件展开

静态检查门禁：
- TypeScript：`npx tsc --noEmit`
- ESLint：`npx eslint src/`
- Clippy：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- rustfmt：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

### 发布打包

**一键打包**：`.\.claude\package.ps1 -Version "0.1.0"`（release 模式，单文件 exe → zip）
加 `-Debug` 用 debug 模式（exe + dll 两个文件）。

手动步骤：
1. `npx tauri build --no-bundle` → `src-tauri/target/release/slterminal.exe`（单文件自包含）
2. `Compress-Archive src-tauri/target/release/slterminal.exe slterminal-v0.1.0-x64.zip`
3. GitHub Releases → 创建 Tag `v0.1.0` → 上传 zip

> **给他人分享**：zip 解压到任意目录，双击 `slterminal.exe` 即可运行。不写注册表、不写 C 盘。
> 首次运行 Windows SmartScreen 会提示"Windows protected your PC"→ 点"更多信息"→"仍要运行"。

## 模块索引

登记规则：增删模块时同步增删行；模块新建子路径 CLAUDE.md 后在「详情」列补链接。

| 模块 | 职责 | 入口 | 详情 |
|------|------|------|------|
| src/ipc | IPC 通信层，前端 invoke 唯一入口 | src/ipc/index.ts | @../src/ipc/CLAUDE.md |
| src/types | DTO 类型定义，与 Rust 模块双边对应（硬约束 #4） | src/types/ | — |
| src/stores | Zustand 状态管理（projects/layout/fontSize/keybindings/sideBar） | src/stores/index.ts | @../src/stores/CLAUDE.md |
| src/workspace | 工作区布局管理（Dockview serde + 面板注册 + titleManager + pageApis） | src/workspace/Workspace.tsx | @../src/workspace/CLAUDE.md |
| src/panels | Dockview 面板系统（terminal + editor + html + gitshow + diff + hooksConfig） | src/panels/index.ts | @../src/panels/CLAUDE.md |
| src/lib | 通用工具 + createActivePointer + useFontSizeWheel + ErrorBoundary + E2E_ENABLED 门控 + 路径函数 | src/lib/index.ts | @../src/lib/CLAUDE.md |
| src/theme | 配色 token 单点（硬约束 #6） | src/theme/colors.ts | — |
| src/features/explorer | 文件浏览器（FileTree + 选中模型 + 键盘快捷键 + useFileTree + FileViewerRegistry 分派） | src/features/explorer/ExplorerPanel.tsx | @../src/features/explorer/CLAUDE.md |
| src/features/fileViewers | 文件查看器注册表（策略模式，扩展名→面板类型映射） | src/features/fileViewers/index.ts | @../src/features/fileViewers/CLAUDE.md |
| src/features/shortcuts | 快捷键模块（ShortcutRegistry 单例 + usePanelFocus + Command/Keybinding 分离 + 用户重绑定） | src/features/shortcuts/index.ts | @../src/features/shortcuts/CLAUDE.md |
| src/features/sidebar | 侧栏项目/页面二级树（项目/页面 CRUD + 页面切换导航） | src/features/sidebar/index.ts | @../src/features/sidebar/CLAUDE.md |
| src/features/sideViews | 侧栏视图系统——活动栏+共享侧栏区+单槽位状态机 | src/features/sideViews/index.ts | @../src/features/sideViews/CLAUDE.md |
| src/features/commit | Commit 侧栏视图（git 变更列表 + 状态→面板分派） | src/features/commit/index.ts | @../src/features/commit/CLAUDE.md |
| src/features/agentStatus | Agent 状态视图（claudeSession 行建模 + 上下文用量） | src/features/agentStatus/index.ts | — |
| src/features/notifications | toast 通知（Tauri 原生 sendNotification + 任务栏闪烁） | src/features/notifications/index.ts | — |
| src/features/hooksConfig | hooks 配置面板 schema 内嵌单点（SchemaStore 官方 schema + hooks 子 schema + Draft07 校验） | src/features/hooksConfig/schema/index.ts | — |
| src/__tests__ | L2 前端测试集中目录 + 共享测试工厂 | — | @../src/__tests__/CLAUDE.md |
| src-tauri/src/pty | PTY 管理，Windows ConPTY 核心 | src-tauri/src/pty/mod.rs | @../src-tauri/src/pty/CLAUDE.md |
| src-tauri/src/fs | 文件系统命令（读/写/列目录/建/删/改名） | src-tauri/src/fs/mod.rs | @../src-tauri/src/fs/CLAUDE.md |
| src-tauri/src/git | Git 状态/diff/HEAD 读取/回滚/取消暂存（git2） | src-tauri/src/git/mod.rs | @../src-tauri/src/git/CLAUDE.md |
| src-tauri/src/notify | 文件系统监听（LruWatcherPool 缓存 + pause/resume 切换） | src-tauri/src/notify/mod.rs | @../src-tauri/src/notify/CLAUDE.md |
| src-tauri/src/hooks | Claude Code hooks 注入/卸载/状态 + hook-event 广播 + 上下文用量 + hooks 配置三层读写（hooks_config_read/write） | src-tauri/src/hooks/mod.rs | @../src-tauri/src/hooks/CLAUDE.md |
| src-tauri settings/projects | 顶层单文件模块：settings.rs（设置持久化浅合并）、projects.rs（项目数据，exe 同级 JSON 绕过沙箱） | src-tauri/src/settings.rs | — |
| e2e-tests | WDIO E2E 端到端测试 | e2e-tests/wdio.conf.ts | @../e2e-tests/CLAUDE.md |

## 需求编号索引

代码和文档中引用的短标识符规则：

**前缀语义**：

| 前缀 | 含义 | 示例 |
|------|------|------|
| H | 需求（早期高层需求清单） | H6 |
| E | 工程机制需求 | E1 |
| P | 问题（阶段-序号） | P1-19 |
| F | 特性 | F3、F5 |
| SEC | 安全约束 | SEC-01 |
| DBG | 调试调查 | DBG-5 |
| B | 缺陷 | B10 |
| FIX | 修复项 | FIX-TE-04 |
| ADR | 架构决策记录 | ADR-0001 |
| L | 测试层级——免登记，定义见「测试策略」 | L1–L4 |
| R | 回归变体——免登记，模块内就近定义 | R2–R4 |

**登记规则**：跨模块引用的标识符首次使用时登记到下表；仅模块内部使用的就近定义，不登记。

| 标识符 | 类型 | 含义 |
|--------|------|------|
| H6 | 需求 | 终端跨页面存活——页面切换不杀 PTY 进程 |
| E1 | 需求 | Channel 可替换 + ring buffer 回放——PTY 重连机制 |
| P1-19 | 问题 | 窗口关闭前杀子进程——Tauri on_window_event 清理 PTY |
| P2-49 | 问题 | dockview-react dispose 内部自动清理——无需手动清 |
| SEC-01 | 安全 | project_root 是页面切换前置条件（路径沙箱） |
| SEC-03 | 安全 | HTML postMessage origin/source/信任标记三层校验 |
| SEC-08 | 安全 | PTY write/resize/kill 的 panelId 归属校验 |
| B10 | 缺陷 | 编辑器去重聚焦须匹配 suffix（普通编辑器与 git 页签互不误聚焦） |
| ADR-0001 | 架构决策 | 侧栏视图换区重建丢失组件内部状态（已确认接受） |
| F3 | 特性 | 终端页签四态 emoji 指示（hook-event + OSC 133 合成） |
| F5 | 特性 | claudeSession 契约行建模（双通道建行/三通道删行） |
| F6 | 特性 | hooks 双模式配置面板（JSON/GUI 编辑 hooks 子树，user/project/local 三层，F2 注入入口并入） |

> 测试策略概览见上方「测试策略」章节；完整用例清单见 `.claude/test-inventory.md`。
