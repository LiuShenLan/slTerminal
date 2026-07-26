# Phase 1 Stage 划分

> 共 6 个 Stage，每 Stage 3-12 项。文档同步固定为最后 Stage。
> 所有跨边界契约写死于本文件「契约速查」段与脚本头部注释。

---

## 契约速查（执行期不可各自推断）

| 项 | 值 |
|----|-----|
| 新增 IPC 命令 | `hooks_inject` / `hooks_uninstall` / `hooks_injection_status` |
| Tauri Event 名 | `hook-event` |
| 信号目录 | `~/.slterminal/hooks-events/`（`dirs::home_dir()`） |
| Hook 脚本 | `~/.slterminal/hooks/slterm-hook-reporter.js` |
| 用户配置路径 | `~/.claude/settings.json`（`dirs::home_dir()`） |
| Rust DTO | `HookInjectionStatus { status: InjectionStatus, version: Option<i32> }`；`InjectionStatus` 序列化为 camelCase 字符串 `"injected"|"notInjected"|"outdated"` |
| JS DTO | `HookInjectionStatus { status: "injected" | "notInjected" | "outdated", version: number | null }`；`HookEventPayload { panelId, event, timestamp, sessionId, transcriptPath, cwd, toolName, notificationType }` |
| 环境变量 | `SLTERM_PANEL_ID`（`pty_spawn` 注入，值 = `request.panel_id`） |
| 10 事件 | `SessionStart` `SessionEnd` `UserPromptSubmit` `Stop` `StopFailure` `PreToolUse` `PostToolUse` `PostToolUseFailure` `Notification` `PermissionRequest` |
| 四态 emoji | `working=⚡` / `attention=🟡` / `done=✅` / `error=❌` |
| 信号并发策略 | 单事件单文件 + 原子 rename（checklist 开放项已决策） |
| 脚本退出码 | 任何路径恒为 0，stderr 不得输出 |

---

## Stage 01：后端 hooks 模块骨架 + 信号 watcher

**内容**：新建 `src-tauri/src/hooks/` 模块（DTO、signal 解析、watcher、注入/卸载/状态命令）；注册命令；启动 watcher。

**项数**：9

**改动文件**：
- 新建 `src-tauri/src/hooks/mod.rs`
- 新建 `src-tauri/src/hooks/signal.rs`
- 新建 `src-tauri/src/hooks/watcher.rs`
- 新建 `src-tauri/src/hooks/inject.rs`
- 新建 `src-tauri/assets/slterm-hook-reporter.js`
- 修改 `src-tauri/src/lib.rs`（注册命令 + `.setup()` 启动 watcher）

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| be-hooks-dto | P1-BE-01/P1-BE-02 DTO + signal 解析纯函数 | `hooks/mod.rs`、`hooks/signal.rs` |
| be-hooks-watcher | P1-BE-03 watcher 生命周期 | `hooks/watcher.rs` |
| be-hooks-inject | P1-BE-04/P1-BE-05 注入/卸载/状态 + 脚本模板 | `hooks/inject.rs`、`assets/slterm-hook-reporter.js` |
| be-lib-register | P1-BE-06/P1-BE-07 lib.rs 注册与 setup | `src-tauri/src/lib.rs` |

**实现要点**：
- `HookEventPayload` 必须 `#[serde(rename_all = "camelCase")]`，与 JS DTO 字段逐字对应。
- `InjectionStatus` 序列化目标为 `"injected"`/`"notInjected"`/`"outdated"`；若 serde 默认外部标签不满足，改用 `#[serde(untagged)]` 或自定义 `Serialize`。
- watcher 使用 `notify_debouncer_full::new_debouncer`，debounce 50ms，`RecursiveMode::NonRecursive`，线程名 `hook-signal-watcher`。
- 单事件单文件：脚本写 `*.tmp` 再 `renameSync`；后端 Create 事件触发后读取 → 解析 → `emit("hook-event", payload)` → `remove_file`。
- 解析失败、缺 `panel_id`、读文件失败均只 `tracing::warn!`，不 panic，仍尝试删除文件。
- `hooks_inject` 对 `~/.claude/settings.json` 原子写（tempfile + persist）；JSON 非法时返回 `AppError` 并**不改动文件**。
- 脚本内 `SCRIPT_VERSION = 1` 常量；`hooks_injection_status` 读取磁盘脚本，正则匹配 `SCRIPT_VERSION\s*=\s*(\d+)` 与模板比对。
- 10 事件 matcher 组 command 为 `node "<脚本绝对路径>"`，`timeout: 5`；识别 slTerminal 段按 command 含 `slterm-hook-reporter` 子串。
- `lib.rs` 在 `.setup(|app| { hooks::start_signal_watcher(app.handle().clone()); Ok(()) })` 启动 watcher；watcher 用 `static Mutex<Option<HookSignalWatcher>>` 持有，避免 state.rs 循环依赖。

