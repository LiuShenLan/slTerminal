# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

通知调度模块（F4）——订阅 agent-event 事件流，在窗口失焦时触发桌面 toast 通知 + 任务栏闪烁。三类事件映射：权限请求 / 任务完成 / 错误。

**点击路由已放弃**：`sendToastNotification` 无 onClick（Tauri 原生通知通道），**任务栏闪烁是唯一的回窗引导通道**，三类事件全覆盖（P2-FE-09 去路由化）。

## 关键约束与决策

### 事件分类（classifyEvent 纯函数，MC-420）

通用门控（失焦/去重/seenRef 截断）CLI 无关保留本模块；类别判定**委托 profile**：

- 按 MC-205 三级解析取 profile 后调 `profile.capabilities.hooks.classifyNotification(payload)`。
- 三级解析经 `resolvePayloadCliId` 单点（ZQ-2）：`payload.cliId` trim 后非空 → `TerminalRegistry.get(panelId)?.agentSession?.cliId` 反查 → `CLAUDE_CLI_ID` 缺省回退；空串/仅空白与 null/undefined 同等回退。
- 未知 cliId（未注册）→ `console.warn` + 返回 null（不通知，MC-206）。
- 无 hooks 能力 profile → 返回 null（不通知）。

claude 类别规则（实现于 `profiles/claude/strategies.ts`）：

- **permission**：`PermissionRequest` 或（`Notification` 且 `notificationType === "permission_prompt"`）
- **error**：`StopFailure` / `PostToolUseFailure`
- **done**：`Stop`
- 其他事件 → 不触发通知

### 失焦门控

`window.__slterm_windowFocused !== false` 时（窗口聚焦中）不触发——后台运行时才打扰用户。

### 去重

`seenRef` 基于 `sessionId|event|timestamp` 键去重（防同一信号文件重复投递）；缓存超 200 条截断保留最近 100 条。

### toast 正文

`<项目名> · <emoji 类别标签> · <时间>`——项目名从 panelId 反查（`parseTerminalPageId` → `useProjects`），反查不到为空。

### 权限懒初始化

`permissionEnsured` 模块级标记，首次调用时 `ensureNotificationPermission()`（仅一次）；权限被拒后 `sendToastNotification` 内部 catch 输出 console.error。

## 测试模式

L2 测试位于 `src/__tests__/`：

- `notifications.test.ts`（P2-TE-04）：失焦 + 三类事件 → toast + 任务栏闪烁、聚焦时不通知、toast 正文含项目名 + 类别、classifyEvent 类别判定委托 profile 分支。
- `notification.test.ts`：`src/ipc/notification.ts` 封装分支覆盖（IHE-02）。
- `cli-profile-claude.test.ts`：覆盖 claude `classifyNotification` 五映射纯函数层。
