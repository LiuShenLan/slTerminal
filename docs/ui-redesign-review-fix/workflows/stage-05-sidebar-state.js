// =====================================================================
// Stage 05 侧栏与活动栏（FE-17/19/22/23）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// =====================================================================

export const meta = {
  name: 'fix-stage05-sidebar-state',
  description: 'Stage 05 侧栏与活动栏：splitRatio 双开保留 + reconcileZones 纯函数化 + ActivityBar 动效/指示线修正',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/ui-redesign-review-fix/checklist.md 对应 ID 条目（先读再动手）。
并行纪律：不跑资源共享型测试——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  { label: "activitybar-fix", prompt: `你负责 FE-17/23：
【FE-17】src/features/sideViews/ActivityBar.tsx:65 删 transition: background-color 0.15s（UI-110 硬约束无动效）。
【FE-23】同文件 :226-231 onDragLeave 误清指示线：改 relatedTarget 判断（e.currentTarget.contains(e.relatedTarget) 为 true 时不清——容器→子元素转移视为未离开），或统一在 dragend/drop 清理；取最小改动。
测试同步：src/__tests__/activityBar.test.tsx 增「style 不含 transition」负断言 + 「容器→子元素转移不清指示线」用例。` },
  { label: "sidebar-ratio", prompt: `你负责 FE-19：
【FE-19】src/features/sideViews/SideBarArea.tsx:58-63 双开时 useEffect 无条件 setSplitRatio(0.5)：改仅「首次进入双视图（无持久化值）」或「值越界（出 [0.1,0.9]）」时回退默认；正常单↔双切换保留用户调节值。effect 条件收窄，勿动 store 持久化链。
测试同步：src/__tests__/sideBarArea.test.tsx 增「双开→拖比例→单开→再双开，比例保留」用例。` },
  { label: "reconcile-pure", prompt: `你负责 FE-22：
【FE-22】src/features/sideViews/sideBarState.ts:184,190-194 reconcileZones 对入参 saved.top filter 结果直接 push：push 前先 [...top] 复制（bottom 同模式自查），保纯函数语义（不 mutate 调用方传入的持久化对象）。
测试同步：src/__tests__/sideBarState.test.ts 增「入参数组不被 mutate」断言（调用前后快照比对）。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
