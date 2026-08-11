# review-05 契约一致性

> 维度：前端类型 ↔ Rust DTO ↔ IPC 命令 ↔ 信号 payload 四层对账。只写问题。

## 问题条目

无发现问题。

## 已检查范围

- A. agent-event payload：reporter.js 9 键 ↔ signal.rs `AgentEventPayload`（camelCase、9 键、`cliId` serde default）↔ `src/types/agent.ts` 接口一致；emit/listen 事件名均为 `"agent-event"`，无 `hook-event` 残留。
- B. IPC 命令：8 条命令名（`agent_hooks_*` ×6 + `agent_history_scan/delete`）前后端逐字一致；参数 camelCase/snake_case 映射经 Tauri 自动转换（`cliId/sessionId/transcriptPath/projectPath` ↔ `cli_id/session_id/transcript_path/project_path`）；返回类型 `ContextUsage` / `AgentHookInjectionStatus` / `AgentHistorySession` 字段一一对应。
- C. trait/能力契约：`CliHooksProvider` 6 方法、`CliHistoryProvider` 3 方法与命令层调用一致；`CodingCliProfile` 的 `HooksCapability`/`HistoryCapability` 字段与 claude profile 实现及消费方（useXterm/useAgentStatus/useAgentNotifications/hooksConfig hub/historyContextMenu/restoreSession）完全对齐。
- D. E2E 契约：helpers.ts 声明的 `__slterm_e2e_*` 与 spec 调用名一致；`data-e2e` 选择器（agent-status-view/agent-status-row/agent-history-row/hooks-restart-hint 等）在组件与 spec 两侧字面一致。
- E. 错误契约：`AppError` camelCase 变体在前端无按 `kind` 字符串匹配代码，仅按消息展示/日志处理，形态一致。
