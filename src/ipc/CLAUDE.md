# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

`src/ipc/` 是前端唯一允许调用 Tauri `invoke` 的通信层。其他前端文件（组件、store、hook）**禁止**直接 `invoke` 或导入 `@tauri-apps/api/core`，必须通过本层封装函数访问后端。

## 模块映射

每个文件映射一个后端功能模块，命名一一对应：

| 文件 | 后端模块 | 封装的命令 |
|------|---------|-----------|
| `pty.ts` | `pty/` | `pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_reattach`, `get_windows_build_number`。`write`/`resize`/`kill` 三 wrapper 签名含 `panelId`（归属校验，后端 SEC-08），invoke payload 同步传 `panelId`（JS `panelId` ↔ Rust `panel_id` 由 Tauri 自动转换） |
| `fs.ts` | `fs/` | `fs_read_file`, `fs_write_file`, `fs_read_dir`, `fs_create_dir`, `fs_delete`, `fs_rename` |
| `git.ts` | `git/` | `git_status`, `git_diff`, `git_file_at_head`, `git_rollback`, `git_unstage` |
| `settings.ts` | settings | `load_settings`, `save_settings` |
| `projects.ts` | projects | `load_projects`, `save_projects`（绕过路径 sandbox，exe 同级 `slterminal-projects.json`） |
| `notify.ts` | `notify/` | `notify_watch`、`onFsEvent`（`listen("fs-event")` 封装） |
| `notification.ts` | Tauri plugin notification | re-export `isPermissionGranted` / `requestPermission` / `sendNotification` + `ensureNotificationPermission()` + `sendToastNotification(title, {body})`（Tauri 原生 `sendNotification` 通道，无 onClick——点击路由放弃。未打包 Win32 WebView2 无 AUMID：banner 可能被抑制、仅通知中心条目 + 任务栏闪烁作为回窗引导） |
| `clipboard.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-clipboard-manager`。由 `keyboard.ts`（Ctrl+Shift+C/V）和 `useXterm.ts`（OSC 52 handler）消费 |
| `dialog.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-dialog` |
| `window.ts` | Tauri Window API | `registerCloseHandler` — 封装 `onCloseRequested` 关闭生命周期 |
| `shell.ts` | Tauri plugin | `@tauri-apps/plugin-opener` 的 `openUrl` re-export |
| `agentHooks.ts` | `hooks/` | `agent_hooks_inject`, `agent_hooks_uninstall`, `agent_hooks_injection_status`, `agent_hooks_restore_statusline`——**wrapper 全部加 cliId 首参**（MC-211 泛化命令，未知 cliId → 后端 Validation）：`inject(cliId)` / `uninstall(cliId)` / `getInjectionStatus(cliId)`（返回 `AgentHookInjectionStatus`）/ `restoreStatusline(cliId)`（关闭清理：还原 statusline 桥接备份，App.tsx 关闭序列调用）；`onAgentEvent`（`listen("agent-event")` 封装，MC-202）。原 `contextUsage`（`agent_context_usage` transcript token 扫描）已随官方 used_percentage 口径退役删除 |
| `hooksConfig.ts` | `hooks/`（config.rs） | `agent_hooks_config_read`, `agent_hooks_config_write`（C13-1 配置编辑命令）：`readHooksConfig(cliId, layer, projectPath?)` 返回该层 settings.json 的 **hooks 子树**（文件不存在或无 hooks 键 → `null`，JSON 损坏 → 后端 Err）；`writeHooksConfig(cliId, layer, hooks, projectPath?)` 传 hooks 子树，后端 **read-modify-write merge**（替换/插入 hooks 键，原样保留 permissions/env 等其他字段），hooks 必须为 JSON Object。user 层不传 projectPath；project/local 层必须传（后端沙箱校验后拼接 `.claude/settings.json` / `.claude/settings.local.json`）。**与 `agentHooks.ts` 区分**：后者是 C6 注入/卸载/状态/用量命令 + agent-event 事件订阅，本文件是 C13-1 配置编辑命令的唯一 invoke 位置 |
| `agentHistory.ts` | `agent_history/` | `agent_history_scan`, `agent_history_delete`（历史会话查询与删除两命令——**rename 已随功能整体移除**，问题 7 修复；MC-303 泛化）：`scanHistory()` **无参聚合**扫描全部已注册 provider（返回 `AgentHistorySession[]`（含 `cliId`），单 provider 失败不阻塞其他、单文件失败降级条目、扫描根不存在返回空数组均非 Err）；`deleteHistorySession(cliId, sessionId)`——后端按 cliId 路由 provider，delete 前经该 provider `validate_session_id` 前置校验（**前端不传路径，仅传 cliId + sessionId，SEC-05 等价强制**），未知 cliId 返回 Err，invoke 参数 camelCase（JS `cliId`/`sessionId` ↔ Rust `cli_id`/`session_id` 由 Tauri 自动转换） |
| `index.ts` | — | barrel export，统一对外暴露；含 `ping()` 健康检查命令 |

