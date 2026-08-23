# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/ipc/` 是前端唯一允许调用 Tauri `invoke` 的通信层。其他前端文件（组件、store、hook）**禁止**直接 `invoke` 或导入 `@tauri-apps/api/core`，必须通过本层封装函数访问后端。本层把后端命令按领域聚合成前端可调用的类型安全 wrapper，同时承载 DTO 与错误解析约定。

## 关键约束与决策

### invoke 单点（硬约束 #1）

`invoke` 调用只出现在本目录文件内。新增系统调用必须先在此封装，禁止组件直接调用 Tauri API。

### Channel 模式

流式数据通过 `Channel<T>` 推送，调用方传入 `onOutput`/`onChunk` 回调。典型用例：

- **PTY spawn**：`pty.spawn(request, onOutput)` 把 `Channel` 的 `onmessage` 绑定到回调。
- **大文件读取（BE-03）**：`fs.readFile` 后端按 256KB 块经 `onChunk` Channel 推送，终态 `{ data: "", done: true }`；wrapper 聚合拼接后 resolve，削大文件内存/IPC 峰值。

### Event 模式

`onFsEvent` / `onAgentEvent` 封装 Tauri `listen(...)`，返回 unsubscribe 函数。调用方负责在卸载时取消订阅。

### PTY 命令归属校验（SEC-08）

`pty.write` / `pty.resize` / `pty.kill` 三 wrapper 签名均含 `panelId`，invoke payload 同步传 `panelId`（JS `panelId` ↔ Rust `panel_id` 由 Tauri 自动转换）。后端凭此校验面板归属。

### 文件监听成对（BE-10）

`notify.startWatch` / `notify.stopWatch` 必须成对调用——项目移除/切换时调用 `stopWatch` 释放后端 watcher，防占用至 LRU 淘汰。`onFsEvent` 是全局事件监听，不成对。

### 剪贴板读权限消费点登记（SEC-06）

`clipboard.readText` 唯一消费点 = `src/panels/terminal/keyboard.ts` 的 `terminal.paste` 命令（Ctrl+Shift+V 显式手势）。OSC 52 handler 仅 `writeText` 只写不读。`clipboard-guard.test.ts` 以 grep 级守卫锁定此约定。

### agent hooks 泛化命令（MC-211）

`agentHooks.ts` 所有 wrapper 加 `cliId` 首参：`inject(cliId)` / `uninstall(cliId)` / `getInjectionStatus(cliId)` / `restoreStatusline(cliId)`。未知 cliId → 后端 Validation。

### hooks 配置命令与 hooks 注入命令分离

- `agentHooks.ts`：C6 注入/卸载/状态/事件订阅（`agent_hooks_inject` 等 + `onAgentEvent`）。
- `hooksConfig.ts`：C13-1 配置编辑（`agent_hooks_config_read` / `agent_hooks_config_write`）。

`writeHooksConfig` 传 hooks 子树，后端 **read-modify-write merge**（替换/插入 hooks 键，保留 permissions/env 等其他字段）。user 层不传 `projectPath`；project/local 层必须传（后端沙箱校验后拼接 `.claude/settings.json` / `.claude/settings.local.json`）。

### agent history 命令（MC-303/306）

- `scanAgentHistory(cliId, force?)`：后端按 (目录 mtime, 文件数) 进程内缓存，`force=true` 绕过缓存。**无参导出已删除**——后端 `cli_id` 必填。
- `deleteHistorySession(cliId, sessionId)`：后端按 cliId 路由 provider，delete 前经该 provider `validate_session_id` 前置校验（前端不传路径，仅传 cliId + sessionId，SEC-05 等价强制）。
- `readHistoryTitle(cliId, sessionId)`：回退链与历史扫描同源，会话文件不存在 → `title: null`（非 Err）。

### 窗口控制 wrapper（TB-03）

`window.ts` 提供六个 wrapper：