**验证项**：
1. `src-tauri/src/lib.rs` `generate_handler!` 含三命令名。
2. `src-tauri/src/lib.rs` `.setup()` 调用 `hooks::start_signal_watcher`。
3. `cargo check --manifest-path src-tauri/Cargo.toml` 通过。
4. `cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1` 通过（signal 解析 + inject 测试）。
5. 临时目录测试：注入后 `~/.claude/settings.json` 含 10 事件段；再次注入无重复；卸载后无 `slterm-hook-reporter` 子串。
6. 信号解析测试：合法 JSON 透传；缺 `panelId` 返回 None；非法 JSON 返回 None。
7. watcher 生命周期测试：start → stop → Drop 线程结束。

**人工验证点**：
- 启动 slTerminal 后，`~/.slterminal/hooks-events/` 目录在首次有信号时被创建。
- hook 脚本任何错误路径（无 panelId/目录不可写/JSON 非法）均 `exit 0` 且 stderr 为空。

**commit message**：`feat: 后端 hooks 模块（信号 watcher + 注入/卸载/状态命令）`

---

## Stage 02：PTY 注入 SLTERM_PANEL_ID

**内容**：在 `pty_spawn` 两条路径追加 `SLTERM_PANEL_ID` 环境变量；补 L1 测试。

**项数**：3

**改动文件**：
- 修改 `src-tauri/src/pty/spawn.rs`
- 修改 `src-tauri/tests/pty_integration_tests.rs`（或 `pty/spawn.rs` 单元测试）

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| be-pty-env | P1-PTY-01/P1-PTY-02 环境变量注入 | `src-tauri/src/pty/spawn.rs` |
| be-pty-test | P1-PTY-03 注入测试 | `src-tauri/tests/pty_integration_tests.rs` 或 `pty/spawn.rs` |

**实现要点**：
- Windows 路径：`extra_envs` Vec 追加 `("SLTERM_PANEL_ID", request.panel_id)`（`spawn.rs:790-794`）。
- 非 Windows fallback：`cmd.env("SLTERM_PANEL_ID", request.panel_id)`（`spawn.rs:854-856`）。
- 不加 shell 类型判断，与 COLORTERM/TERM/TERM_PROGRAM 一致。
- 测试需 `--test-threads=1`；可 spawn `cmd.exe` 后执行 `echo %SLTERM_PANEL_ID%` 读取验证，或检查 `PtySession` 创建后子进程环境（更轻量）。

**验证项**：
1. `grep -n "SLTERM_PANEL_ID" src-tauri/src/pty/spawn.rs` 命中两条路径。
2. `cargo test --manifest-path src-tauri/Cargo.toml pty_env -- --test-threads=1` 通过（或对应测试名）。
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过。

**人工验证点**：
- 真实 claude 页签启动后，在页签内执行 `echo $env:SLTERM_PANEL_ID`（pwsh）或 `echo %SLTERM_PANEL_ID%`（cmd），输出与面板 ID 一致。

**commit message**：`feat: pty_spawn 注入 SLTERM_PANEL_ID 环境变量`

---

## Stage 03：前端 IPC hooks + 四态映射单点

**内容**：新建 `src/ipc/hooks.ts`、barrel export、四态单点 `src/lib/claudeStatus.ts`；补 L2 契约测试。

**项数**：5

**改动文件**：
- 新建 `src/ipc/hooks.ts`
- 修改 `src/ipc/index.ts`
- 新建 `src/lib/claudeStatus.ts`
- 新建 `src/__tests__/ipc-hooks-contract.test.ts`
- 新建 `src/__tests__/claude-status.test.ts`

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| fe-ipc | P1-FE-01/P1-FE-02 IPC 封装 | `src/ipc/hooks.ts`、`src/ipc/index.ts` |
| fe-status | P1-FE-03 四态单点 | `src/lib/claudeStatus.ts` |
| fe-tests | P1-FE-04/P1-FE-05 L2 测试 | `src/__tests__/ipc-hooks-contract.test.ts`、`src/__tests__/claude-status.test.ts` |

**实现要点**：
- `src/ipc/hooks.ts`：`inject()`/`uninstall()`/`getInjectionStatus()` 对应 Rust 三命令；`onHookEvent(cb)` 用 `listen<HookEventPayload>("hook-event")`。
- DTO 字段 camelCase，与 Rust `rename_all = "camelCase"` 对应。
- `src/lib/claudeStatus.ts`：导出 `ClaudeStatus` 类型、`STATUS_EMOJI` 常量、`eventToStatus(event, notificationType?)` 纯函数；状态机完整表按 F3。
- 契约测试：mockIPC 验证命令名、参数结构、返回值；`onHookEvent` 验证 `listen` 订阅与 unsubscribe。
- 四态测试：10 事件 × notificationType 组合全表覆盖；断言返回值与 emoji 常量。

