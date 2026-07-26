# Stage 3 逐项验证断言

> stage-3 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P2-FE-07**：`src/features/sideViews/sideViewDefs.ts` 含 `id: "agent-status"` 的 `sideViewRegistry.register` 调用；title 为 "Agent 状态"；icon 为 "🤖"；component 为 AgentStatusView。
- **P2-FE-08**：`src/features/sideViews/sideBarState.ts` 的 `DEFAULT_ZONES.top` 数组含 `"agent-status"`。
- **P2-FE-09**：存在 `src/features/agentStatus/AgentStatusView.tsx` 与 `src/features/agentStatus/index.ts`；AgentStatusView 接受 `SideViewComponentProps`（`switchToPage`、`onDeletePage`）；渲染三态（no-root / empty / ready）；标题栏为 "AGENT STATUS"；根容器 `data-e2e="agent-status-view"`。
- **P2-FE-10**：存在 `src/features/agentStatus/useAgentStatus.ts`；从 `useLayout` + `useProjects` 推导当前项目；初始扫描 `TerminalRegistry.getAll()`；订阅 `onHookEvent`；按事件更新/移除行；按项目过滤；按 `lastEventAt` 倒序排序；事件驱动调用 `hooksContextUsage`。
- **P2-FE-11**：存在 `src/features/agentStatus/AgentStatusRow.tsx`；显示标题、四态图标、用量条；用量条百分比计算使用 `CLAUDE_CONTEXT_LIMIT`（200000）上限；不可用态显示 "--"；颜色引用 `theme/colors.ts` token。
- **P2-FE-12**：点击 AgentStatusRow 时调用 `switchToPage` 与 `window.__dockviewApi?.getPanel(panelId)?.focus()`。
- **P2-FE-13**：`useAgentStatus` 中 `contextUsage` 调用仅在事件含 `transcriptPath` 时触发；解析失败不抛异常，行仍显示；`CLAUDE_CONTEXT_LIMIT` 仅定义于 `src/features/agentStatus/consts.ts`。
- **P2-FE-14**：`src/theme/colors.ts` 存在 `AGENT_STATUS_USAGE_COLORS` token 组，含 `low`/`medium`/`high` 三段色。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`

## 语义式断言

- Agent Status 视图不得轮询刷新：不存在 `setInterval` / `setTimeout` 周期性拉取（grep `setInterval` / `setTimeout` 在 `src/features/agentStatus/` 中须 Read 确认仅用于一次性 debounce 或副作用清理，非轮询）。
- 行的 `panelId` 必须来自 `TerminalRegistry` 的键或 hook-event payload，不接受硬编码测试值（须 Read 代码确认）。
- 视图范围必须严格等于“当前项目”：代码中过滤逻辑使用当前活跃项目的 pages 数组，不得显示其他项目会话（须 Read 代码确认）。
- **Stop 事件后对应行状态置 `done` 并保留在列表中；`SessionEnd`/exit 事件才移除行**（须 Read 代码确认）。
- 所有颜色必须从 `theme/colors.ts` 引用，禁止硬编码色值（grep `src/features/agentStatus/` 中的 `#` 十六进制字面量，除 CSS token 外应为零；须 Read 确认）。
