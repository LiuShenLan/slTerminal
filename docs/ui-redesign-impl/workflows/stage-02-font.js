// =====================================================================
// Stage 02 — 字体内置（FT-01~FT-04、FT-08）
// 契约：全局字体栈唯一真值 = "JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage02-font',
  description: 'Stage 02: JetBrains Mono 字体内置 + 全局字体栈统一',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "font-deps", prompt: `你负责 FT-01/FT-08：
【FT-01】执行 npm install @fontsource/jetbrains-mono（package.json 新增依赖）；src/main.tsx ② 阶段（setActive 之后、ROOT_CSS_VARS 注入前后均可）新增两行 import：@fontsource/jetbrains-mono/400.css 与 @fontsource/jetbrains-mono/500.css（静态 import 放文件顶部即可——main.tsx 为入口，字体 CSS 随产物打包，断网可用）。
【FT-08】src/main.tsx:28 超时错误页 font-family:monospace 改为完整规格栈："JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace（inline style 字符串内形式 font-family:'JetBrains Mono','Cascadia Mono',Consolas,'Microsoft YaHei UI',monospace——注意外层字符串引号冲突，先 Read 现状拼接再改）。` },
  { label: "font-stack", prompt: `你负责 FT-02/FT-03/FT-04：全局字体栈统一为 "JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace。
【FT-02】src/App.css:8 font-family 现值（Cascadia Code 优先那串）改规格栈。
【FT-03】src/panels/editor/useCodeMirror.ts:48 与 :57 两处 .cm-scroller fontFamily（现 JetBrains Mono, monospace）改规格栈。
【FT-04】src/panels/terminal/theme.ts:13 fontFamily（现 JetBrains Mono, monospace）改规格栈。
改完全仓 grep font-family/fontFamily 确认无其它声明残留（如发现新声明点，只读报告不改）。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm run test:l3
7. npx vite build
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