**验证项**：
1. `npx tsc --noEmit` 通过。
2. `npx eslint src/ipc/hooks.ts src/lib/claudeStatus.ts` 通过。
3. `npm test ipc-hooks-contract claude-status` 通过。
4. `src/ipc/index.ts` re-export 了 `hooks`。
5. `eventToStatus("Notification", "permission_prompt")` 返回 `"attention"`；`eventToStatus("Notification", "auth_success")` 返回 `null`。

**人工验证点**：无（纯前端单元）。

**commit message**：`feat: 前端 IPC hooks 层 + 四态状态机单点`

---

## Stage 04：页签四态集成

**内容**：`useCommandDetection` 改 attention 启动态；`useXterm` 订阅 hook-event；`DefaultTab` 渲染 emoji；`tabRules` 删图标；`TerminalPanel` 修正 title/icon 更新逻辑。

**项数**：6

**改动文件**：
- 修改 `src/panels/terminal/useCommandDetection.ts`
- 修改 `src/panels/terminal/useXterm.ts`
- 修改 `src/panels/terminal/TerminalPanel.tsx`
- 修改 `src/panels/terminal/tabRules.ts`
- 修改 `src/workspace/PageDockviewHost.tsx`
- 修改/新增 `src/__tests__` 相关测试

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| fe-cmd | P1-F3-01 useCommandDetection 四态触发 | `src/panels/terminal/useCommandDetection.ts` |
| fe-xterm | P1-F3-02 useXterm hook-event 订阅 | `src/panels/terminal/useXterm.ts` |
| fe-panel | P1-F3-03 TerminalPanel title/icon 逻辑 | `src/panels/terminal/TerminalPanel.tsx` |
| fe-tab | P1-F3-04/P1-F3-05 DefaultTab emoji + tabRules 删图标 | `src/workspace/PageDockviewHost.tsx`、`src/panels/terminal/tabRules.ts` |
| fe-tests | P1-F3-07 相关 L2 测试更新 | `src/__tests__/*` |