## 编码约定

- **invoke 单点**：`invoke` 调用只出现在本目录文件内（架构硬约束 #1）。
- **Channel 模式**：流式数据（如 PTY 输出）通过 `Channel<T>` 推送，调用方传入 `onOutput` 回调。
- **Event 模式**：`onFsEvent` 封装 Tauri `listen<FsEvent>("fs-event")`，返回 unsubscribe 函数。`registerCloseHandler` 封装 `getCurrentWindow().onCloseRequested` 生命周期。
- **类型对应**：封装函数的参数/返回值使用 `src/types/` 中的 DTO 类型，与 Rust 端 `snake_case` 字段对应。
- **thin wrapper**：clipboard、dialog、shell 是 Tauri 官方插件的直接 re-export，仅为了聚合到本层，不添加额外逻辑。notification 包含 `sendToastNotification` 工厂逻辑（Tauri 原生 `sendNotification` 通道，无点击路由），非纯 re-export。新增 Tauri 插件导入遵循同一模式。
- **命名**：函数名 camelCase，对应的 Rust 命令为 snake_case（如 `pty_spawn` → `spawn()`）。
- **参数序列化**：`Uint8Array` 需转 `Array.from(data)` 再传给 `invoke`（`pty.write`）。`write`/`resize`/`kill` 的 invoke payload 均含 `panelId`（后端 SEC-08 归属校验），调用方须传入作用域内现成的 panelId。

## 测试模式

测试文件：`src/__tests__/ipc-contract.test.ts`（65 用例，含 3 条 DBG-4 契约守卫）+ `ipc-ping.test.ts`（2 用例，IHE-07① 改调 `ping()` wrapper）+ `ipc-agent-hooks-contract.test.ts`（22 用例，MC-212 泛化——含 cliId 首参四维）+ `ipc-hooks-config-contract.test.ts`（12 用例，C13-1 配置命令四维验证，含 cliId 首参）+ `ipc-agent-history-contract.test.ts`（8 用例，MC-306 更名——scan 无参 / delete 参数 `{cliId, sessionId}`）+ `ipc-window-contract.test.ts`（10 用例，`registerCloseHandler` 关闭生命周期契约）+ `notification.test.ts`（9 用例，IHE-02 分支覆盖）。共享工厂位于 `src/__tests__/helpers/ipc-contract.ts`（IHE-06）。

### mockIPC 盲区声明（IHE-01）

契约测试用 `mockIPC` 只守 **JS 侧形状**——命令名、payload 字段名/类型、返回透传、异常传播。以下行为**不在 mock 层验证**，由 L4 E2E（真实 WebView2 + 真实后端）守卫：

- camelCase ↔ snake_case 真实字段转换（Tauri invoke 序列化）
- Channel 序列化（PTY 输出流）
- Uint8Array ↔ number[] 实际序列化
- listen 回调运行时解包（`event.payload`）

即：**契约测试只防 wrapper 写错命令名/参数结构，真实序列化由 L4 守卫**。后端必填参数缺失时 invoke 必 reject 且被调用方 catch 吞 = 契约全绿但运行时静默失败——此场景由 L4 兜底（引用 DOC-01/02：L4 为半端到端定位声明）。四个契约文件（ipc-contract / ipc-agent-hooks-contract / ipc-hooks-config-contract / ipc-agent-history-contract）文件头均含此盲区注释。

