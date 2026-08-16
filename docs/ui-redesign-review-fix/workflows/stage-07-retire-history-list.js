// =====================================================================
// Stage 07 退役组件删除（FE-25）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// 单项独立成 Stage 的豁免理由（stages.md 已注明）：跨多文件协同删除 + 测试迁移
//   为强耦合单一任务，拆分会制造中间态编译断裂
// =====================================================================

export const meta = {
  name: 'fix-stage07-retire-history-list',
  description: 'Stage 07 退役组件删除：HistorySessionList/Row 删除 + 测试迁移 NavHistoryRow + 注释同步',
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
  { label: "retire-components", prompt: `你负责 FE-25 主体：
【FE-25】删除退役组件（生产零消费方，仅测试引用）：
1. 删 src/features/agentHistory/HistorySessionList.tsx 与 HistorySessionRow.tsx
2. src/features/agentHistory/index.ts 删 :7,8,12,13 的 HistorySessionList/HistorySessionRow 导出（含类型导出）
3. src/__tests__/agent-history-row.test.tsx 迁移：仍有独立语义且未被 src/__tests__/nav-tree-history.test.tsx 覆盖的用例（四态同源/交互回调/选中态等）改写为 NavHistoryRow 面向（就近放入 nav-tree-history.test.tsx 或新建 nav-history-row 测试文件）；已覆盖语义删除
4. 孤儿 helper 清理（仅限本次删除所孤儿者）：grep 全仓确认 groupByCwd 等仅 HistorySessionList 消费的导出零引用（生产+测试均无）后才删；有引用保留不动
5. src/features/agentHistory/CLAUDE.md 不在本 Stage 动（归 Stage 09）` },
  { label: "comment-sync", prompt: `你负责 FE-25 注释同步（只动注释，不动任何逻辑）：
【FE-25 注释】以下文件中「照 HistorySessionList」类迁移注记改写为「原 HistorySessionList（已删）」口径或就近重写：
- src/lib/panelId.ts:8,69
- src/features/navTree/NavTree.tsx（多处「照 HistorySessionList」注释，含 :15,:100,:240,:330,:353,:373,:689 附近——先 grep 定位全部命中再逐处改写）
- src/features/navTree/useNavTree.ts:137
- src/features/navTree/NavHistoryRow.tsx:8
- src/features/navTree/NavContextMenu.tsx:54
- src/features/agentHistory/historyContextMenu.ts:11,13
注意：retire-components agent 同 Stage 并行删除组件文件，你只改上述注释文件，不碰 agentHistory/index.ts 与测试文件。` },
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-07.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
