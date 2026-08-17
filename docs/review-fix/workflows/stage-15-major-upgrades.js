// =====================================================================
// stage-15-major-upgrades.js — S15 major 升级（TE-07/08/09/10）
// =====================================================================
// 串行理由：四步均改 package.json + lock，pipeline 严格串行；按风险升序，
//   dockview-react 8 最高风险最后。
// 特殊纪律（覆盖通用测试纪律）：本 Stage 每个串行步骤完成后必须在步骤内跑
//   「npx tsc --noEmit + npx eslint src/ + npm test + npm run test:l3」门禁——
//   依赖升级的兼容性只有测试能暴露（jsdom 行为变更等），编译级检查不够；
//   四步严格串行无并发，不存在共享资源冲突。步骤红则修复，无法修复回滚该步
//   并在报告中注明（后续步骤基于绿态继续）。
// commit：单条（workflow 原子执行，逐步 commit 不可行；逐步门禁已保证中间态绿）。
// fix-loop 调用约定：args.testCommands 传本脚本下方 8 条门禁；
//   args.constraints 传「依赖升级步骤红则修复，无法修复回滚该步」。
// =====================================================================

export const meta = {
  name: 'stage15-major-upgrades',
  description: 'Stage 15: jsdom 30/typescript 7/json-schema-library 11/dockview-react 8 四步串行 major 升级（TE-07/08/09/10）',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入；H6 多 Dockview 实例架构与 layoutSerde 单点（约束 #7）不得因 dockview 升级改变。
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S15 节（先读再动手）。
本 Stage 特殊测试纪律：你的步骤完成后必须依次跑 npx tsc --noEmit、npx eslint src/、npm test、npm run test:l3 并全绿才交付；红则修复，无法修复则回滚你这一步的全部改动并在报告注明（严禁把红态留给下一步）。
版本风格：本 Stage 版本号保持现状风格（^/精确照原行），统一策略在 S16 收敛。`

// === Phase 1: 并行重构（本 Stage 仅步骤 A；后续步骤见串行块）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'step-a-test-toolchain',
    prompt: `你负责 TE-10（步骤 A）：jsdom 29→30、@testing-library/jest-dom 6→7、@types/node 25→26、cross-env 7→10，只许改 package.json、package-lock.json + L2 测试适配文件（src/__tests__/ 下，jsdom 30 行为变更导致的适配）。
npm install 后跑步骤门禁（tsc/eslint/npm test/test:l3）全绿交付；jsdom 30 行为变更（如 API 移除/默认变化）逐处适配。无法修复则回滚本步并注明。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（共享 package.json，严格串行）===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'step-b-typescript7',
    prompt: `你负责 TE-07（步骤 B）：typescript 6.0.3 → 7.x（按 D2 纳入），只许改 package.json、package-lock.json、tsconfig*.json（如需适配）、源码类型适配文件（tsc 报错逐处修，禁止用 any/as unknown as 压制——确需类型断言处注释理由）。
npm install 后 npx tsc --noEmit 全绿；再跑步骤门禁（tsc/eslint/npm test/test:l3）全绿交付。vitest/eslint 工具链与 TS 7 兼容性如冲突，逐处解决并记录。无法修复则回滚本步并注明。`
  },
  {
    label: 'step-c-json-schema',
    prompt: `你负责 TE-09（步骤 C）：json-schema-library 9.3.5 → 11.x（跨 2 major，按 D2 纳入），只许改 package.json、package-lock.json、src/features/hooksConfig/ 下适配文件。
同时评估与 codemirror-json-schema 的去重——能统一则统一到单一库（优先保留与 CodeMirror 集成更好的 codemirror-json-schema，保存校验改用它；以实测 API 为准），评估结论写进报告（登记在 S19）。
npm install 后跑步骤门禁全绿交付（hooksConfig 模块测试重点盯）。无法修复则回滚本步并注明。`
  },
  {
    label: 'step-d-dockview8',
    prompt: `你负责 TE-08（步骤 D，最高风险最后）：dockview-react 6.6.1 → 8.x（跨 2 major，按 D2 纳入），只许改 package.json、package-lock.json、src/workspace/ 下适配文件（layoutSerde/Workspace/PageDockviewHost 等）、src/panels/ 下适配文件（如需）、src/__tests__/ 下对应测试。
适配要点：dockview 8 breaking changes 逐个核对官方迁移文档（组件 API/样式类名/serde 格式）；layoutSerde 的 toJSON/fromJSON 契约不变（约束 #7——旧布局 JSON 必须能恢复）；样式 import 路径变更须核对。
npm install 后跑步骤门禁全绿交付（layout-serde 测试重点盯）。无法修复则回滚本步并注明——本步可独立回滚，不影响 A/B/C 已绿成果。`
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run test:l3
7. npx vite build
8. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 15 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-15.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
