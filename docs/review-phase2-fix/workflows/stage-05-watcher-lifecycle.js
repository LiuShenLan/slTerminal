// =====================================================================
// Stage 05：watcher 生命周期前端（BE-10、FE-38）
// 编排：单 agent（两项同改 Workspace.tsx 同一 SEC-01 effect）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-05.md
// 人工验证点：删末页/移除活跃项目后旧目录改动不再触发 fs-event
// =====================================================================

export const meta = {
  name: 'stage05-watcher-lifecycle',
  description: 'S05 空页面 stopWatch + effect await 串行（BE-10、FE-38）',
  phases: [
    { title: '重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 npm test（全量测试 agent 单点跑）；编译级自查用 \`npx tsc --noEmit\`。`

// === Phase 1: 重构（单 agent）===
phase('重构')
const refactorResults = await parallel(
  [{
    label: 'A-workspace-effect',
    prompt: `你负责【BE-10】空页面路径 stopWatch +【FE-38】SEC-01 effect await 成功后再 startWatch。先读 \`docs/review-phase2-fix/checklist.md\` 第 5 节 BE-10/FE-38 条目（各含可照抄代码块）——两项同改 \`src/workspace/Workspace.tsx\` 的 SEC-01 effect（约 :237-258）。

触碰文件：\`src/workspace/Workspace.tsx\`、\`src/__tests__/workspace-switch-order.test.tsx\`（或合适的新增用例位置）、\`src/workspace/CLAUDE.md\`

步骤：
1. 【BE-10】effect 首行 \`if (!activePageId) return;\` 改为 checklist BE-10 条目中的代码块：置 null 时对 \`prevRootRef.current\` 调 \`stopWatch\` 并清 ref（stopWatch wrapper 已存在于 \`src/ipc/notify.ts\`，import 补充）
2. 【FE-38】effect 内 \`setProjectRoot(...).catch(...)\` 与 \`void startWatch(...)\` 并排放火的两行（约 :248-253）改为 checklist FE-38 条目中的 promise 链：setProjectRoot 成功后（then 回调内）才 startWatch + 过期守卫（prevRootRef.current !== targetRoot 丢弃）+ catch 内保留 console.error + toast
3. 测试：补两个用例——①种子活跃项目触发 effect 后将 activePageId 置 null，断言 stopWatch 以旧 rootPath 被调用；②setProjectRoot resolve 前 startWatch 未被调用、reject 时 startWatch 不调用且 toast 出现
4. \`src/workspace/CLAUDE.md\`「文件监听跟随项目激活」段补「activePageId 置 null → stopWatch（BE-10）」
5. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + 新增用例名。`,
  }].map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 5 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