- `registerCloseHandler`：封装 `onCloseRequested` 生命周期（preventDefault + 回调 + finally destroy）。
- `onFocusChanged`：窗口焦点监听。
- `requestUserAttention`：任务栏闪烁。
- `minimizeWindow` / `toggleMaximizeWindow` / `closeWindow`：自绘标题栏三钮。`closeWindow` 必须走 `getCurrentWindow().close()` 触发 `onCloseRequested`——复用 P1-19 关窗链路杀 PTY，禁止 `destroy`/`process.exit` 绕过。

### AppError 统一解析（FE-02）

`appError.ts` 提供 `parseAppError(err)` + `getErrorMessage(err)` + `APP_ERROR_VARIANTS`。`parseAppError` 按 camelCase 变体名解析后端 AppError 序列化形态（`src-tauri/src/error.rs` serde 枚举），非 AppError 返回 null。`APP_ERROR_VARIANTS` 须与 error.rs 枚举逐一对应。

### thin wrapper 与工厂包装

- clipboard、dialog（open/save）、shell 是 Tauri 官方插件的直接 re-export，仅为了聚合到本层。
- notification 包含 `sendToastNotification` 工厂逻辑（Tauri 原生 `sendNotification` 通道，无点击路由），非纯 re-export。
- **`dialog.ask` 已删除（OV-02）**：确认语义改经 `src/lib` 的 `confirmDialog`，不再透出 `ask`。

### 参数序列化

- `Uint8Array` 需转 `Array.from(data)` 再传给 `invoke`（Tauri IPC 不支持 TypedArray）。
- camelCase ↔ snake_case 由 Tauri 自动转换。

## 外部坑/红线

- **mockIPC 不验证真实序列化**：契约测试用 `mockIPC` 只守 JS 侧形状（命令名、payload 字段名/类型、返回透传、异常传播）。camelCase↔snake_case 真实字段转换、Channel 序列化、Uint8Array↔number[]、listen 回调运行时解包由 L4 E2E 守卫。
- **后端必填参数缺失时 invoke 必 reject 且被调用方 catch 吞 = 契约绿但运行时静默失败**：此场景由 L4 兜底。
- **PTY `onOutput` 必须绑定到 `Channel.onmessage`**：spawn wrapper 负责把回调挂到 Channel，测试需断言此绑定。
- **`dialog.ask` 不存在**：任何确认需求改走 `src/lib/ConfirmDialog`。
- **notification 无点击路由**：Tauri 原生通知无 onClick，仅作回窗引导（任务栏闪烁）。

## 测试模式

- 测试文件位于 `src/__tests__/`：`ipc-contract.test.ts`、`ipc-ping.test.ts`、`app-error.test.ts`、`clipboard-guard.test.ts`、`ipc-agent-hooks-contract.test.ts`、`ipc-hooks-config-contract.test.ts`、`ipc-agent-history-contract.test.ts`、`ipc-window-contract.test.ts`、`notification.test.ts`。
- **契约四维验证**：命令名、参数结构、正常返回、异常传播。共享工厂 `helpers/ipc-contract.ts` 以声明式 schema 驱动。
- **Channel 绑定验证**：PTY spawn / fs read 用 `assertArgs` 断言 `Channel.onmessage` 绑定到传入回调。
- **Uint8Array 序列化验证**：`pty.write()` 用例断言参数为 `number[]` 而非 `Uint8Array`。
- **listen 事件封装手写测试**：`onFsEvent` / `onAgentEvent` 不走 invoke 工厂——捕获 `listen(event, handler)` 注册的 handler，构造 `{ payload }` 事件对象断言 callback 收到解包后 payload。
- **notify 全局 mock**：`setup.ts` 全局 mock 了 `../ipc/notify`。需要真实 `startWatch` 的测试须在文件顶部用 `vi.mock` 覆盖并 `importOriginal`。