### IPC 合约测试（IHE-06 工厂化）

核心思想：**用 `mockIPC` 拦截真实的 `invoke` 调用，验证每条封装函数的命令名、参数结构、返回类型和异常传播**。四文件经共享工厂 `describeIpcContract(scope, cases)`（`helpers/ipc-contract.ts`）以声明式 schema 驱动——每条用例为 `{ cmd, call, respond?, mockThrow?, expectArgs?, expectExactKeys?, expectResult?, expectUndefined?, expectReject?, assertArgs? }`。**四维断言不丢**：命令名逐字、payload 精确匹配/键集合/自定义、返回透传/void/fallback、异常传播。

### 四维验证

每个 IPC wrapper 测试覆盖四个维度：

| 维度 | 验证内容 | 示例 |
|------|---------|------|
| 命令名 | `invoke` 调用的 Tauri 命令名（snake_case） | `pty_spawn` 而非 `ptySpawn` |
| 参数结构 | 字段名、类型、值正确 | `expectArgs: { request: SPAWN_REQUEST, onOutput: expect.any(Channel) }` |
| 正常返回 | mockIPC 返回模拟数据 → wrapper 正确透传 | `expectResult: "mock-session-01"` |
| 异常传播 | mockIPC throw → wrapper 不吞异常 | `mockThrow + expectReject` |

### 关键模式

```typescript
// helpers/ipc-contract.ts 工厂内部等价实现（用例声明式驱动）
import { mockIPC } from "@tauri-apps/api/mocks";

// 1. 声明用例（命令名 + 参数 + 返回 + 异常四维）
{
  cmd: "pty_spawn",
  call: () => pty.spawn(request, onOutput),
  respond: "mock-session-01",
  expectArgs: { request, onOutput: expect.any(Channel) },
  expectResult: "mock-session-01",
  assertArgs: (args) => {
    expect((args.onOutput as Channel<unknown>).onmessage).toBe(onOutput);
  },
}

// 2. 工厂内部注册 mockIPC + spy
const commandSpy = vi.fn();
mockIPC((cmd, args) => {
  commandSpy(cmd, args);
  if (cmd === case.cmd && case.mockThrow !== undefined) throw new Error(case.mockThrow);
  return case.respond;
});

// 3. 验证命令名 + 参数 + 返回值（工厂统一断言）
expect(commandSpy).toHaveBeenCalledTimes(1);
expect(commandSpy.mock.calls[0][0]).toBe("pty_spawn");
expect(commandSpy.mock.calls[0][1]).toEqual({ request, onOutput: expect.any(Channel) });
```

### Channel 绑定验证

PTY spawn / reattach 的 `onOutput` 回调必须绑定到 `Channel.onmessage`——工厂 `assertArgs` 内断言：

```typescript
assertArgs: (args) => {
  expect((args.onOutput as Channel<unknown>).onmessage).toBe(onOutput);
}
```

### Uint8Array 序列化验证

`pty.write()` 必须将 `Uint8Array` 转为 `number[]`（Tauri IPC 不支持 TypedArray）：

```typescript
// 用例声明：expectArgs 断言转换结果
expectArgs: {
  sessionId: "session-01",
  panelId: "panel-1",
  data: [72, 101, 108, 108, 111],  // number[]，非 Uint8Array
}
```

### wrapper 行为契约（IHE-01②）

listen 事件封装（`onFsEvent` / `onAgentEvent`）**不走 invoke 工厂**——手写模拟驱动断言：捕获 `listen(event, handler)` 注册的 handler，构造 `{ payload }` 事件对象 → 断言 callback 收到**解包后** payload。Tauri `listen` 的运行时解包本身由 L4 E2E 守卫（mockIPC 层不验证）。

### notify mock 覆盖

`setup.ts` 全局 mock 了 `../ipc/notify`（防所有测试 import 时触发实际 listen）。ipc-contract 测试需要真实 `startWatch` → 在测试文件顶部用 `vi.mock` 覆盖全局 mock，`importOriginal` 获取原始实现。`notification.test.ts` 直接 `vi.mock("@tauri-apps/plugin-notification")` 覆盖插件三函数。
