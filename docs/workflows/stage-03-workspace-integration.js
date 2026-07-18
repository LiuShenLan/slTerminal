// =====================================================================
// Stage 3 — Workspace 集成 + App 接线（SB-21~SB-24）
// =====================================================================
// 生成依据：docs/sidebar-checklist.md「Stage 3 — Workspace 集成 + App 接线」逐 ID 原文
//           + docs/sidebar-execution-plan.md §5（实现要点）
// 前置：Stage 1/2 已 commit——src/features/sideViews/（ActivityBar、SideBarArea、
//       sideViewDefs、index）与 src/stores/sideBar.ts 已存在。
// 单 agent：Workspace.tsx + App.tsx + 测试适配关联紧密，一个上下文完成。
// 调用：Workflow({ scriptPath: 'docs/workflows/stage-03-workspace-integration.js' })
// commit message（verify 全绿后）：
//   feat: Workspace 三栏改造——活动栏 + 可显隐侧栏区接入 Workspace 与 App 启动/关闭接线
//
// fix-loop 调用参数（verify 不通过时）：
//   Workflow({ scriptPath: 'docs/workflows/fix-loop.js', args: {
//     stage: 3, failedItems, fixContext,
//     verifyFile: 'docs/workflows/verify/stage-03.md',
//     constraints: 'switchToPage/onDeletePage/SEC-01/页面回调 map 逻辑不动；e2e-tests/ 本 Stage 禁碰；SidebarTree/ExplorerPanel 组件本体不改；存量测试只允许新增种子/mock 与修正断言，禁止 it.skip 删除用例'
//   }})
//
// === 人工验证点（commit 后、进 Stage 4 前，主 agent 必须提示用户实测）===
// npm run tauri dev 实测五条（详见 docs/workflows/verify/stage-03.md 尾部）：
// ① 默认项目列表打开；📋 关闭→侧栏区消失主区占满；再点恢复且项目树展开状态保留
// ② 📁 替换为文件浏览器；📁 拖到下区→上下分区；分隔条调比例
// ③ 📋 拖到下区（两视图打开中）→项目列表跟随到下区，文件浏览器被关闭
// ④ 重启 app→打开状态/宽度/比例/按钮归属全部恢复
// ⑤ 终端 claude 启动正常（主区零改动冒烟）
// =====================================================================

