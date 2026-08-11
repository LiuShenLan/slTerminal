# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

Agent 状态视图（`agent-status` 侧栏视图，图标 🤖）——一屏总览当前活跃项目所有**运行中**的编码 CLI 会话：四态（F3）、上下文用量条、最后事件时间，点击行跳转聚焦对应终端页签。行 = 运行中的编码 CLI 会话（`agentSession` 为 null/undefined 的纯 shell 终端不建行）。

宿主：注册为 `src/features/sideViews/sideViewDefs.ts` 的 `agent-status` 视图（详见 @../sideViews/CLAUDE.md）；`useAgentHistory` 上提本模块单实例（历史区数据经 props 注入）。

## 架构决策

### 双行式行（AgentStatusRow）

行1 = 四态图标 + **CLI 品牌 logo（F9）** + 标题（12px 粗体）；行2 = 用量条 + 百分比 + 相对时间（11px 灰，缩进对齐图标列）。

- **图标列 40px flex 簇**：emoji 与 CLI logo（16×16，`cliProfileRegistry.get(row.cliId)?.iconSrc`——**按行 cliId 查 profile**（MC-411），OSC 133-only 行同样有 cliId（logo 可展示），未注册 cliId → undefined → 无 logo 不报错）列内居中成组（列内 gap 4px）；`{icon}` 为空仍渲染空列占位（行1 标题起点恒定不漂移）；logo 仅随 emoji 显示（icon 为空 → 无 logo）
- **行2 缩进 48px**（= 图标列 40 + gap 8），用量条与行1 标题起点对齐

**用量口径** = `(inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / contextLimit`（`contextLimit` = `cliProfileRegistry.get(row.cliId)?.capabilities?.hooks?.contextLimit`，缺失 → `--`，MC-412；outputTokens 不计占用保留为信息字段）。时间口径与历史区统一（`formatRelativeTime`，见 @../agentHistory/CLAUDE.md）。

### 行建模：建行双通道 / 删行三通道（F5）

- **建行双通道幂等**：`sessionChange` 事件 session 非 null ∨ hook 事件非 SessionEnd/Exit 且行不存在
- **删行三通道**：`sessionChange` session 为 null ∨ SessionEnd/Exit 事件 ∨ `remove`（面板关闭）
- **行 cliId（MC-410）**：hook 事件通道建行按 MC-205 三级解析写入——经 `resolvePayloadCliId` 单点（`src/panels/terminal/resolvePayloadCliId.ts`，ZQ-2 契约 4：`payload.cliId` trim 后非空 → `TerminalRegistry.get(panelId)?.agentSession?.cliId` 反查 → `CLAUDE_CLI_ID` 缺省；空串/仅空白与 null/undefined 同等回退）；OSC 133/sessionChange 通道建行取 `agentSession.cliId`（缺省兜底）；无 hooks 能力或未知 cliId → `console.warn` + 跳过（不建行/不置图标/不通知，MC-206）
- **建行 status（ZQ-3 决策 2）**：hook 事件通道建行 `status = newStatus` 原样写入（null 映射事件建行但 status null 无图标）——感知存活（SessionStart 丢失场景）且不误标 attention；更新已有行 null 不覆盖旧值逻辑不变；活跃区 null 行 ↔ 历史区「status null 不产出键」语义一致
- **初始扫描**：只建 `agentSession` 非 null 的行并携 `usageSourcePath` 主动拉 `contextUsage`（修复切项目后 idle 会话用量永远 --）
- **竞态双保险**：双 listener 经 ref 读最新状态 + deps `[]` 订阅永不重建 + reconcile 对账兜底

### 相对时间 60s ticker

`now` 相对时间基准由 60s ticker 驱动 `formatRelativeTime` 重算——idle 会话无 hook 事件时时间文本冻结的修复。

### E2E 兼容红线

`data-e2e="agent-status-view"` 根容器、`data-e2e="agent-status-row"`、标题栏 "AGENT STATUS"、空态文案「选择一个项目」「无运行中的编码 CLI 会话」逐字保留（`agent.e2e.ts` 依赖，MC-414 本域唯一用户可见文案变动）。

## 文件

| 文件 | 职责 |
|------|------|
| `AgentStatusView.tsx` | 视图组件（F7 三下拉框结构）：活跃会话（useAgentStatus + AgentStatusRow，行标题经历史区 scan 数据覆盖——问题 6 修复，复合键 `cliId\|sessionId` 匹配，MC-314）+ 当前项目历史会话 + 全部项目历史会话（挂载 `AgentHistorySections` 受控区，useAgentHistory 上提本组件单实例）；默认活跃展开、两历史区收起；区块标题 13px 粗体 + 内容缩进引导线（问题 4 三级字号） |
| `AgentStatusRow.tsx` | 会话行组件（双行式，见上） |
| `useAgentStatus.ts` | 数据 hook：`useAgentStatus()` → `AgentStatusResult`（`state` + `rows: AgentSessionRow[]` + `now` 时间基准）+ TerminalRegistry 订阅（register/remove/sessionChange）+ contextUsage 拉取 |
| `index.ts` | barrel export：`export { AgentStatusView }` |

## 测试模式

L2 测试位于 `src/__tests__/`（用例数见 `.claude/test-inventory.md`）：

- `agent-status-view.test.tsx` — AgentStatusRow 双行布局 + AgentStatusView 三区结构
- `agent-status-hook.test.ts` — useAgentStatus sessionId 字段、行建模通道（F5）

另有 `terminal-registry.test.ts` / `use-xterm-lifecycle.test.ts` 覆盖 agentSession sessionId/status 存储与 hook 事件写入（HUK1-9）。
