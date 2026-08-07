// =====================================================================
// Stage 04 启动序列：main.tsx 动态 import 链 + App.css 归位（BOOT-01~03）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   C1（Stage 02 产物，本 Stage 消费）facade 31 导出 + ROOT_CSS_VARS
//     键集合恰 { "--sl-bg-primary", "--sl-fg-primary" }。
//   启动链时序不变量：schemeRegistry setActive 必须先于任何 facade 求值
//   （即先于 import ./theme barrel 与 import ./App）；E2E helpers 注入
//   在 setActive 之后、App 渲染之前。
// fix-loop 调用约定：本 Stage 不传 args.constraints。
// 计划文档：docs/color-plan/checklist.md + docs/color-plan/stages.md。
// =====================================================================

export const meta = {
  name: 'stage4-bootstrap',
  description: 'Stage 04 启动序列：main.tsx 动态 import 链 + App.css 归位（BOOT-01~03）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
本 Stage 单 agent——启动链时序强耦合，四文件必须同视野。
E2E 时序不变量：helpers 注入在 setActive 之后。
跨边界契约以脚本头部注释为准，不各自推断。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A1-boot',
    prompt: `你负责 BOOT-01/02/03（src/main.tsx + src/App.tsx + src/App.css + src/__tests__/bootstrap.test.ts 四文件）。先 Read docs/color-plan/checklist.md 中 BOOT-01~03 条目与 docs/color-scheme-refactor-spec.md §5（启动序列章节），再 Read 四个现状文件。

【BOOT-01】src/main.tsx 启动链改造：
① 静态 import 由现状 6 个收敛为 3 个——react、react-dom/client、./lib/e2eEnabled 深导入。注意：./lib barrel 会经 ErrorBoundary:10-13 传递引用 theme，导致 facade 在 setActive 前求值，故 e2eEnabled 必须深导入绕开 barrel；
② 启动序列严格按序：IPC wait + fail-safe（现 :29-33 逻辑不变）→ loadSettings().catch(() => null) + 动态 import("./theme/schemeRegistry") + import("./theme/schemes")（side-effect 注册 darcula）+ setActive(settings?.colorScheme)（未知 id 回退 darcula 由注册表内部保证）→ 动态 import("./theme") 取 ROOT_CSS_VARS 注入 document.documentElement → E2E helpers 注入（现 :42-45 逻辑不变，仅位置随链，必须在 setActive 之后）→ await import("./App") + render；
③ src/__tests__/bootstrap.test.ts 适配：补 ../ipc/settings mock（loadSettings resolve null），使动态 import 链在 jsdom 下可跑；既有断言按新链对齐，不改预期行为。

【BOOT-02】src/App.tsx——现 :23 dockview.css import 之后追加 import "./App.css"（CSS 加载顺序：dockview.css 先、App.css 后，变量覆盖语义正确）。

【BOOT-03】src/App.css——删现 :5-7 三行（注释行 + --sl-fg-primary: #cdd6f4; + --sl-fg-secondary: #a6adc8;），var() 引用（现 :15/:16/:36/:37）全部保留——变量来源改由 BOOT-01 的 ROOT_CSS_VARS 注入。

完成后报告：四文件改动摘要 + main.tsx 静态 import 清单自检（须恰 3 个）。`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
- npx tsc --noEmit
- npx eslint src/
- npm test
- npx vite build
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/color-plan/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

// 人工验证点（不可自动化，须人工确认后 Stage 04 才算完成——真值源 stages.md 人工验证点汇总段）
log('Stage 04 人工验证点：① E2E helpers 时序 = helpers 注入在 setActive 之后（真实 WebView2 启动时序，jsdom 不可验，L4/人工兜底）；② CSS 加载顺序 = dockview.css 先、App.css 后（构建产物抽查）')

return { refactorResults, testResult, verifyResult }
