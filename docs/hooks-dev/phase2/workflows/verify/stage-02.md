# Stage 2 逐项验证断言

> stage-2 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P2-FE-01**：`package.json` 的 `dependencies` 含 `@tauri-apps/plugin-notification`（grep 命中）。
- **P2-FE-02**：存在 `src/ipc/notification.ts` 且 re-export `isPermissionGranted` / `requestPermission` / `sendNotification`；存在 `sendClickableNotification(title, options, onClick)` 工厂，内部使用 `new Notification(title, options)` 并绑定 `onclick`；`src/ipc/index.ts` barrel export 含 `notification`。
- **P2-FE-03**：`src/ipc/hooks.ts` 存在 `contextUsage(transcriptPath)` 函数，内部调用 `invoke("hooks_context_usage", { transcriptPath })`。
- **P2-FE-04**：`src/App.tsx` 中引入窗口焦点监听逻辑；焦点状态写入模块级可访问位置（ref 或 window 全局），供通知模块读取；`NotificationListener` 组件被挂载（grep 命中）。
- **P2-FE-05**：`src/features/notifications/useClaudeNotifications.ts` 存在；订阅 `onHookEvent`；失焦门控代码读取焦点状态；三类事件（权限请求/Stop/StopFailure）分别触发 `sendClickableNotification`；权限请求期间调用任务栏闪烁 API `getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`（值 1）；窗口聚焦后调用 `requestUserAttention(null)` 停止。**不使用 `flashFrame`**。
- **P2-FE-06**：`sendClickableNotification` 的 `onClick` 回调中调用窗口聚焦、解析 pageId、调用 `switchToPage`、调用 `dockviewApi.getPanel(panelId)?.focus()`；页签关闭时不抛异常（代码含 optional chaining 或 try-catch，须 Read 确认）；**禁止**在 `sendNotification` 的 Options 上写 `onClick`（该字段不存在）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`

## 语义式断言

- 前端除 `src/ipc/notification.ts` 外，不存在任何直接 import `@tauri-apps/plugin-notification` 的文件（grep 全仓确认）。
- 通知触发必须受焦点门控约束：代码中不存在绕过焦点状态直接 `sendClickableNotification` / `sendNotification` 的路径（须 Read 代码确认所有调用点均在 `if (!focused)` 或等价的门控分支内）。
- toast 点击路由的 `panelId` 必须来自 hook-event payload，不接受硬编码（须 Read 代码确认取值来源）。
- 任务栏闪烁停止逻辑必须在窗口聚焦回调或事件状态变化时被调用，禁止持续闪烁（须 Read 代码确认）。
- `sendNotification` 调用点若存在，不得携带 `onClick` 选项（grep 确认）；所有点击路由必须走 `sendClickableNotification` 工厂。
