// =====================================================================
// Stage 02：依赖升级（TE-06、TE-07、TE-14）
// 编排：pipeline 串行 A(dialog) → B(typescript) → C(dedupe)（三项同改 package.json/lock）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-02.md
// 人工验证点：本 Stage 完成后真实产物冒烟（终端/编辑器/hooks 面板各开一次）
// =====================================================================

export const meta = {
  name: 'stage02-deps',
  description: 'S02 依赖升级：dialog 2.7.2 + typescript ^7.0.2 主字段直改 + WDIO dedupe（TE-06/07/14）',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确步骤与 fallback 决策树）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'A-dialog',
    prompt: `你负责【TE-06】@tauri-apps/plugin-dialog npm 升 2.7.2。先读 \`docs/review-phase2-fix/checklist.md\` 第 2 节 TE-06 条目。

步骤：
1. \`package.json\` 第 39 行 \`"@tauri-apps/plugin-dialog": "2.7.1"\` 改为 \`"2.7.2"\`
2. \`npm install\` 刷新 package-lock.json
3. \`npm ls @tauri-apps/plugin-dialog\` 确认单版本 2.7.2（无多实例）

完成后报告：npm ls 输出 + lock 变更摘要。`,
  },
  {
    label: 'B-typescript',
    prompt: `你负责【TE-07】TypeScript 主字段直改 ^7.0.2（D14 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 2 节 TE-07 条目（含完整 fallback 决策树）。

步骤（严格按序，禁跳步）：
1. 实查兼容（禁凭印象）：\`npm view typescript-eslint version\` 与 \`npm view typescript-eslint@<查得的最新版> peerDependencies\`——确认其支持的 TypeScript 版本范围含 7.x，记录结论
2. 确认别名零消费：grep 全仓 \`@typescript/native\`（除 package.json/package-lock.json 外应零命中——若有引用先报告并暂停）
3. \`package.json\` 第 80 行 \`"typescript": "npm:@typescript/typescript6@^6.0.2"\` 改为 \`"typescript": "^7.0.2"\`；删除第 59 行 \`"@typescript/native": "npm:typescript@^7.0.2"\` 整行
4. \`npm install\` 刷 lock；\`npm ls typescript\` 确认单实例 7.x（无 typescript6 残留）
5. 即时验证：\`npx tsc --noEmit\` → \`npx eslint src/\`（npm test 由全量测试 agent 跑）
6. 若 eslint 报 TS 版本不兼容：升级 \`typescript-eslint\` 至步骤 1 查实的兼容版；仍不行则 package.json \`overrides\` 钉兼容组合——任何妥协方案在报告中明确记录（供 S10 写 ADR）

完成后报告：步骤 1 兼容结论原文 + npm ls typescript 输出 + tsc/eslint 结果 + 妥协方案（若有）。`,
  },
  {
    label: 'C-dedupe',
    prompt: `你负责【TE-14】WDIO 工具链同包多版本收敛。先读 \`docs/review-phase2-fix/checklist.md\` 第 2 节 TE-14 条目。

步骤：
1. \`npm ls @wdio/globals expect-webdriverio webdriverio\` 记录多版本现状
2. \`npm dedupe\` 后重跑步骤 1——收敛（各单版本）则完成
3. 未收敛：\`package.json\` 的 \`overrides\`（现有 4 项）追加 \`"@wdio/globals": "^9.31.0"\`、\`"expect-webdriverio": "^6.0.5"\`、\`"webdriverio": "^9.30.1"\` → \`npm install\` → 重跑步骤 1 确认各单版本
4. \`npm run build:e2e\` 构建通过（E2E helper 链路编译验证）

完成后报告：收敛方式（dedupe / overrides）+ 最终 npm ls 输出 + build:e2e 退出码。`,
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
逐项检查 Stage 2 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read/命令实跑逐条核实并给出证据。
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
