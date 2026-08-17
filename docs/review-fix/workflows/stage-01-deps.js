// =====================================================================
// stage-01-deps.js — S01 依赖安全排雷（TE-01/02/05/06/12）
// =====================================================================
// 跨边界契约（写死，并行 agent 不各自推断）：
//   WDIO 三处版本必须两侧对齐 1.3.0——npm: @wdio/tauri-plugin、@wdio/tauri-service；
//   cargo: tauri-plugin-wdio-webdriver。任一侧偏离 E2E 链路即挂。
// fix-loop 调用约定：args.testCommands 传本脚本下方 7 条门禁；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage01-deps',
  description: 'Stage 01: 依赖安全排雷——WDIO 9.30.1 消 serialize-javascript RCE、git2 0.21、Tauri patch、隐式依赖声明、knip 配置（TE-01/02/05/06/12）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S01 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：WDIO 三处版本两侧对齐 1.3.0（npm @wdio/tauri-plugin、@wdio/tauri-service；cargo tauri-plugin-wdio-webdriver）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'npm-deps',
    prompt: `你负责 TE-01（npm 侧）、TE-02，只许改 package.json、package-lock.json：
【TE-01 npm 侧】所有 @wdio/* 9.28.0 → 9.30.1（含 @wdio/mocha-framework——其传递依赖 serialize-javascript 存在 RCE GHSA-5c6j-r48x-rmvq，须升 ≥7.0.5）；@wdio/tauri-plugin、@wdio/tauri-service 1.1.0 → 1.3.0（契约：与 Rust 侧对齐）；expect-webdriverio 升至与 WDIO 9.30 兼容版本。npm install 后检查 package-lock.json 中 serialize-javascript 全部实例 ≥7.0.5，不足则用 package.json overrides 强制。最后 npx npm audit 复验 WDIO 链路 high=0（报告剩余 high 数）。
【TE-02】json-schema（消费点 src/panels/hooksConfig/JsonMode.tsx，先 grep 核实 import 来源）与 @lezer/highlight（消费点 src/theme/overrides.ts，先 grep 核实）为未声明传递依赖——按 package-lock.json 中当前传递版本显式声明进 dependencies（版本照抄 lock 解析值）。`
  },
  {
    label: 'cargo-deps',
    prompt: `你负责 TE-01（Rust 侧）、TE-05、TE-06，只许改 src-tauri/Cargo.toml、src-tauri/Cargo.lock（TE-05 API 适配时含 src-tauri/src/git/ 下调用点文件）：
【TE-01 Rust 侧】tauri-plugin-wdio-webdriver 1.1.0 → 1.3.0（契约：与 npm 侧 @wdio/tauri-plugin 1.3.0 对齐）。
【TE-05】git2 0.20 → 0.21（vendored-libgit2 feature 保持）；cargo check 编译，git2 0.21 API 变更则适配 src-tauri/src/git/ 下 status/diff 调用点，逐处改到编译通过。
【TE-06】tauri 2.11.3 → 2.11.5、tauri-build 同步升、其余 tauri-plugin-* 按兼容 patch 升级。
只做 cargo check / cargo clippy --manifest-path src-tauri/Cargo.toml --no-deps 编译级检查，禁止 cargo test。`
  },
  {
    label: 'knip-cfg',
    prompt: `你负责 TE-12，只许新建 knip.json：
新建 knip.json——entry 含 e2e-tests/**/*.ts（@wdio/* 被 e2e 消费但未入 entry 遭误报）及前端既有入口（参照 vite 结构：index.html / src/main.tsx）。运行 npx knip --production 确认 @wdio/* 不再误报未使用。knip 可能报出其它真实死代码——本 Stage 只消 @wdio/* 误报类，其它发现原样写进报告不处理（死代码清理在 S14）。`
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
6. npx vite build
7. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
