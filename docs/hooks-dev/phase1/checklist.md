# Phase 1 清单：Hooks 宿主侧增强 · 状态可视化核心

> 范围：F1 信号文件通道 + F2 一键注入/卸载 + F3 页签四态指示的最小闭环。
> 真值源：`docs/hooks-dev/contract.md` + `docs/hooks-dev/feature-plan/phase1-status-core.md` + `ADR-0001-signal-file-channel.md`。

---

## 开放项决策：信号并发策略

| 备选 | 方案 | 评估 |
|------|------|------|
| A | 单事件单文件，hook 脚本先写 `.tmp` 再 `renameSync` 成 `.json` | 原子性强，无跨进程追加损坏风险；后端按 Create 事件读取后即可删除，生命周期清晰 |
| B | JSONL 追加，同 panelId 的事件写到同一个 `.jsonl` | 文件数少，但 Windows 上多 hook 进程并发 `appendFile` 无法保证行级原子；需额外加锁/合并逻辑 |

**决策：采用备选 A（单事件单文件 + 原子 rename）**。

理由：
1. 契约 C2 只要求"同页签高频事件写入不得相互损坏"，并未限制文件数；PreToolUse/PostToolUse 每秒数次，现代 SSD/NTFS 完全可承受。
2. 后端 `notify` watcher 监听的是固定 home 目录，Create 事件触发即代表文件完整落盘（由 rename 保证），天然避免半写文件。
3. 删除已处理文件可防止目录无限膨胀；即使后端偶尔遗漏，目录体积也受事件频率 × 单次大小（约 200B）限制。
4. JSONL 追加在 Windows 上需引入文件锁或合并线程，增加 Phase 1 复杂度且无显著收益。

---

## P1-BE 后端 hooks 模块

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-BE-01 | 新建 `src-tauri/src/hooks/mod.rs` | 模块入口：暴露 DTO、`start_signal_watcher`、三命令入口；管理 `static WATCHER` 或 `AppState` 字段（推荐静态 `Mutex<Option<HookSignalWatcher>>`，避免 state.rs 与 hooks 循环依赖） | P0 |
| P1-BE-02 | 新建 `src-tauri/src/hooks/signal.rs` | 信号解析纯函数：`parse_signal_file(content: &str) -> Option<HookEventPayload>`；DTO `HookEventPayload`（8 字段，camelCase）；单文件处理流程：读 → 解析 → `emit("hook-event")` → 删文件 | P0 |
| P1-BE-03 | 新建 `src-tauri/src/hooks/watcher.rs` | 目录监听器 `HookSignalWatcher`：复用 `notify` + `notify-debouncer-full`，debounce 50ms，监听 `~/.slterminal/hooks-events/`（`dirs::home_dir()`），NonRecursive；线程名 `hook-signal-watcher`；带 `stop()` + `Drop` 清理 | P0 |
| P1-BE-04 | 新建 `src-tauri/src/hooks/inject.rs` | 注入/卸载/状态三命令实现：脚本落盘（`~/.slterminal/hooks/slterm-hook-reporter.js`）、settings.json merge/移除、版本比对（脚本内 `SCRIPT_VERSION` 常量 vs 内嵌模板）、非法 JSON 中止 | P0 |
| P1-BE-05 | 新建 `src-tauri/assets/slterm-hook-reporter.js` | Node 单文件脚本：读 stdin → 解析 → 按 C1 组装 JSON → 原子写信号文件；任何异常静默 `process.exit(0)`；无 `SLTERM_PANEL_ID` 直接退出 | P0 |
| P1-BE-06 | `src-tauri/src/lib.rs:66-91` | `generate_handler!` 注册 `hooks_inject`/`hooks_uninstall`/`hooks_injection_status`；`.setup()` 中启动 `hooks::start_signal_watcher` | P0 |
| P1-BE-07 | `src-tauri/src/lib.rs setup` | 启动 watcher 时传入 `app.handle()`，目录不存在则自动创建 | P0 |
| P1-BE-08 | `src-tauri/src/hooks/mod.rs` `#[cfg(test)]` | 单元测试：DTO serde（camelCase 字段名）、`parse_signal_file` 全分支（合法/缺 panelId/非法 JSON/空串）、watcher 生命周期（start/stop/Drop） | P1 |
| P1-BE-09 | `src-tauri/src/hooks/inject.rs` `#[cfg(test)]` | 单元测试：注入幂等（空 settings/已有用户 hooks/已注入升级）、卸载干净、状态检测（injected/outdated/notInjected）、非法 JSON 中止 | P1 |

---

## P1-PTY PTY 环境变量注入

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-PTY-01 | `src-tauri/src/pty/spawn.rs:790-794` | `extra_envs` Vec 追加第 4 项 `("SLTERM_PANEL_ID", request.panel_id)`，与 COLORTERM/TERM/TERM_PROGRAM 同一时机 | P0 |
| P1-PTY-02 | `src-tauri/src/pty/spawn.rs:854-856` | 非 Windows fallback 的 `cmd.env()` 同步追加 `SLTERM_PANEL_ID` | P0 |
| P1-PTY-03 | `src-tauri/src/pty/spawn.rs` `#[cfg(test)]` | 单元/集成测试：spawn 后验证子进程环境含 `SLTERM_PANEL_ID` 且值等于 `request.panel_id`（可借 `pty_integration_tests` 或新增独立测试） | P1 |

---

