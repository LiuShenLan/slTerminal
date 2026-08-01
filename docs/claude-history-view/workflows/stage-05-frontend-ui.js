// =====================================================================
// Stage 05: 历史区 UI（双行式/搜索/右键菜单）+ AgentStatusView 三下拉框
// 覆盖项: FE-07、FE-08、FE-09、FE-11、FE-12
// 跨边界契约（写死，双 agent 不各自推断）:
//   interface HistorySessionRowProps {
//     session: HistorySession; active: boolean; orphan: boolean; noCwd: boolean;
//     selected: boolean;
//     onSelect(id: string): void;
//     onDoubleClick(session: HistorySession): void;
//     onContextMenu(session: HistorySession, pos: { x: number; y: number }): void;
//   }
//   interface InputDialogProps { title: string; initialValue: string; onSubmit(v: string): void; onCancel(): void; }
//   getHistoryContextMenuItems(session, opts): { label: string; disabled?: boolean; action(): void }[]
//     opts = { active: boolean; orphan: boolean; noCwd: boolean;
//              onCopy(): void; onFork(): void; onDelete(): void; onRename(): void }
//   useClaudeHistory() 返回形状（Stage 04 契约）: { state, sessions, activeIds, rootPath, scan, removeLocal, updateLocalTitle }
// E2E 兼容红线（硬约束，实证消费方 e2e-tests/test.e2e.ts 与 agent-status-view.test.tsx）:
//   AgentStatusView 改造必须保留——根容器 data-e2e="agent-status-view"、活跃行
//   data-e2e="agent-status-row"、"AGENT STATUS" 标题栏文本、空态文案
//   「无运行中的 claude 会话」「选择一个项目」原文。
// data-e2e 清单（FE-12，逐字）:
//   agent-history-search / agent-history-refresh / agent-history-section-current /
//   agent-history-section-all / agent-history-group / agent-history-row /
//   agent-history-menu / agent-history-input-dialog
// fix-loop 调用本 Stage 时 args.constraints 传空串（无特殊纪律）
// =====================================================================

