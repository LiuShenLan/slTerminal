// =====================================================================
// Stage 06 · e2e 设施（TE-04/05/06/08/09）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行；本 Stage 三 agent 顺序写死
// plan-env-helper → katex-font-probe → wdio-launcher——
// e2e-tests/CLAUDE.md 统一归 wdio-launcher（禁双 agent 碰同一文件），
// TE-06/TE-08 的登记段由它读代码现状后按实写（代码自证，不经报告转述）。
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 06 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-06-e2e-infra',
  description: 'S06 e2e 设施：哨兵 env + 探针降级 + writeFakePlanEnv 收编 + KaTeX 字体锚点 + WARN 计数',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
SEC-18 红线：真实凭据禁入任何 git 追踪文件；测试仅允许 sk-test 假值占位符。
Stage 特殊纪律：e2e-tests/ 不在根 tsconfig include——构建级门禁 = 实跑对应 spec，不得以静态检查绿代替；L4 前提 = slTerminal 窗口前台聚焦（TQ-E-10 探针），探针 fast-fail 则停下报告，不自行绕过。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构（顺序写死 B→C→A）===
phase('串行重构')
const agents = [
  { label: 'plan-env-helper', prompt: `你负责 TE-06（先读 checklist TE-06 六段式原文）：
新建 e2e-tests/node-helpers.ts（Node runner 侧公共件——与浏览器侧 helpers.ts 分工：helpers.ts 经 VITE_E2E 打包进前端挂 window 全局禁 Node API，本文件仅被 spec 在 Node 进程 import）；writeFakePlanEnv 函数照抄 checklist TE-06 代码块（以 settings 版为基含 mkdirSync，参数化 claudeSettingsPath）。
e2e-tests/settings.e2e.ts 删 :200-217 本地函数改 import 调用；e2e-tests/background-tasks.e2e.ts 删 :261-275 本地副本改 import 调用。
e2e-tests/CLAUDE.md 归 wdio-launcher agent，勿动。
自验：node e2e-tests/run-wdio.cjs --spec settings.e2e.ts 与 node e2e-tests/run-wdio.cjs --spec background-tasks.e2e.ts 各自 solo 绿（后者验证 mkdirSync 对齐——原副本缺 mkdirSync 的 solo ENOENT 破口闭合）。` },
  { label: 'katex-font-probe', prompt: `你负责 TE-08（先读 checklist TE-08 六段式原文——本项为**实证驱动**，两分支代码均在 checklist 写死，你的第一步是实跑定分支）：
1. 实证：e2e-tests/markdown.e2e.ts :115 后临时插入分支 a 探针（照抄 checklist TE-08 步骤 1 代码块）；直接 node e2e-tests/run-wdio.cjs --spec markdown.e2e.ts 跑（spec 改动不需重建产物）。看输出 [TE-08 探针] JSON：control===false 证明 check 语义有效；katex===true → **分支 a 成立**；katex===false → 转分支 b。
2. 分支 a 成立：探针固化为常驻断言（照抄 checklist 步骤 2 代码块，finally 切回 main）；复跑 --spec markdown.e2e.ts 确认绿。
3. 分支 b：照 checklist 步骤 3 全链落地（buildInjectedScript.ts fontProbe 段 + previewMessages.ts 白名单第四类型 + MarkdownPanel VITE_E2E 拼装 + PreviewFrame.tsx 上行收束写 window.__slterm_e2e_fontProbe + e2e waitUntil 断言 + 三个守卫测试同步）；改 src/ 后须 npx tauri build --debug --no-bundle 重建再复跑 spec。
4. 报告必须含：探针实证输出原文（[TE-08 探针] JSON 行）+ 落地分支（a/b）——主 agent 据此写 Stage commit body。
e2e-tests/CLAUDE.md 归 wdio-launcher agent，勿动（它在 CLAUDE.md 登记你的实证结论时读代码现状）。
自验：node e2e-tests/run-wdio.cjs --spec markdown.e2e.ts 绿；分支 b 时另 npx vitest run doc-viewer-preview-messages markdown-panel html-panel doc-viewer-injection 绿。` },
  { label: 'wdio-launcher', prompt: `你负责 TE-04 + TE-05 + TE-09 + e2e-tests/CLAUDE.md 全部登记（先读 checklist 三条目六段式原文）：
【TE-04】e2e-tests/run-wdio.cjs:86 SETTINGS_SENTINEL_KEYS 改 ["hooks", "statusLine", "env"]；:83-85 哨兵注释按 checklist 口径改写；:143-144 注释键列举同步（若有）。
【TE-05】e2e-tests/wdio.conf.ts:85-90 探针改首 worker fast-fail + 其余降级 warn——照抄 checklist TE-05 代码块（WDIO_WORKER_ID 判定：undefined 或 startsWith("0-") 为首 worker）。
【TE-09】run-wdio.cjs 加 wireWarnCounting 计数器（照抄 checklist TE-09 步骤 1）；runWdio（:296-304）execSync 改 spawn stdio:['inherit','pipe','pipe'] + FORCE_COLOR=1 + 两路 wireWarnCounting + close 打印计数行（照抄步骤 2——spawn 数组形态，cliArgs 拼串变量弃用改 ...process.argv.slice(2)，execSync 引用删除）；fallback（:326-333）同款接线（计数行插在既有 process.exit 前）。
【CLAUDE.md 登记】e2e-tests/CLAUDE.md 五处——TE-04：「防复发校验」节哨兵口径 + 「已知并发误报面」节 env 改写；TE-05：「定向运行官方形态」节改写（多 spec 降级 warn 但 $ 族吃 +5s，仍推荐单 spec/全量形态）；TE-09：「$()/elementClick 触发焦点检查」节矛盾句改写 + 节末补 WARN 可观测化段（基线数字先留占位「全量基线待本 Stage 全量 e2e 实跑填记」）；TE-06：「E2E helper 命名与挂载位置」节补 node-helpers.ts 分工条；TE-08：「多 webview WDIO 可达性」节补字体锚点登记——读代码现状判定分支（rg "fonts.check" e2e-tests/markdown.e2e.ts 命中 = 分支 a；rg "fontProbe" src/panels/docViewer/ 命中 = 分支 b），按实写一句。
自验：node e2e-tests/run-wdio.cjs --spec cli-aliases.e2e.ts 绿且输出末尾含 [wdio-launcher] tauri-service core.invoke WARN 计数 = N 行（N 写入报告——基线填记用）。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行；e2e 全量 = 构建级门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行）：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run e2e
第 4 条 = build:e2e + wdio 全量（e2e-tests/ 不在根 tsconfig include，构建级门禁 = 实跑兜底）；前提：slTerminal 窗口前台聚焦（TQ-E-10 探针 fast-fail 则原样报告不自行重试）。时长可达 20+ 分钟属正常勿中止。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。npm test 报告最终统计行原文；npm run e2e 报告各 spec 通过/失败汇总行 + 末尾 [wdio-launcher] WARN 计数行原文。
（本 Stage 无 Rust 源码改动——分支 b 触及 src/ 前端由 tsc/eslint/npm test 覆盖；收窄不跑 clippy/fmt/L1 全量，理由已登记 stages.md。）
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
