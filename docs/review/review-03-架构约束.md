# review-03 架构约束

> 维度：11 条硬约束 + multi-cli 新增纪律的合规性。只写问题。

## 问题条目

无发现问题。

## 已检查范围

1. **前端 invoke 单点**：`grep \binvoke\b` 于 `D:\data\learn\code\slTerminal\src`，仅 `src/ipc/` 与 `src/__tests__/` 命中，生产代码无例外。
2. **后端模块隔离**：`src-tauri/src/hooks/` 与 `src-tauri/src/agent_history/` 无互相 import；`pty/fs/git/notify` 未被 hooks/agent_history 引用。
3. **命令统一注册**：`agent_hooks_inject/uninstall/injection_status/context_usage/config_read/config_write` + `agent_history_scan/delete` 均注册于 `src-tauri/src/lib.rs` 的 `generate_handler!`，签名均为 `Result<_, AppError>`，阻塞 I/O 使用 `tokio::task::spawn_blocking`。
4. **DTO 双边对应**：`src/types/agent.ts`、`src/types/agentHistory.ts`、`src/types/hooksConfig.ts` 与 Rust `hooks/signal.rs::AgentEventPayload`、`hooks/mod.rs::AgentHookInjectionStatus/ContextUsage`、`agent_history/mod.rs::AgentHistorySession` 字段数量、命名、snake_case↔camelCase、可选性一致。
5. **面板封闭**：`HooksConfigPanel` 已在 `D:\data\learn\code\slTerminal\src\panelRegistry.ts` 注册。
6. **配色单点**：`src/features/agentStatus/`、`src/features/agentHistory/`、`src/features/cliProfiles/`、`src/panels/hooksConfig/` 无硬编码色值，均引用 `src/theme/colors.ts` token。
7. **布局单点**：`HooksConfigPanel` 的 `selectedCli` 经 `api.updateParameters` + `saveLayout(containerApi)` + `onLayoutChange` 持久化，符合 F8 先例。
8. **会话元数据单点**：`agentSession` 读写全部经 `D:\data\learn\code\slTerminal\src\panels\terminal\TerminalRegistry.ts` 的 `setAgentSession/getAll/subscribe`。
9. **平台分支收敛**：`src-tauri/src/hooks/` 与 `src-tauri/src/agent_history/` 中 `#[cfg(windows)]` 零命中。
10. **权限最小化**：`D:\data\learn\code\slTerminal\src-tauri\capabilities\default.json` 仅列显式插件权限，无通配 `*`。
11. **测试覆盖**：`hooks/provider.rs`、`agent_history/provider.rs` 注册表用例，`agent-history-model.test.ts` 复合键用例，`hooks-config-panel.test.tsx`/`mock-cli-profile.test.tsx` hub 选择行用例，`no-claude-literals.test.ts` AC-5 守卫均存在。

**multi-cli 新增纪律**：
- AC-5：除 `src/__tests__/no-claude-literals.test.ts` 已守卫的七路径外，额外抽查 `src/workspace/`、`src/stores/`、`src/features/explorer/`、`src/features/sidebar/`、`src/features/shortcuts/`、`src/features/sideViews/`、`src/features/commit/`，生产代码中无 `"claude"` 字符串字面量。
- claude 知识领地：生产代码中 `profiles/claude/` 外部仅引用其导出的 `CLAUDE_CLI_ID`/`SESSION_END_EVENT`/`EXIT_EVENT` 常量，无直接 import `profiles/claude/strategies.ts`。
- 注册触发点：仅 `D:\data\learn\code\slTerminal\src\workspace\Workspace.tsx` 显式 import `../features/cliProfiles/profiles` 触发 side-effect 注册。