**实现要点**：
- `useCommandDetection`：OSC 133 C 匹配到规则时 `onTabStateChange({ active: true, title: rule.title, icon: "🟡" })`；D 时 `{ active: false }`。
- `useXterm`：`useEffect` 中 `onHookEvent` 订阅，过滤 `payload.panelId === panelId`；非 null 状态调 `onTabStateChange({ active: true, icon: STATUS_EMOJI[status] })`；`SessionEnd` 调 `{ active: false }`；卸载时 unsubscribe。
- `TerminalPanel.handleTabStateChange`：active=true 时，`state.title` 存在才 `setTitle`，`state.icon !== undefined` 才 `updateParameters`；active=false 时恢复原标题并清 icon。**不覆盖 `originalTitleRef`**。
- `DefaultTab`：检测 `tabIcon` 是否为 URL/路径（含 `/`、`\`、`http:`、`data:`），是则 `<img>`，否则 `<span>` 渲染 emoji。
- `tabRules.ts`：移除 `claudeLogo` import，注册项只保留 `command` + `title`。
- 测试：mock `onHookEvent` 验证 useXterm 中按 panelId 过滤、状态映射、SessionEnd 清 icon；`DefaultTab` 测试 emoji 与图片分支；`tabRules` 测试确认无 icon。

**验证项**：
1. `npx tsc --noEmit` 通过。
2. `npx eslint src/panels/terminal src/workspace/PageDockviewHost.tsx` 通过。
3. `npm test` 全量通过（含已有 terminal 测试不回归）。
4. `tabRules.ts` 不再 import `claudeLogo`。
5. `DefaultTab` 对 `tabIcon="⚡"` 渲染 span，对 `tabIcon="/assets/..."` 渲染 img。
6. `useXterm` 测试：hook-event panelId 不匹配时不更新；匹配时更新 emoji；SessionEnd 时清 icon。

**人工验证点**：
- 未注入 hooks 的 claude 页签：启动后页签出现 🟡，退出后消失。
- 注入 hooks 后：启动 🟡 → 提交 prompt ⚡ → 完成 ✅ → 退出无图标。

**commit message**：`feat: 页签四态指示（OSC 133 + hook-event 集成）`

---

## Stage 05：L4 E2E 关键路径

**内容**：E2E helper 扩展 + 1-2 条 L4 用例验证页签图标随 hook-event 流转。

**项数**：2

**改动文件**：
- 修改 `e2e-tests/helpers.ts`
- 修改 `e2e-tests/test.e2e.ts`

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| e2e-helpers | P1-TE-04 helper 扩展 | `e2e-tests/helpers.ts` |
| e2e-test | P1-TE-03 L4 用例 | `e2e-tests/test.e2e.ts` |

**实现要点**：
- helper 追加（`E2E_ENABLED` 门控）：
  - `__slterm_e2e_injectHooks()` → `hooks.inject()`
  - `__slterm_e2e_uninstallHooks()` → `hooks.uninstall()`
  - `__slterm_e2e_getHookInjectionStatus()` → `hooks.getInjectionStatus()`
- L4 用例 1：注入状态
  - `browser.execute(() => __slterm_e2e_injectHooks())`
  - `browser.execute(() => __slterm_e2e_getHookInjectionStatus())` 返回 `status === "injected"`
- L4 用例 2：图标流转
  - 创建项目 + 终端面板，获取 panelId
  - Node 端写信号文件到 `~/.slterminal/hooks-events/`，payload panelId 匹配、event=`UserPromptSubmit`
  - `browser.execute` 查询该页签 DOM 包含 ⚡
  - Node 端写 event=`SessionEnd` 信号
  - 查询页签不再包含 emoji
- E2E 构建必须用 `npm run build:e2e`（`VITE_E2E=1`）。

**验证项**：
1. `npm run build:e2e` 成功（二进制含 helper）。
2. `npm run wdio` 中新增用例通过。
3. 生产 dist 不含 helper（CI grep 或本地 `npx vite build` 后检查）。

**人工验证点**：
- 真实 claude 全状态机走查：启动 🟡 → prompt ⚡ → 权限 🟡 → 放行 ⚡ → 完成 ✅ → 新 prompt ⚡ → 退出无图标。

**commit message**：`test: L4 E2E 页签图标 hook-event 流转关键路径`

---

## Stage 06：文档同步

**内容**：按 `config.json` 的 `claudeMdFiles` + 新增模块 CLAUDE.md + test-inventory 更新。

**项数**：5

**改动文件**：
- 新建 `src-tauri/src/hooks/CLAUDE.md`
- 修改 `src/ipc/CLAUDE.md`
- 修改 `src/lib/CLAUDE.md`
- 修改 `src/panels/CLAUDE.md`
- 修改 `.claude/test-inventory.md`

**Agent 文件分工表**：

| label | 负责项 | 文件 |
|-------|--------|------|
| doc-hooks | P1-DOC-01 新建 hooks 模块 CLAUDE.md | `src-tauri/src/hooks/CLAUDE.md` |
| doc-sync | P1-DOC-02/P1-DOC-03/P1-DOC-04/P1-DOC-05 其余文档与用例清单 | `src/ipc/CLAUDE.md`、`src/lib/CLAUDE.md`、`src/panels/CLAUDE.md`、`.claude/test-inventory.md` |

**实现要点**：
- 新建 `src-tauri/src/hooks/CLAUDE.md`：模块职责、文件列表、信号目录、watcher 启动、三命令、注入配置段识别、版本检测、测试模式。
- `src/ipc/CLAUDE.md`：模块映射表追加 `hooks.ts` ↔ `src-tauri/src/hooks/`。
- `src/lib/CLAUDE.md`：文件表追加 `claudeStatus.ts`。
- `src/panels/CLAUDE.md`：更新 useCommandDetection（OSC 133 C→🟡）、useXterm（hook-event 订阅）、DefaultTab（emoji 渲染）、tabRules（图标移除）。
- `.claude/test-inventory.md`：新增 L1/L2/L4 用例数，更新总计。

**验证项**：
1. 所有修改的 CLAUDE.md 无 markdown 语法错误。
2. `.claude/test-inventory.md` 中新增文件与用例数正确。
3. `config.json` 的 `claudeMdFiles` 清单包含 `src-tauri/src/hooks/CLAUDE.md`（新增）。

**人工验证点**：无。

**commit message**：`docs: Phase 1 hooks 模块与四态指示文档同步`

---

## Stage 汇总表

| Stage | 内容 | 项数 | 并行 agent 数 |
|-------|------|------|--------------|
| 01 | 后端 hooks 模块骨架 + 信号 watcher | 9 | 4 |
| 02 | PTY 注入 SLTERM_PANEL_ID | 3 | 2 |
| 03 | 前端 IPC hooks + 四态单点 | 5 | 3 |
| 04 | 页签四态集成 | 6 | 5 |
| 05 | L4 E2E 关键路径 | 2 | 2 |
| 06 | 文档同步 | 5 | 2 |
