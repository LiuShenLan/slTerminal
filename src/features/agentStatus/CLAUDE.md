# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

**活跃会话数据 hook 单点**——`useAgentStatus()` 维护运行中的编码 CLI 会话行模型（`AgentSessionRow[]`），是导航树活跃会话区（`navTree/NavSessionRow`）的唯一数据源。

**视图组件已退役（NAV-08，2026-08）**：`AgentStatusView` / `AgentStatusRow`（原 `agent-status` 侧栏视图，图标 🤖）随 UI 重设计整体删除——活跃会话区迁入导航树 NavTree（单行化 `NavSessionRow`，见 @../navTree/CLAUDE.md）；历史会话区迁入导航树历史折叠节点（`NavHistoryRow`）。`useAgentStatus` 留存不改（NAV-01 数据接入契约），`AgentStatusRow` 的展示逻辑（用量条四档分级/logo 按行 cliId 查 profile）由 `NavSessionRow` 照搬。

## 架构决策

### 行建模：建行双通道 / 删行三通道（F5）

行 = 运行中的编码 CLI 会话（`agentSession` 为 null/undefined 的纯 shell 终端不建行）。

- **建行双通道幂等**：`sessionChange` 事件 session 非 null ∨ hook 事件非 SessionEnd/Exit 且行不存在
- **删行三通道**：`sessionChange` session 为 null ∨ SessionEnd/Exit 事件 ∨ `remove`（面板关闭）
- **行 cliId（MC-410）**：hook 事件通道建行按 MC-205 三级解析写入——经 `resolvePayloadCliId` 单点（`src/panels/terminal/resolvePayloadCliId.ts`，ZQ-2 契约 4：`payload.cliId` trim 后非空 → `TerminalRegistry.get(panelId)?.agentSession?.cliId` 反查 → `CLAUDE_CLI_ID` 缺省；空串/仅空白与 null/undefined 同等回退）；OSC 133/sessionChange 通道建行取 `agentSession.cliId`（缺省兜底）；无 hooks 能力或未知 cliId → `console.warn` + 跳过（不建行/不置图标/不通知，MC-206）
- **建行 status（ZQ-3 决策 2）**：hook 事件通道建行 `status = newStatus` 原样写入（null 映射事件建行但 status null 无图标）——感知存活（SessionStart 丢失场景）且不误标 attention；更新已有行 null 不覆盖旧值逻辑不变
- **初始扫描**：只建 `agentSession` 非 null 的行（usage 由 ContextUsage 信号事件推送——桥接脚本 1s 节流，行建立后很快有数据；无数据 → `--`）
- **ContextUsage 信号分支**：`handleHookEvent` 前置分支——行存在才更新 `usage`（`usedPercentage` 数字校验），不建行/删行/不动状态（先于建行到达时忽略）；事件名经 `CONTEXT_USAGE_EVENT` 常量（profiles/claude，AC-5）
- **竞态双保险**：双 listener 经 ref 读最新状态 + deps `[]` 订阅永不重建 + 初始扫描按注册表现值对账兜底（事件丢失场景）

### 相对时间 60s ticker

`now` 相对时间基准由 60s ticker 驱动 `formatRelativeTime` 重算——idle 会话无 hook 事件时时间文本冻结的修复。

### 项目域过滤

`useAgentStatus` 内部按活跃项目过滤（`projectPageIds`/`projectRoot` 经 ref 供稳定订阅读取）——切换项目时仅展示该项目页面下的会话行。`AgentSessionRow` 携带 `panelId`/`pageId`/`projectId`/`cliId`/`title`/`status`/`usage` 字段，导航树据此按页面归组（`useNavTree` sessionsByPage）。

## 文件

| 文件 | 职责 |
|------|------|
| `useAgentStatus.ts` | 数据 hook：`useAgentStatus()` → `AgentStatusResult`（`state` + `rows: AgentSessionRow[]` + `currentProjectName` + `now` 时间基准）+ TerminalRegistry 订阅（register/remove/sessionChange）+ ContextUsage 信号事件处理（行 usage 更新）；行标题经 `resolveTitle`（getPageApi 查 dockview 面板标题） |

> 原 `AgentStatusView.tsx` / `AgentStatusRow.tsx` / `index.ts` 已删除（NAV-08）；本目录无 barrel——`useAgentStatus` 经 `../navTree/useNavTree.ts` 直接引用。

## 测试模式

L2 测试位于 `src/__tests__/`（用例数见 `.claude/test-inventory.md`）：

- `agent-status-hook.test.ts` — useAgentStatus sessionId 字段、行建模通道（F5）+ ContextUsage 信号分支（行存在更新/不建行/字段缺失忽略/不删除）
- `nav-tree.test.tsx` — NavSessionRow 消费（活跃会话按页面归组/单行渲染/StatusDot/迷你用量条/logo），逻辑照 AgentStatusRow 口径断言

另有 `terminal-registry.test.ts` / `use-xterm-lifecycle.test.ts` 覆盖 agentSession sessionId/status 存储与 hook 事件写入（HUK1-9）。
