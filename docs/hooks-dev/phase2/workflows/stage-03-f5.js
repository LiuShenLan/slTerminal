// =====================================================================
// Stage 3 Workflow：前端 F5 Agent Status 侧栏视图
// =====================================================================
// 跨边界契约（本脚本头部写死，agent 不各自推断）：
//   侧栏视图 id：agent-status
//   标题：Agent 状态
//   图标：🤖
//   默认归属：DEFAULT_ZONES.top（追加在 commit 之后）
//   上下文用量上限：CLAUDE_CONTEXT_LIMIT = 200_000（前端单点 src/features/agentStatus/consts.ts）
//   事件名：hook-event
//   命令名：hooks_context_usage（Stage 1）
//   四态映射：src/lib/claudeStatus.ts（阶段 1）
//   AgentStatusView 必须接受 SideViewComponentProps（switchToPage, onDeletePage）
// =====================================================================

export const meta = {
  name: 'stage3-f5-agent-status',
  description: '阶段 2 Stage 3：前端 F5 Agent Status 侧栏视图',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\data\learn\code\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
Stage 特殊纪律：本 Stage 只改前端代码；禁止改后端、禁止改 ConPTY；所有颜色必须从 theme/colors.ts 引用（硬约束 #6）；新增侧栏视图必须走 sideViewRegistry 注册流程并接受 SideViewComponentProps。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'fe-registry',
    prompt: `你负责 P2-FE-07 / P2-FE-08：注册 agent-status 侧栏视图并更新默认 zones。

【P2-FE-07】src/features/sideViews/sideViewDefs.ts：
- 导入新建的 AgentStatusView（路径 src/features/agentStatus/AgentStatusView）。
- 追加注册：
  \`\`\`ts
  sideViewRegistry.register({
    id: "agent-status",
    title: "Agent 状态",
    icon: "🤖",
    component: AgentStatusView,
  });
  \`\`\`

【P2-FE-08】src/features/sideViews/sideBarState.ts：
- \`DEFAULT_ZONES.top\` 追加 \`"agent-status"\`（位于 \`"commit"\` 之后）。

要求：
- 不要修改其他视图的注册顺序或默认值。
- 完成后报告两处修改行。`
  },
  {
    label: 'fe-view',
    prompt: `你负责 P2-FE-09：新建 src/features/agentStatus/AgentStatusView.tsx 与 index.ts。

前置：src/features/sideViews/sideViewDefs.ts 已注册 agent-status（fe-registry agent）；src/lib/claudeStatus.ts 已存在（阶段 1）。

【P2-FE-09】AgentStatusView.tsx：
- 接受 \`SideViewComponentProps\`（\`switchToPage\`、\`onDeletePage\`），与 SidebarTree/ExplorerPanel/CommitView 对齐。
- 使用 \`useAgentStatus()\`（本 Stage fe-hook agent 负责）获取 \`state\`、\`rows\`、\`currentProjectName\`。
- 状态机（优先级自上而下）：
  - \`no-root\`：显示 "选择一个项目以查看 Agent 状态"；
  - \`empty\`：显示 "当前项目无运行中的 claude 会话"；
  - \`ready\`：渲染行列表。
- 标题栏 "AGENT STATUS"（28px 高、大写、letterSpacing 1、fontSize 11、颜色用 INPUT_BORDER），样式照 CommitView.tsx。
- 根容器使用 PANEL_BG、flex column、overflow hidden，data-e2e="agent-status-view"。
- index.ts：barrel export \`AgentStatusView\`。

要求：
- 所有颜色从 theme/colors.ts 引用。
- 完成后报告文件清单与关键渲染路径。`
  },
  {
    label: 'fe-hook',
    prompt: `你负责 P2-FE-10 / P2-FE-13：新建 src/features/agentStatus/useAgentStatus.ts 与 consts.ts。

前置：src/ipc/hooks.ts 的 onHookEvent 与 contextUsage 已可用；src/lib/claudeStatus.ts 已存在；src/panels/terminal/TerminalRegistry 已存在。

【P2-FE-10】useAgentStatus.ts：
- 导出类型与 hook：
  \`\`\`ts
  export interface AgentSessionRow {
    panelId: string;
    pageId: string;
    projectId: string;
    title: string;
    status: ClaudeStatus;
    lastEventAt: number;
    transcriptPath?: string;
    usage?: { inputTokens: number; outputTokens: number } | null;
  }
  export function useAgentStatus(): { state: AgentStatusState; rows: AgentSessionRow[]; currentProjectName: string | null };
  \`\`\`
- 从 \`useLayout\` + \`useProjects\` 推导当前活跃项目的 projectId、rootPath/cwd、projectName。
- 初始扫描 \`TerminalRegistry.getAll()\` 的 panelId 列表，仅保留属于当前项目 pageId 的终端。
- 订阅 \`onHookEvent\`：
  - 按 \`panelId\` 更新行：
    - \`SessionStart\` / \`UserPromptSubmit\` / \`PreToolUse\` / \`PostToolUse\` / \`PermissionRequest\` / \`Notification\`（工作/注意事件）→ \`working\` 或 \`attention\`（具体由 claudeStatus.ts 映射决定）。
    - \`Stop\` → 状态改为 \`done\`（**保留在行列表**）。
    - \`SessionEnd\` / OSC 133 退出事件 → 立即移除。
  - 更新 \`lastEventAt\` 为事件 timestamp。
  - 事件含 \`transcriptPath\` 时，异步调用 \`hooksContextUsage(transcriptPath)\` 更新该行的 \`usage\`；失败则 \`usage = null\`。
- 过滤：仅保留 panelId 所属 pageId 在当前项目 pages 数组中的行。
- 排序：按 \`lastEventAt\` 倒序。
- 切换项目时：清空旧项目行，根据新项目重新扫描 TerminalRegistry。

【P2-FE-13】consts.ts 与用量降级：
- 新建 \`src/features/agentStatus/consts.ts\`，单点导出 \`export const CLAUDE_CONTEXT_LIMIT = 200_000;\`。
- \`usage === undefined\` 且尚未请求时显示加载态（可选，推荐直接显示不可用）。
- \`usage === null\` 时显示不可用态（灰色条 + "--"）。
- 正常值时计算百分比。

要求：
- 不要轮询；仅在事件到达时触发 \`contextUsage\`。
- 所有项目遍历使用 useProjects.getState() 快照。
- \`CLAUDE_CONTEXT_LIMIT\` 不得在其他文件重复定义。
- 完成后报告状态流转与事件处理表。`
  },
  {
    label: 'fe-row',
    prompt: `你负责 P2-FE-11 / P2-FE-12 / P2-FE-14：新建 src/features/agentStatus/AgentStatusRow.tsx 并在 theme/colors.ts 新增 token。

前置：useAgentStatus.ts 已定义 AgentSessionRow（fe-hook agent）；consts.ts 已导出 CLAUDE_CONTEXT_LIMIT。

【P2-FE-14】src/theme/colors.ts：
- 新增 \`AGENT_STATUS_USAGE_COLORS\` token 组：
  \`\`\`ts
  export const AGENT_STATUS_USAGE_COLORS = {
    low: "#629755",
    medium: "#BBB529",
    high: "#F44747",
  } as const;
  \`\`\`
- 阈值由组件逻辑决定：<50% low，50-80% medium，>80% high。

【P2-FE-11】AgentStatusRow.tsx：
- props：\`{ row: AgentSessionRow; onFocus: (panelId: string) => void }\`。
- 行容器：flex row、align center、padding "4px 8px"、hover 背景用 SIDEBAR_COLORS.hover、selected 背景用 SIDEBAR_COLORS.selected。
- 图标：调用 \`getStatusIcon(row.status)\`（来自 src/lib/claudeStatus.ts），显示为固定宽度。
- 标题：显示 \`row.title\`，超出截断（textOverflow ellipsis）。
- 用量条：
  - 百分比 \`total = row.usage ? row.usage.inputTokens + row.usage.outputTokens : 0\`；
  - \`percent = Math.min(100, (total / CLAUDE_CONTEXT_LIMIT) * 100)\`；
  - 渲染一个固定宽度（如 80px）的条形，填充部分颜色按 percent 分段读取 \`AGENT_STATUS_USAGE_COLORS\`；
  - 不可用态：条形全灰 + 文本 "--"。
- 时间：右侧显示 \`new Date(row.lastEventAt).toLocaleTimeString()\`。

【P2-FE-12】点击行：
- 整行可点击；onClick 中调用 \`onFocus(row.panelId)\`。
- \`onFocus\` 由父组件 AgentStatusView 提供，内部执行：
  1. 解析 panelId → pageId；
  2. 调用 \`switchToPage(row.projectId, row.pageId)\`；
  3. 调用 \`window.__dockviewApi?.getPanel(row.panelId)?.focus()\`。

要求：
- 所有颜色从 theme/colors.ts 引用。
- data-e2e="agent-status-row" 与 \`data-panel-id={row.panelId}\`。
- 完成后报告组件 props 与点击链路。`
  }
]

const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\data\learn\code\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage3 的改动是否实际生效（项目根 D:\data\learn\code\slTerminal）。
先读 docs/hooks-dev/phase2/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
返回 JSON：{ "allFixed": true/false, "failedItems": ["未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
