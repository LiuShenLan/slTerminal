// =====================================================================
// Stage 06 — 侧栏 IA 重构（NAV-01~NAV-11）——最大改造 Stage，含串行块
// 跨边界契约（写死，全部 agent 与 E2E 共用）：
//   - 导航树视图 id = "nav"（sideViewDefs 注册）；explorer/commit 视图 id 不变
//   - 配置钮 id = "config"（ActivityBar 底部独立按钮，不入 SideViewRegistry，
//     点击 = 打开 hooksConfig 面板——复用 SidebarTree 现「打开 Hooks 配置」逻辑提取的公共函数）
//   - data-e2e 选择器：nav-tree / nav-row-project / nav-row-page / nav-row-session /
//     nav-history-node / activity-btn-config；活动栏按钮选择器 activity-btn-<id> 沿用
//   - 旧选择器 agent-status-view / agent-status-row / "AGENT STATUS" 文案随本 Stage 废除
//   - 会话行归属：活跃会话经 panelId→pageId 挂页面下；历史会话经 cwd 前缀匹配挂项目下
// fix-loop 调用本 Stage 时 args.constraints 传：
//   「E2E 重写必须实跑 npm run e2e 验证，禁止仅改不跑」
// =====================================================================

export const meta = {
  name: 'stage06-sidebar',
  description: 'Stage 06: 统一导航树 IA（项目→页面→会话）+ 活动栏三槽 + 配置钮入口唯一化',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。
【本 Stage 特殊纪律】E2E 重写必须实跑 npm run e2e 验证，禁止仅改不跑。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "navtree-new", prompt: `你负责 NAV-01/NAV-02/NAV-03/NAV-04/NAV-09（全部新文件落于 src/features/navTree/ 新建目录，只读引用既有数据层，禁止改动 navTree/ 之外的任何文件）：
【NAV-01】NavTree.tsx + 行组件（UI-303/501/502/503）：层级恰为 项目→页面→会话 三级；行结构 = chevron 12px fg-3（icons.tsx IconChevronRight/Down）+ 图标 + 名称 + 右侧 11px fg-4 元数据；行高 28px（会话行 30px）、圆角 5px、hover #222227（SIDEBAR_COLORS.hover token）；选中行 rgba(110,159,242,0.13) 底（ACTIVE_SELECTION_BG token；hover 时 SELECTION_HOVER_BG token）+ fg-1 文字；每级左缩 15px + 1px 发丝引导线（SIDEBAR_COLORS.treeGuide token）；展开/折叠状态组件内维护。
【NAV-02】活跃会话行（UI-504/109）：StatusDot + CLI logo 14px（cliProfileRegistry iconSrc——照 AgentStatusRow 现逻辑）+ 标题 + 右侧（32x3 迷你用量条 + 百分比 11px fg-4）；数据源 = useAgentStatus()（Read src/features/agentStatus/useAgentStatus.ts 确认 rows 形态）；归属 = 行 panelId → 页面前缀解析（Read src/panels/terminal/TerminalRegistry 与 B14 的 parseTerminalPageId 确认解析函数）；点击行聚焦对应终端页签（照 AgentStatusRow/现跳转逻辑——Read 确认后原样迁移）；用量阈值 ≥90/≥70/≥50 逻辑与 computeUsagePercent 委托不变。
【NAV-03】历史会话折叠节点挂项目下（UI-303/505）：IconHistory 时钟 + 「历史」+ 计数 pill（#1a1a1e 底 SIDEBAR_BG token、fg-4 PLACEHOLDER_FG）；展开 = 历史行（StatusDot+logo+标题+右侧相对时间，单行 30px）；prompt 预览改原生 title tooltip；数据源 = agentHistory scan（Read src/features/agentHistory/ 确认 useAgentHistory/scan 调用形态，cwd 前缀匹配项目 rootPath 归属）；双击恢复 + 右键菜单（复制恢复命令/分支恢复/删除）沿用 historyContextMenu 策略（Read 确认导出形态直接引用）；空历史显示空态（15px 线性图标 fg-4 + 说明 fg-3）。
【NAV-04】搜索框（UI-506）：顶部「导航」分组标题（11px 全大写 0.08em fg-3）+ 刷新钮（IconRefresh）下置搜索框——#1a1a1e 底（INPUT_BG token）圆角 5px、12px、占位「搜索项目 / 页面 / 会话…」（fg-4）、focus 描边 FOCUS_BORDER；子串不区分大小写过滤项目/页面/会话名，父节点因子命中而显示，无结果显示空态。
【NAV-09】项目行（UI-505）：500 字重 fg-1 + 彩色文件夹图标（icons.tsx IconFolder，accentFg 或六色盘蓝——执行期定写注释）；当前活跃项目带「当前」pill（accent-dim 底 ACTIVE_SELECTION_BG、#8fb4f5 字 ACCENT_FG、10px）；右侧页面计数 pill（#1a1a1e 底 fg-4）。
数据接入：项目/页面树数据照 SidebarTree 现 store 订阅（Read src/stores/projects 确认）；项目/页面右键菜单承接 SidebarTree 现菜单项但删除「打开 Hooks 配置」项（UI-802 视觉规范：项 28px、圆角 5px、hover #222227、危险项 ERROR_FG）；内联重命名与 CRUD 逻辑照 SidebarTree 迁移（Read SidebarTree.tsx 全文，行为不变）；新建页面空布局 makeEmptyLayout 逻辑迁移。
测试数据属性：容器 data-e2e=nav-tree，行 data-e2e=nav-row-project/nav-row-page/nav-row-session/nav-history-node（契约写死）。` },
  { label: "history-migrate", prompt: `你负责 NAV-08 前半（历史组件单行化改造，供导航树复用）：
改造 src/features/agentHistory/HistorySessionRow.tsx：双行式改单行 30px——行 = StatusDot + CLI logo 14px + 标题 + 右侧相对时间 11px fg-4；prompt 预览第二行删除，预览文本改放行容器原生 title 属性；✗ 孤儿标记保留（IconClose 12px ERROR_FG，Stage 03 已改则不动）；行 props 签名保持兼容（HistorySessionList 调用零改动）。
改造 src/features/agentHistory/HistorySessionList.tsx：组标题箭头确认 chevron 化（Stage 03 已改则不动）；其余结构不动（搜索框/刷新/两区块逻辑保留——Stage 后续由 sidebar-switch 决定整体去留，本 agent 只做行单行化适配）。
同步更新 src/__tests__/ 中 HistorySessionRow/List 相关测试（双行断言改单行+title tooltip 断言）。` },
  { label: "navtree-test", prompt: `你负责 NAV-10 的 L2 部分（只新建测试）：
新建 src/__tests__/nav-tree.test.tsx（按需拆分多个文件）：以 stages.md Stage 06 实现要点与脚本头契约为规格写 L2 测试——三级层级渲染（项目→页面→会话）、行高规格、选中态 token、活跃会话行构成（StatusDot/logo/迷你用量条/百分比）、历史折叠节点（计数 pill/展开/项目归属）、搜索过滤（命中/未命中/父节点因子的显示）、「当前」pill、右键菜单无「打开 Hooks 配置」项、data-e2e 选择器齐备。mock：useAgentStatus、agentHistory scan、stores/projects（照 src/__tests__/ 既有种子模式——先 Read sideBar.test.ts/activityBar.test.tsx 学习既有 mock 形态）。若 navtree-new agent 产出尚未就绪，按契约与 stages 要点写，禁止改动 navTree 源文件。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（共享文件依赖，前序 agent 的产出供后序使用）===
phase('串行重构')
const sequentialAgents = [
  { label: "sidebar-switch", prompt: `你负责 NAV-05/NAV-06/NAV-07/NAV-08 后半（前序 navtree-new 已产出 src/features/navTree/，先 Read 其导出形态再动手）：
【NAV-05】src/features/sideViews/sideViewDefs.ts 重组三槽：注册 nav（导航树 NavTree 组件，title 导航树，icon IconNav）、explorer、commit（icon 均 Stage 03 已定组件形态）；删除 agent-status 注册。src/features/sideViews/sideBarState.ts：DEFAULT_ZONES 改 [nav, explorer, commit]；ACTIVITY_BAR_SIZE 40→46。src/features/sideViews/ActivityBar.tsx：宽 46px、按钮 34x34 圆角 6px、激活态 = accent-dim 底（ACTIVE_SELECTION_BG token）+ accent-fg 图标（ACCENT_FG token）+ 左侧 2px accent 竖条（FOCUS_BORDER token，沿用现指示条机制）；底部新增配置钮（id config、IconConfig、data-e2e=activity-btn-config、点击 = 打开 hooksConfig 面板——先 Read src/features/sidebar/SidebarTree.tsx 现「打开 Hooks 配置」实现，提取为公共函数放 src/features/hooksConfig/ 或 src/lib/（执行期定），SidebarTree 删除前完成提取）；配置钮不入 SideViewRegistry、不参与拖拽/持久化。
【NAV-06】删除 src/features/sidebar/SidebarTree.tsx（NavTree 已承接全部行为）；grep 全仓 SidebarTree 引用逐处清理（src/features/sidebar/index.ts barrel、Workspace、测试等）；grep 右键菜单「打开 Hooks 配置」零残留（菜单项已随 SidebarTree 删除、配置钮入口唯一）。
【NAV-07】src/stores/sideBar.ts 持久化迁移：恢复时 zones/open 中未注册 id（projects/agent-status 等）丢弃回退默认——先 Read reconcileZones/sanitizeSideBar 确认过滤语义覆盖 open 字段（R9 已有 zones 过滤；open 指向未知 id 须置 null）；新增/更新对应 L2 用例。
【NAV-08 后半】删除 src/features/agentStatus/AgentStatusView.tsx、AgentStatusRow.tsx、src/features/agentHistory/AgentHistorySections.tsx；useAgentStatus.ts 保留（导航树数据源）；grep 全仓被删组件引用逐处清理（sideViewDefs、index.ts barrel、Workspace、测试）；src/__tests__/ 中 AgentStatusView/AgentStatusRow/AgentHistorySections 专属测试文件删除或迁移至 nav-tree 测试（行为等价用例保留，视图结构用例删除——逐文件判断写理由）。` },
  { label: "e2e-rewrite", prompt: `你负责 NAV-10 的 E2E 部分（前序 sidebar-switch 已完成，新视图 id/选择器已生效）：
重写 e2e-tests/sidebar.e2e.ts、agent.e2e.ts、mockcli.e2e.ts、helpers.ts：
- agent-status 视图相关用例改 nav 视图（选择器 activity-btn-nav、nav-tree、nav-row-session 等脚本头契约）；活动栏序位断言 projects(0)/explorer(1)/commit(2)/agent-status(3) 改 nav(0)/explorer(1)/commit(2)；
- agent.e2e.ts：打开 agent-status 视图逻辑改打开 nav；agent-status-row 出现/消失等待改 nav-row-session；⚡ emoji 文本断言改圆点存在性断言（DOM 结构——Read StatusDot 确认渲染标签与测试属性，无测试属性则断言圆点元素存在）；AGENT STATUS 标题断言改导航标题；
- mockcli.e2e.ts:170-216：__slterm_e2e_toggleSideView("agent-status") 改 "nav"；行断言同上；
- helpers.ts：toggleSideView helper 参数与相关断言同步（该文件在根 tsconfig include 外——改完必须经 npx vite build 构建级验证）；
- 完成后必须实跑 npm run e2e（= build:e2e + wdio）验证全部通过，禁止仅改不跑。` },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run test:l3
7. npx vite build
8. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。npm run e2e 耗时长（含构建），属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
