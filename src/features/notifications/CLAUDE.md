# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

通知调度模块（F4）——订阅 hook-event 事件流，在窗口失焦时触发桌面 toast 通知 + 任务栏闪烁。三类事件映射：权限请求 / 任务完成 / 错误。

**点击路由已放弃**：`sendToastNotification` 无 onClick（Tauri 原生通知通道），**任务栏闪烁是唯一的回窗引导通道**，三类事件全覆盖（P2-FE-09 去路由化）。

## 架构决策

### 事件分类（classifyEvent 纯函数）

规则（优先级自上而下）：

- **permission（🔐 权限请求）**：`PermissionRequest` 或（`Notification` 且 `notificationType === "permission_prompt"`）
- **error（❌ 错误）**：`StopFailure` / `PostToolUseFailure`
- **done（✅ 任务完成）**：`Stop`
- 其他事件（PreToolUse/PostToolUse/SessionStart/SessionEnd 等）→ 不触发通知

### 失焦门控

`window.__slterm_windowFocused !== false` 时（窗口聚焦中）不触发——后台运行时才打扰用户。

### 去重

`seenRef` 基于 `sessionId|event|timestamp` 键去重（防同一信号文件重复投递）；缓存超 200 条截断保留最近 100 条。

### toast 正文

`<项目名> · <emoji 类别标签> · <时间>`——项目名从 panelId 反查（`parseTerminalPageId` → `useProjects`），反查不到为空。

### 权限懒初始化

`permissionEnsured` 模块级标记，首次调用时 `ensureNotificationPermission()`（仅一次）；权限被拒后 `sendToastNotification` 内部 catch 输出 console.error。

## 文件

| 文件 | 职责 |
|------|------|
| `useClaudeNotifications.ts` | F4 通知调度 hook：`useClaudeNotifications()`（订阅 onHookEvent → 门控 → 分类 → 去重 → 闪烁 + toast）+ `classifyEvent` 纯函数 + `NotificationListener` 无 UI 副作用组件（App.tsx 挂载一次） |
| `index.ts` | barrel export |

## 测试模式

L2 测试位于 `src/__tests__/notification.test.ts`（用例数见 `.claude/test-inventory.md`）：`vi.mock("@tauri-apps/plugin-notification")` 覆盖插件三函数，覆盖 classifyEvent 分支（IHE-02）。`ipc-window-contract.test.ts` 覆盖 `requestUserAttention` 契约。
