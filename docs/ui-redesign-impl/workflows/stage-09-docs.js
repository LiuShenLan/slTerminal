// =====================================================================
// Stage 09 — 文档同步（DOC-01~DOC-06）
// fix-loop 调用本 Stage 时 args.constraints 传：
//   「文档描述须对照当前代码核实，禁止凭记忆写文档」
// =====================================================================

export const meta = {
  name: 'stage09-docs',
  description: 'Stage 09: 实施期文档同步（CLAUDE.md/CONTEXT/ADR/测试清单）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。
【本 Stage 特殊纪律】文档描述须对照当前代码核实，禁止凭记忆写文档；遵循渐进式披露（根文件只登记，细节归子路径）。`

// === Phase 1: 并行重构（文档间零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "docs-src", prompt: `你负责 DOC-01（src 各子路径 CLAUDE.md 同步，全部对照当前代码核实后改写）：
- src/theme/CLAUDE.md：darcula→linear（方案文件/默认 id/回退语义）、types.ts 新槽位（syntax 9 键/plainText 3 键/ui 3 新标量）、facade 34 导出、editorSyntaxHighlight 导出与 ACC-05 注入顺序、fail-safe 新值 #0a0a0b、测试模式用例更新
- src/features/sideViews/CLAUDE.md：三槽（nav/explorer/commit）+ 底部配置钮（不入注册表）、agent-status 退役、DEFAULT_ZONES、46px
- src/features/sidebar/CLAUDE.md：SidebarTree 已被 navTree 取代——本目录状态说明（目录删除则本文件一并删除，执行期确认 src/features/sidebar/ 残留内容）
- src/features/agentStatus/CLAUDE.md：AgentStatusView/AgentStatusRow 删除、useAgentStatus 保留为导航树数据源
- src/features/agentHistory/CLAUDE.md：迁移导航树（行单行化/title tooltip/HistorySessionList 现状）
- src/workspace/CLAUDE.md：DefaultTab 形态（扁平/指示条/hover 关闭/StatusDot/FileIcon/「+」22px）、Watermark 空态规范
- src/lib/CLAUDE.md：icons.tsx/StatusDot/ConfirmDialog/toast 新组件登记
- src/panels/CLAUDE.md：页签 tabStatus 链路（tabIcon 已退役）
- src/ipc/CLAUDE.md：dialog ask 删除（open/save 保留）、window 三 wrapper 登记
- src/features/explorer/CLAUDE.md：darcula 字样与 FileIcon 描述（emoji→六色盘 SVG）同步
- 兜底：grep -ri "darcula" src/ --include=CLAUDE.md 命中的其它文件一并同步（panels/theme/explorer 之外如有残留）
- 新建 src/features/navTree/CLAUDE.md 与 src/features/titleBar/CLAUDE.md（照子文件模板：职责→架构决策（关键约束）→文件表→测试模式）` },
  { label: "docs-root", prompt: `你负责 DOC-02/DOC-04/DOC-05/DOC-06：
【DOC-02】CONTEXT.md：UI 重设计节核实术语（明度阶梯/发丝线/统一导航树/状态圆点/双轨配色与实现一致）；「Agent Status 视图」条标注已退役（2026-08 实现期并入统一导航树）；活动栏节补「配置」钮（hooks 配置面板唯一入口）。
【DOC-04】docs/ui-redesign/requirements.md：UI-405/406/407 三条补「远期愿景，本期不实施（2026-08-16 决策：聊天式 Agent 面板为独立产品方向，未来单独立项）」注记（改表格或行内批注，不动编号）。
【DOC-05】.claude/adr.md：ADR-0003 追加「实现期决策（2026-08）」小节——Agent 面板三条剔除/darcula 删除 linear 替换/配置钮入口唯一化/导航树挂法（会话挂页面、历史挂项目）/lucide-react 与 @fontsource/jetbrains-mono 两新依赖/自绘标题栏取舍（失 Snap Layouts）。
【DOC-06】根 .claude/CLAUDE.md：模块索引表——新增 src/features/navTree 与 src/features/titleBar 行、sidebar/agentStatus/agentHistory/sideViews/lib/ipc/workspace 行职责更新（对照当前代码）；编号索引核实（无需新增编号则不增）。` },
  { label: "docs-test", prompt: `你负责 DOC-03：.claude/test-inventory.md 全量同步——本次实施新增用例（scheme 色值/emoji-scan/title-bar/nav-tree/confirm-dialog/toast 等）、修改用例（colors/scheme-registry/overrides/activityBar/sideBar 等改造）、删除用例（AgentStatusView/AgentStatusRow/AgentHistorySections 专属与被清理的 E2E 用例）逐条登记；对照 src/__tests__/ 与 e2e-tests/ 当前文件实查核实，禁止凭记忆。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
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

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 09 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
