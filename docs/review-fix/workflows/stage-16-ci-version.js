// =====================================================================
// stage-16-ci-version.js — S16 版本策略 + CI 门禁（TE-03/04/11/13）
// =====================================================================
// 跨边界契约：无。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage16-ci-version',
  description: 'Stage 16: 版本策略统一（生产精确/开发 ^）+ CI 增 audit/knip 门禁 + ADR 登记（TE-03/04/11/13）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S16 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：dependencies 全精确版本、devDependencies 全 ^（xterm beta 等既定例外以 adr 登记为准）；ADR 三条登记 = 版本策略约定 / xterm beta 升级审批约定 / notify RC 现状。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'version-policy',
    prompt: `你负责 TE-11、TE-03、TE-04，只许改 package.json、.claude/adr.md：
【TE-11】59 依赖版本策略不一致（8 精确 + 51 ^）。统一约定：dependencies（生产运行时）全精确版本、devDependencies（开发工具）全 ^——package.json 逐条调整（不改 lock 解析版本本身，npm install 刷新 lock）。
【TE-03】xterm 三件套 beta（6.1.0-beta.288）保留——.claude/adr.md 补升级审批约定（xterm 升级须全量 L3+E2E+实测滚轮，引用既有 ADR 动机：调查5修复，回退稳定版会回归）。
【TE-04】notify@9.0.0-rc.4 / notify-debouncer-full@0.8.0-rc.2 保持 RC——.claude/adr.md 登记（一手证据：rc.4 即最新无稳定版可升，Cargo.toml:36-37 已有跟踪注释；notify 模块 51 条 L1 watcher 回归守护）。
只做 npx tsc --noEmit 编译级检查（package.json 变更后 npm install 一次确保 lock 一致），禁止 npm test。`
  },
  {
    label: 'ci-gates',
    prompt: `你负责 TE-13，只许改 .github/workflows/ci.yml：
按 D10 增加三项 CI 门禁：
1. npm audit --registry=https://registry.npmjs.org/ --audit-level=high（high 级即阻断——注意带 registry 参数避免镜像源 audit 端点缺失）
2. npx knip --production（死代码门禁，用 S01 所建 knip.json）
3. cargo install cargo-audit && cargo audit（advisory-db CI 可拉取；本地网络受限跳过）
插入位置：既有测试 job 内合适步骤（先 Read ci.yml 现状再改）。不跑测试。`
  },
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

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 16 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-16.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
