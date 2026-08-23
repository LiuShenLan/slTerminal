// Stage 09：CI 门禁（TQ-CI-03, TQ-CI-06, TQ-CI-07, TQ-E-09）——单 agent
// fix-loop 调用时 args.constraints 传空
export const meta = {
  name: 'stage-09-ci',
  description: 'Stage 09：CI 门禁补全——rustfmt/timeout/npm 缓存/E2E flakiness 观察面（4 项）',
  phases: [
    { title: '修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。`

phase('修复')
const fixResult = await agent(`${PREAMBLE}

你负责 TQ-CI-03 + TQ-CI-06 + TQ-CI-07 + TQ-E-09（全部 CI 改动）：
1. .github/workflows/ci.yml：插入 cargo fmt --check 步骤（先本地跑 cargo fmt --manifest-path src-tauri/Cargo.toml -- --check 确认当前过——不过先 cargo fmt 修齐并报告，由主 agent 决定单独 commit）；各 job 补 timeout-minutes；setup-node 补 npm cache；E2E 构建步骤的 VITE_E2E=1 保持不动。
2. e2e-tests/wdio.conf.ts：retries 1 → 0（flakiness 观察面，checklist TQ-E-09）。
3. yaml 改动后用 node 解析校验语法（node -e "const fs=require('fs');const yaml=require('yaml');yaml.parse(fs.readFileSync('.github/workflows/ci.yml','utf8'))"——yaml 包不可用时报 npx js-yaml 等替代方案的实际使用结果）。
触碰：.github/workflows/ci.yml, e2e-tests/wdio.conf.ts。`, { label: 'fix-ci' })

phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证：
1. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check（本地 fmt 门禁前置）
2. ci.yml 语法解析校验（node + yaml/js-yaml 解析，报告解析结果）
3. npm run e2e（WDIO_RETRIES 改动冒烟——全量跑，耗时长勿中止）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 09 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { fixResult, testResult, verifyResult }