export const meta = {
  name: 'stage05-frontend-ui',
  description: 'Stage 05: 历史区 UI（双行式/搜索/右键菜单）+ AgentStatusView 三下拉框',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 FE-07..09/11/12 条目 + docs/claude-history-view/stages.md Stage 05 实现要点与「跨 Stage 契约」「E2E 兼容红线」段 + docs/claude-history-view/README.md 第 4.3/4.4 节（视图结构与行操作），再动手。配色全部引 theme/colors.ts token（硬约束 #6），禁止硬编码色值。`

// === Phase 1: 并行重构（双 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'row-components-agent',
    prompt: `你负责 FE-07 的展示组件部分（HistorySessionRow + InputDialog）与各自测试：

【src/features/claudeHistory/HistorySessionRow.tsx】
- props 接口逐字契约（见脚本头 HistorySessionRowProps）。
- 双行式行（README 4.3.3）：行1 = 粗体标题（title 为 null 时显示 sessionId 前 8 位）+ 右上角相对时间（调 Stage 04 的 formatRelativeTime，先 Read src/features/claudeHistory/historyModel.ts 确认导出签名）；行2 = firstPrompt 单行截断（CSS text-overflow: ellipsis + white-space: nowrap + overflow: hidden）。
- 状态标记：active=true → 标题前 ⚡；orphan=true → ✗（cwd 目录已删除）；noCwd=true 不显示 ✗（无 cwd 跳过孤儿判定）。
- 单击 → onSelect(session.sessionId)（选中高亮，背景色 EXPLORER_SELECTION_BG，src/theme/colors.ts token）；双击 → onDoubleClick(session)；右键 → onContextMenu(session, { x: e.clientX, y: e.clientY })。
- 根元素 data-e2e="agent-history-row"。
- 配色全部 theme/colors.ts token，零硬编码色值。

【src/features/claudeHistory/InputDialog.tsx】
- props 接口逐字契约（见脚本头 InputDialogProps）。
- 自绘模态：遮罩层 + 居中输入框（初始值 initialValue，挂载自动 focus + 全选）+ 确认/取消按钮；Enter → onSubmit(当前值 trim 后)；Escape / 点击遮罩 / 取消按钮 → onCancel()；空值禁止确认（按钮 disabled）。
- 根元素 data-e2e="agent-history-input-dialog"。
- 配色全部 token。

【测试（L2，src/__tests__/）】
- claude-history-row.test.tsx：渲染断言（标题/时间/prompt；title null → sessionId 前 8 位；⚡/✗ 标记三分支 active/orphan/noCwd）；单击 onSelect 参数；双击 onDoubleClick 参数；右键 onContextMenu 坐标；选中态样式。
- claude-history-input-dialog.test.tsx：初始值渲染；Enter 提交 trim 值；Escape/遮罩/取消按钮 onCancel；空值禁确认；自动 focus。

约束：不 import 任何 ipc 模块（展示组件纯受控）；不改 AgentStatusView.tsx / ClaudeHistorySections.tsx（归另一 agent）；不建 index.ts（barrel 归另一 agent）。`,
  },
  {
    label: 'view-integration-agent',
    prompt: `你负责 FE-07 的集成部分（Sections/List/菜单/barrel）+ FE-08 视图改造 + FE-09 空态 + FE-11 配色 + FE-12 data-e2e：

【src/features/claudeHistory/historyContextMenu.ts】
- getHistoryContextMenuItems 签名逐字契约（见脚本头；先 Read src/features/commit/commitContextMenu.ts 照其策略模式）。
- 操作矩阵（README 4.4 + stages.md）：
  - 复制恢复命令：全行可用；命令 = cwd 存在时 \`cd '<cwd>' && claude --resume <id>\`，cwd 为 null 时 \`claude --resume <id>\`；复制经 src/ipc/clipboard 的 writeText。
  - 分支恢复：orphan=true 或 noCwd=true → disabled；可用时 action 调 restoreHistorySession(session, { fork: true })（Stage 04 产物，先 Read 签名）。
  - 删除：active=true → disabled；可用时 action = dialog.ask 确认（照 commitContextMenu 先例）→ deleteHistorySession(sessionId) → 成功后调 opts 注入的 removeLocal（即时局部刷新）。
  - 重命名：全行可用；action 由调用方打开 InputDialog → 提交后 renameHistorySession(sessionId, newTitle) → 成功后 updateLocalTitle(id, newTitle)。
- 菜单项 label：「复制恢复命令」「分支恢复」「删除」「重命名」。

【src/features/claudeHistory/HistorySessionList.tsx】
- 当前项目区：平铺 HistorySessionRow 列表（无分组）。
- 全部项目区：二级折叠——组标题 = cwd basename（title 属性悬停完整路径；data-e2e="agent-history-group"），组可展开/收起；空组不显示；无 cwd 组标题「(未知目录)」。
- 搜索过滤经 matchesSearch（Stage 04 产物）作用于两区；分组经 groupByCwd。
- 双击分派（行内 onDoubleClick 消费方）：普通行 → restoreHistorySession(session)；orphan/noCwd 行 → 无操作；active 行 → dialog.ask「该会话已在运行中」引导分支恢复（确认 → fork 恢复）。
- 删除/重命名完成回调经 props 注入（removeLocal/updateLocalTitle 来自 useClaudeHistory）。
- 右键菜单渲染：ContextMenu 纯渲染组件（照 src/features/commit/CommitFileList.tsx 私有 ContextMenu 模式：position:fixed、zIndex、外点击关闭），根元素 data-e2e="agent-history-menu"。

【src/features/claudeHistory/ClaudeHistorySections.tsx】
- 组合件：搜索框（位于两个历史下拉框之上；data-e2e="agent-history-search"）+ 刷新按钮（data-e2e="agent-history-refresh"，点击调 scan()）+ 「当前项目历史会话」下拉框（data-e2e="agent-history-section-current"）+ 「全部项目历史会话」下拉框（data-e2e="agent-history-section-all"）。
- 消费 useClaudeHistory()（Stage 04 产物，先 Read 返回形状）；展开/收起 state 上提给 AgentStatusView 或本组件内部（照 stages.md 契约段——AgentStatusView 持有三区展开 state，本组件受控接收；先 Read stages.md Stage 05 组件契约逐字对齐）。
- 空态文案（FE-09）：当前项目区空 → 「该项目暂无历史会话」；全部项目区空 → 「暂无历史会话」；rootPath 为 null（无活跃项目）→ 当前项目区显示「无活跃项目」；搜索无结果 → 提示文案（如「无匹配的会话」）。
- 选中态 selectedId 由本组件持有。

【src/features/claudeHistory/index.ts】
- barrel export（照 commit/index.ts 模式）：导出 ClaudeHistorySections、useClaudeHistory、restoreHistorySession、HistorySessionRow、InputDialog、historyModel 纯函数、getHistoryContextMenuItems 等视图所需公共 API。

【FE-08 src/features/agentStatus/AgentStatusView.tsx 三下拉框改造】
- 先 Read 现状（活跃会话区 + useAgentStatus + AgentStatusRow）——活跃区逻辑零改动（import 与行渲染原样保留）。
- 改造为三个可展开/收起区块：「活跃会话」（默认展开）+ 「当前项目历史会话」（默认收起）+ 「全部项目历史会话」（默认收起）；历史区挂载 ClaudeHistorySections 的对应部分。
- 历史区首次展开时触发 scan()（仅首次；之后靠刷新按钮）。
- 整视图可滚动（overflow 样式照现状容器模式）。
- 【E2E 兼容红线，逐字保留】：根容器 data-e2e="agent-status-view"；活跃行 data-e2e="agent-status-row"（AgentStatusRow.tsx 不改）；标题栏 "AGENT STATUS" 文本；空态文案「选择一个项目」「无运行中的 claude 会话」原文保留。

【FE-11 配色 + FE-12 data-e2e】
- 本 Stage 全部新增/修改 UI 文件：颜色只引 theme/colors.ts token（选中高亮复用 EXPLORER_SELECTION_BG），grep 无硬编码色值。
- data-e2e 清单逐字落盘：agent-history-search / agent-history-refresh / agent-history-section-current / agent-history-section-all / agent-history-group / agent-history-row / agent-history-menu / agent-history-input-dialog（row 与 input-dialog 归另一 agent，集成处不重复设置）。

【测试（L2，src/__tests__/）】
- claude-history-view.test.tsx：渲染 ClaudeHistorySections（mock useClaudeHistory 或真实 hook + mock ../ipc/claudeHistory）——三区结构与默认态（活跃展开/历史收起）；展开历史区触发 scan()；搜索框输入过滤行；菜单可用性矩阵（普通行四操作可用 / orphan 行分支恢复禁用 / active 行删除禁用 / noCwd 行恢复与分支禁用）逐项断言；空态四文案；双击分派三分支（普通→restore / orphan→无操作 / active→ask）。
- agent-status-view.test.tsx（既有文件，先 Read）：同步更新适配三下拉框结构；保留对活跃区行为的断言（useAgentStatus mock 模式不变）；新增默认态与展开触发断言；确认 E2E 红线四件断言仍在（agent-status-view/agent-status-row/AGENT STATUS/空态文案）。

约束：不改 HistorySessionRow.tsx / InputDialog.tsx（归另一 agent）；不改 useAgentStatus.ts / AgentStatusRow.tsx / consts.ts；配色零硬编码。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 5 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