## P1-FE 前端 IPC 与四态单点

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-FE-01 | 新建 `src/ipc/hooks.ts` | `inject()`/`uninstall()`/`getInjectionStatus()` 封装 C6 三命令；`onHookEvent(cb)` 封装 `listen<HookEventPayload>("hook-event")`，返回 unsubscribe | P0 |
| P1-FE-02 | `src/ipc/index.ts` | barrel export：`export * as hooks from "./hooks"` | P0 |
| P1-FE-03 | 新建 `src/lib/claudeStatus.ts` | 四态类型 `ClaudeStatus` + `STATUS_EMOJI` 常量 + `eventToStatus(event, notificationType?)` 纯函数；状态机完整表按 F3（含 Notification 三类→attention、其他→null） | P0 |
| P1-FE-04 | `src/__tests__/ipc-hooks-contract.test.ts` | 四维验证：命令名/参数结构/返回值/异常传播；`onHookEvent` Channel 绑定 | P1 |
| P1-FE-05 | `src/__tests__/claude-status.test.ts` | 纯函数映射表全分支覆盖（10 事件 × notificationType 组合 + OSC 133 C 语义注释说明） | P1 |

---

## P1-F3 页签四态集成

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-F3-01 | `src/panels/terminal/useCommandDetection.ts` | OSC 133 C 触发时调用 `onTabStateChange({ active: true, title, icon: "🟡" })`；OSC 133 D 触发时 `isCommandRunningRef.current = false` 并调用 `{ active: false }`；标题切换保留 | P0 |
| P1-F3-02 | `src/panels/terminal/useXterm.ts` | 新增 `onHookEvent` 订阅：按 `panelId` 过滤事件 → `eventToStatus` → 调用 `onTabStateChange({ active: true, icon: emoji })`；`SessionEnd` 调 `{ active: false }` | P0 |
| P1-F3-03 | `src/panels/terminal/TerminalPanel.tsx:67-77` | `handleTabStateChange` 修正：`active=true` 时只有 `state.title` 存在才 `setTitle`，只有 `state.icon !== undefined` 才 `updateParameters`；`active=false` 时恢复原标题并清 icon；不覆盖 `originalTitleRef` | P0 |
| P1-F3-04 | `src/workspace/PageDockviewHost.tsx:181-223` | `DefaultTab` 支持 emoji `tabIcon`：URL/路径型仍走 `<img>`，非 URL 字符串走 `<span>` 渲染 emoji | P0 |
| P1-F3-05 | `src/panels/terminal/tabRules.ts` | 删除 claude 规则的 `icon` 字段（移除 `claudeLogo` import 与注册项的 icon），保留 `command` + `title` | P0 |
| P1-F3-06 | `src/panels/terminal/useXterm.ts` | E2E helper 扩展：可选增加 `__slterm_e2e_injectHooks`/`__slterm_e2e_uninstallHooks`/`__slterm_e2e_getHookInjectionStatus`（`E2E_ENABLED` 门控） | P2 |
| P1-F3-07 | `src/__tests__/use-xterm-lifecycle.test.ts` 等 | 补充 hook-event 过滤、状态更新、`DefaultTab` emoji 渲染、`tabRules` icon 移除的 L2 断言 | P1 |

---

## P1-TE 测试

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-TE-01 | `src-tauri/src/hooks/` | L1：hooks 模块单元测试 ≥ 20 条；pty env 注入测试 ≥ 1 条；全部 `--test-threads=1` | P1 |
| P1-TE-02 | `src/__tests__/` | L2：`ipc-hooks-contract.test.ts` + `claude-status.test.ts` + useXterm/TerminalPanel/TabTitleRegistry 更新 ≥ 15 条新增 | P1 |
| P1-TE-03 | `e2e-tests/test.e2e.ts` | L4 关键路径 1-2 条：注入命令状态回显 + Node 写信号文件 → 页签 emoji 出现/消失 | P2 |
| P1-TE-04 | `e2e-tests/helpers.ts` | E2E helper：`installAllE2eHelpers` 中追加 hooks 相关 helper（inject/uninstall/status） | P2 |
| P1-TE-05 | `.claude/test-inventory.md` | 更新用例数（L1 + L2 + L4 新增） | P3 |

---

## P1-DOC 文档同步（最后 Stage）

| ID | 位置 | 修复/实现要点 | 优先级 |
|----|------|--------------|--------|
| P1-DOC-01 | 新建 `src-tauri/src/hooks/CLAUDE.md` | 模块职责、信号目录、watcher 生命周期、三命令、注入配置段识别规则、测试模式 | P3 |
| P1-DOC-02 | `src/ipc/CLAUDE.md` | 新增 `src/ipc/hooks.ts` 映射表条目 | P3 |
| P1-DOC-03 | `src/lib/CLAUDE.md` | 新增 `src/lib/claudeStatus.ts` 条目 | P3 |
| P1-DOC-04 | `src/panels/CLAUDE.md` | 更新 useCommandDetection、useXterm、DefaultTab、tabRules 描述；F3 四态决策说明 | P3 |
| P1-DOC-05 | `.claude/test-inventory.md` | 同步新增测试文件与用例数 | P3 |

---

## 跨边界契约速查（写死于 stages.md 与脚本头部）

- IPC 命令：`hooks_inject` / `hooks_uninstall` / `hooks_injection_status`
- Tauri Event：`hook-event`
- 信号目录：`~/.slterminal/hooks-events/`
- Hook 脚本：`~/.slterminal/hooks/slterm-hook-reporter.js`
- 用户配置：`~/.claude/settings.json`
- DTO：`HookInjectionStatus { status: "injected"|"notInjected"|"outdated", version: number|null }`、`HookEventPayload { panelId, event, timestamp, sessionId, transcriptPath, cwd, toolName, notificationType }`
- 环境变量：`SLTERM_PANEL_ID`
- 四态 emoji：`working=⚡` / `attention=🟡` / `done=✅` / `error=❌`