export const meta = {
  name: 'stage3-workspace-integration',
  description: 'Stage 3 Workspace 集成：三栏改造 + App 启动/关闭接线 + 存量测试适配 + 新增集成测试（SB-21~SB-24）',
  phases: [
    { title: '重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；主区 Dockview / SidebarTree / ExplorerPanel 组件本体 / 后端 Rust（src-tauri/）一律不动。
背景：修复要点详见 docs/sidebar-checklist.md 对应 ID 条目（先读再动手）；规则编号 R1~R9 见 docs/sidebar-requirements.md §4。
本 Stage 特殊纪律：
1. Workspace.tsx 中 switchToPage / onDeletePage / SEC-01 effect / 页面回调 map（pageCallbacksRef、ensurePageInitialized 等）逻辑全部不动——只改 Allotment 三栏结构与尺寸常量。
2. e2e-tests/ 目录本 Stage 禁碰（SB-25 属 Stage 4）。
3. 存量测试适配只允许新增种子/mock 与修正断言，禁止 it.skip / xit / 删除任何存量用例；适配判断以全量 npm test 红绿为准逐文件适配，不预判。
4. 禁止执行测试命令（npm test / npx vitest）——统一由全量测试 agent 单点跑；编译级自检可用 npx tsc --noEmit。
5. Stage 1/2 产物签名（useSideBar / deriveLayout / ACTIVITY_BAR_SIZE / WIDTH_MIN / WIDTH_MAX / ActivityBar / SideBarArea / sideViewDefs）以 src/ 真实代码为准，先 Read 再消费。`

// === Phase 1: 重构（单 agent——Workspace + App + 测试适配同一上下文）===
phase('重构')
const refactorResults = await agent(`${PREAMBLE}\n\n你负责 SB-21、SB-22、SB-23、SB-24。先读 docs/sidebar-checklist.md「Stage 3」与 docs/sidebar-execution-plan.md §5「实现要点」，再 Read src/workspace/Workspace.tsx、src/App.tsx、src/stores/sideBar.ts、src/features/sideViews/index.ts 后动手。

文件：
- src/workspace/Workspace.tsx（改）
- src/App.tsx（改）
- src/__tests__/workspace-multi-instance.test.tsx（按需适配）
- src/__tests__/workspace-switch-order.test.tsx（按需适配）
- src/__tests__/workspace-e2e-ready.test.tsx（预期零改动，以 npm test 为准）
- src/__tests__/e2e-gating-workspace.test.tsx（预期零改动，以 npm test 为准）
- 其余因 Workspace 结构变红的套件（以全量 npm test 为准一并适配）
- src/__tests__/workspace-sideviews.test.tsx（新建）

【SB-21】Workspace.tsx 三栏改造：
- pane1：preferredSize={ACTIVITY_BAR_SIZE}（40）minSize={40} maxSize={40} → <ActivityBar />（import 自 ../features/sideViews）。
- pane2：preferredSize={width}（width 来自 useSideBar）minSize={WIDTH_MIN}（160）maxSize={WIDTH_MAX}（500）visible={anyOpen} → <SideBarArea switchToPage={switchToPage} onDeletePage={onDeletePage} />。
- pane3 主区不变。
- anyOpen = deriveLayout(open) !== "hidden"（open 来自 useSideBar 订阅）；外层 Allotment onChange={(sizes) => anyOpen && setWidth(sizes[1])}（clamp 由 store 保证）。
- 删除旧六常量 SIDEBAR_PREFERRED_SIZE/SIDEBAR_MIN_SIZE/SIDEBAR_MAX_SIZE/EXPLORER_PREFERRED_SIZE/EXPLORER_MIN_SIZE/EXPLORER_MAX_SIZE（当前 36-41 行），尺寸常量一律 import 自 ../features/sideViews（ACTIVITY_BAR_SIZE/WIDTH_MIN/WIDTH_MAX）。
- 顶部加 import "../features/sideViews/sideViewDefs"（side-effect 注册——静态 import 链保证 App init 的 loadFromDisk 运行时注册已完成）。
- switchToPage/onDeletePage/SEC-01/回调 map 逻辑全部不动。
【SB-22】App.tsx 接线：
- init() 在 useKeybindings.getState().loadFromDisk() 之后、loadAllProjects() 之前插入独立 try/catch 块：await useSideBar.getState().loadFromDisk()（catch 保默认，注释对齐现有两块风格）。
- 关窗冲刷在 cancelKeybindingsSave() 之后加 cancelSideBarSave()；import { cancelPendingSave as cancelSideBarSave } from "./stores/sideBar"（useSideBar 同处引入）。
【SB-23】现有测试适配：
- workspace-multi-instance.test.tsx / workspace-switch-order.test.tsx 真实渲染 Workspace → 需种子 useSideBar.setState({ ...默认值, loaded: true })（beforeEach）或 mock ../features/sideViews 模块导出 stub 组件。
- workspace-e2e-ready.test.tsx / e2e-gating-workspace.test.tsx 的 MockAllotment（Pane=null）对 pane 数量不敏感，预期零改动。
- 以全量 npm test 红绿为准逐文件适配，不预判；只允许新增种子/mock 与修正断言，禁止 it.skip/删除用例。
【SB-24】workspace-sideviews.test.tsx（新建，~8 用例）：Workspace 集成——活动栏 pane 40 固定（preferredSize/min/max）、侧栏区 pane visible=anyOpen 四态（hidden→false、单开上/下→true、双开→true）、外层 onChange→setWidth、主区 pane 保持、switchToPage 透传 SideBarArea。mock allotment 捕获 Pane props + 真实 sideBar store 种子。`, { label: 'workspace-integration' })

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx vite build
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/workflows/verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage3 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
