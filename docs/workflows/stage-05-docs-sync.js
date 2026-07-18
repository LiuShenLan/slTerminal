// =====================================================================
// Stage 5 — 文档同步（SB-26，八处文档）
// =====================================================================
// 生成依据：docs/sidebar-checklist.md「Stage 5 — 文档同步」SB-26 原文
//           + docs/sidebar-execution-plan.md §7
// 前置：Stage 1~4 已 commit——全部代码与测试已落盘，文档必须反映最终状态。
// 单 agent：八处文档同一上下文，先读 git log/diff 再动笔，禁凭记忆写文档。
// 调用：Workflow({ scriptPath: 'docs/workflows/stage-05-docs-sync.js' })
// commit message（verify 全绿后）：
//   docs: 侧栏改造文档同步——sideViews 模块文档 + 各 CLAUDE.md + test-inventory
//
// fix-loop 调用参数（verify 不通过时）：
//   Workflow({ scriptPath: 'docs/workflows/fix-loop.js', args: {
//     stage: 5, failedItems, fixContext,
//     verifyFile: 'docs/workflows/verify/stage-05.md',
//     constraints: '本 Stage 只改文档（.md），禁止改任何代码/配置文件；文档内容必须对照真实代码与测试实际输出核实，禁凭记忆/照抄计划文档'
//   }})
// =====================================================================

export const meta = {
  name: 'stage5-docs-sync',
  description: 'Stage 5 文档同步：sideViews 模块文档 + 各 CLAUDE.md + test-inventory（SB-26，八处）',
  phases: [
    { title: '重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust（src-tauri/）一律不动。
背景：文档要求详见 docs/sidebar-checklist.md SB-26 条目（先读再动手）；术语以 CONTEXT.md「侧栏」节与 docs/sidebar-requirements.md §2 为准（禁止称"页面"/"面板"）。
本 Stage 特殊纪律：
1. 只改文档（.md），禁止改任何代码/配置文件。
2. 动笔前必须先读 git log -4 --stat 与四个 Stage 的实际 diff（git show --stat HEAD~3..HEAD 按需深入），再 Read 相关真实代码——禁凭记忆写文档，禁照抄计划文档。
3. 用例数必须以全量测试 agent 的 npm test / npm run wdio 实际输出为准（本脚本 Phase 3 结果——你在 Phase 1 拿不到，先搭好文档结构，test-inventory.md 的具体数字留待主 agent 在拿到测试结果后转告你补全，或你先执行 npx vitest run --reporter=verbose 统计「侧栏视图」相关 6 个测试文件的用例数）。
4. 文档规范遵循项目渐进式披露：根 .claude/CLAUDE.md 只加模块索引行，实现细节放 src/features/sideViews/CLAUDE.md。`

// === Phase 1: 重构（单 agent——八处文档同一上下文）===
phase('重构')
const refactorResults = await agent(`${PREAMBLE}\n\n你负责 SB-26（八处文档）。先读 docs/sidebar-checklist.md SB-26 条目、git log -4 --stat、四个 Stage 的实际 diff，再 Read 相关真实代码后动笔。

八处文件（逐条对应 checklist SB-26 原文）：
【SB-26-1】新建 src/features/sideViews/CLAUDE.md——职责、状态机单槽位模型（无历史记忆、展示=f(按钮归属,各区打开的视图) 纯推导）、SideViewRegistry 扩展指南（实现组件+一行 register，类比项目现有四个注册表）、HTML5 拖拽（computeDropTarget 落点判定+插入指示线）、关闭语义（display:none 保挂载+换区重建已知行为）、文件表、测试模式（参照其它模块 CLAUDE.md 风格）。
【SB-26-2】.claude/CLAUDE.md 模块表加 src/features/sideViews 行（职责：侧栏视图系统——活动栏+共享侧栏区+单槽位状态机；入口 src/features/sideViews/index.ts；详情 @../src/features/sideViews/CLAUDE.md）。
【SB-26-3】src/workspace/CLAUDE.md 布局段重写——Allotment 三栏改为：活动栏（40px 固定）+ 侧栏区（visible=anyOpen，内含垂直 Allotment 上下半区，SideBarArea 视图槽 display:none 保挂载）+ 主区；宽度/上下分割比例持久化（settings.json sideBar 段）；更新「关键集成点」中 SidebarTree/ExplorerPanel 的宿主描述。
【SB-26-4】src/stores/CLAUDE.md Store 清单加 sideBar.ts——状态形状（zones/open/width/splitRatio/loaded）、settings.json sideBar 段、默认态（DEFAULT_ZONES/DEFAULT_OPEN）、与 fontSize/keybindings 同持久化模式；测试文件表加 sideBar.test.ts 行。
【SB-26-5】src/features/sidebar/CLAUDE.md 补一句宿主变化——宿主从 Allotment 常驻栏变为侧栏区视图槽（经 sideViewDefs 注册为 projects 视图），组件本体不变。
【SB-26-6】src/features/explorer/CLAUDE.md 同⑤补宿主变化，并注明已知行为：按钮跨区拖拽导致视图跨 pane 移动时组件重建，文件树展开状态丢失（ADR-0001 已确认接受）。
【SB-26-7】e2e-tests/CLAUDE.md helpers 表加 3 个 helper（__slterm_e2e_getSideBarState / __slterm_e2e_toggleSideView / __slterm_e2e_moveSideViewButton）+ test.e2e.ts 用例清单加「侧栏视图」2 用例（12→14）。
【SB-26-8】.claude/test-inventory.md L2 新增「侧栏视图」类目——6 个测试文件（sideBarState.test.ts / sideViewRegistry.test.ts / sideBar.test.ts / activityBar.test.tsx / sideBarArea.test.tsx / workspace-sideviews.test.tsx），用例数以实际测试输出为准；L2 总数更新；L4 12→14。`, { label: 'docs-sync' })

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
补充：npm test 输出中请单独统计「侧栏视图」6 个测试文件（sideBarState / sideViewRegistry / sideBar / activityBar / sideBarArea / workspace-sideviews）的用例数小计与 L2 总数——供 test-inventory.md 核对。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/workflows/verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage5 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照真实代码抽查（防文档撒谎）：文档描述的签名/行为/用例数与当前代码与测试实际输出不一致即判 not_fixed。
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
