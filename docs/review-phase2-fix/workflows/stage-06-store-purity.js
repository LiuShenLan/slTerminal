// =====================================================================
// Stage 06：store 纯状态 + 页面切换链（FE-37、FE-36、BE-23）
// 编排：pipeline 串行 A(上提+toast) → B(全局计数)（A/B 同改 projects.ts）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-06.md
// 人工验证点：页面切换无回归 + setProjectRoot 失败 toast 实测
// =====================================================================

export const meta = {
  name: 'stage06-store-purity',
  description: 'S06 switchToPage IPC 上提 + MAX_PAGES 全局化 + 切换失败 toast（FE-37/36、BE-23）',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 npm test（全量测试 agent 单点跑）；编译级自查用 \`npx tsc --noEmit\`。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'A-lift-toast',
    prompt: `你负责【FE-37】switchToPage 剥离 IPC 上提调用方（D18 决策）+【BE-23】switchToPageShared setProjectRoot 失败 toast。先读 \`docs/review-phase2-fix/checklist.md\` 第 6 节 FE-37/BE-23 条目（各含可照抄代码块）。

触碰文件：\`src/stores/projects.ts\`、\`src/workspace/pageApis.ts\`、直调 store switchToPage 的测试文件（已知：\`src/__tests__/layout-switch.test.ts\`、\`src/__tests__/projects.test.ts\`、\`src/__tests__/workspace-multi-instance.test.tsx\` 等——grep 实查补全）、\`src/__tests__/pageapis.test.ts\`、\`src/stores/CLAUDE.md\`

【FE-37】步骤：
1. \`src/stores/projects.ts\` switchToPage（约 :159-186）：删除 SEC-01 setProjectRoot 块（约 :163-170，含注释），switchToPage 变纯 set 状态转换
2. 检查 \`setProjectRoot\` import（约 :8）——本文件其他位置无消费则删除该 import（grep 文件内确认；toast import 保留——其他位置仍有消费）
3. 生产调用点零改动确认：grep 全仓 \`.switchToPage(\`——生产代码仅 Workspace.tsx（委托 switchToPageShared）与 NavTree.tsx（缺省回退 switchToPageShared），均天然含 setProjectRoot，无需改
4. 测试适配：直接调 store switchToPage 的测试文件不再期待 setProjectRoot 被调用；断言「切换后 setProjectRoot 调用」的用例改为经 switchToPageShared 驱动或删除该断言

【BE-23】步骤：
5. \`src/workspace/pageApis.ts\` switchToPageShared catch 块（约 :60-62）按 checklist 条目补 \`toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝")\`；文件头加 \`import { toast } from "../lib";\`
6. \`src/__tests__/pageapis.test.ts\` 增用例：setProjectRoot mock reject → 断言 toast.show 以 warning 调用且切换仍完成

7. \`src/stores/CLAUDE.md\` projects.ts 节补「switchToPage 为纯状态转换（FE-37：setProjectRoot 已上提调用方 switchToPageShared，约束 #12 合规）」
8. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + 适配的测试文件清单 + 新增用例名。`,
  },
  {
    label: 'B-global-count',
    prompt: `你负责【FE-36】MAX_PAGES 全局总数校验。先读 \`docs/review-phase2-fix/checklist.md\` 第 6 节 FE-36 条目（含可照抄代码块）。前序 agent 已完成 FE-37 剥离，你在其产出上继续。

触碰文件：\`src/stores/projects.ts\`、\`src/__tests__/projects.test.ts\`、\`src/stores/CLAUDE.md\`

步骤：
1. \`src/stores/projects.ts\` addPage（约 :107-115）：\`if (project.pages.length >= MAX_PAGES)\` 改为 checklist 条目中的全局计数代码块（\`Object.values(get().projects).flatMap((p) => p.pages).length >= MAX_PAGES\`）；附近 FE-01 注释同步补「FE-36 全局化」一句
2. \`src/__tests__/projects.test.ts\` MAX_PAGES 用例（约 :356-387）适配为全局构造；新增跨项目用例：项目 A 15 页 + 项目 B 5 页时，B addPage 拒绝 + toast
3. \`src/stores/CLAUDE.md\`「页面总数上限（FE-01）」段补「跨项目全局计数（FE-36）」
4. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + 新增用例名。`,
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

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
逐项检查 Stage 6 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { sequentialResults, testResult, verifyResult }
