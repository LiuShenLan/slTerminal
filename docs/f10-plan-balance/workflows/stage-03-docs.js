// =====================================================================
// F10 编码套餐余量展示 — Stage 03 文档与登记（PB-DOC-01~04）
// =====================================================================
// 清单真值源：docs/f10-plan-balance/checklist.md（PB-DOC-01~04 六段式）
// Stage 特殊纪律：文档 Stage——只改文档与登记文件，禁止改生产代码与测试代码
//   （fix-loop 调用时 args.constraints 传：本 Stage 只改文档与登记文件，禁止改生产/测试代码）
// =====================================================================

export const meta = {
  name: 'f10-stage03-docs',
  description: 'F10 Stage 03：模块文档与用例清单登记（navTree/ipc/src-tauri CLAUDE.md + test-inventory）',
  phases: [
    { title: '文档登记' },
    { title: '测试取数' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
特殊纪律：本 Stage 只改文档与登记文件，禁止改生产代码与测试代码。
背景：实现依据 = docs/f10-plan-balance/checklist.md 对应 ID 条目（先读再动手）；文档口径必须对照当前真实代码核实，防文档撒谎（ADR-0011）。`

// === Phase 1: 文档登记（单 agent）===
phase('文档登记')
const parallelAgents = [
  {
    label: 'docs',
    prompt: `你负责 PB-DOC-01 / PB-DOC-02 / PB-DOC-03 / PB-DOC-04。先 Read docs/f10-plan-balance/checklist.md 对应四条目六段式；每处文档写入前 Read 目标节现状 + 对照最终代码核实口径。

【PB-DOC-01】src/features/navTree/CLAUDE.md + src/ipc/CLAUDE.md：
  1. navTree CLAUDE.md 行结构契约节加 footer 条目（照 checklist 文案：位置 U1/行高 28/fg-3/logo 14px onError 隐藏/整块不渲染含发丝线/点击刷新节流 5s/颜色全 token 无例外）
  2. data-e2e 契约加 plan-balance-footer / plan-balance-row 两值
  3. 测试模式节 L2 文件清单加 plan-balance-model.test.ts / plan-balance-footer.test.tsx / ipc-plan-balance-contract.test.ts
  4. ipc CLAUDE.md Event 模式段 onFsEvent/onAgentEvent 句加 onPlanBalanceUpdated；新增「planBalance 命令（F10）」小段（getPlanBalance 挂载拉快照 / refreshPlanBalance 恒 Ok 返回最新快照（D6）/ onPlanBalanceUpdated 订阅 plan-balance-updated（有变化才推送，含 updatedAt 口径 D5））
【PB-DOC-02】src-tauri/src/CLAUDE.md「settings.rs — 浅合并 + 保存互斥 + 白名单」节 SEC-11 行：白名单清单改五键（加 planBalance），补注 F10 轮询间隔（手改文件，读取侧 plan_balance 模块 resolve_poll_interval，越界回退 60s）。
【PB-DOC-03】.claude/test-inventory.md 登记（计数纪律 TQ-CI-01，**实跑取数**）：
  1. L1 表加 plan_balance 五文件各行（mod/source/query/deepseek/kimi），settings.rs 行用例数 25→26，覆盖要点标 F10
  2. L2「IPC 层」表加 ipc-plan-balance-contract.test.ts 行；「导航树」表加 plan-balance-model.test.ts / plan-balance-footer.test.tsx 两行
  3. 豁免清单表加一行（照 checklist 文案四列：plan_balance 真实 HTTP 查询（ureq fetch）与 tokio 轮询任务本体 | 真实外部 API 依赖 + Tauri 运行时（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖 + L2 UI 四场景 + 人工实测 | F10）
  4. 计数实跑取数：cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 总通过数 + grep -c '#\\[test]' 双核对（8.3 短名注意：只取计数不比较路径）；npm test 的 Vitest 报告总数。表头总数行 / 计数口径行（L1 文件数 34→39、L2 文件数 156→159，以实际为准）/ 段小计 / 行级和四处一致
  5. 新增行均含 F10 标记
【PB-DOC-04】验证项（默认不改）：Read CONTEXT.md:240-253 四术语与 .claude/CLAUDE.md:193 F10 索引行，对照最终实现（src-tauri/src/plan_balance/ 与 src/features/navTree/PlanBalanceFooter.tsx）核实无失实；仅当实现口径漂移时修订对应描述（修订 CONTEXT.md 时须告知主 agent 需临时 git add CONTEXT.md）。

【文档纪律】每处修改前 Read 目标节；写入内容对照真实代码（grep/Read 核实函数名/键名/事件名与代码一致）；不改任何 .rs/.ts/.tsx 文件。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 测试取数（文档 Stage 门禁 = 双测试命令全绿 + 统计供 verify 核对登记数）===
phase('测试取数')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行以下命令（相互独立，并行启动执行，收集全部结果）：
1. npm test
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
**另需统计**（供 verify 核对 test-inventory 登记数）：
- npm test 的 Vitest 最终报告：Test Files 通过数/总数、Tests 通过数/总数
- cargo test 最终统计行：test result: ok. N passed
- grep -c '#\\[test]' src-tauri/src/plan_balance/*.rs 五文件各自计数 + src-tauri/src/settings.rs 计数
cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/f10-plan-balance/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照真实代码核实，防文档撒谎。
以下为测试 agent 的测试执行与统计结果，测试类断言与计数断言据此判定（无需重跑）：
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
