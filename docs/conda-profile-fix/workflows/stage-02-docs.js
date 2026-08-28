// =====================================================================
// Stage 02 — 文档同步（pty/CLAUDE.md 红线 + 根编号索引 + 用例清单）
// =====================================================================
// 改动项（逐 ID 对照 docs/conda-profile-fix/checklist.md 原文）：
//   DOC-B17a（src-tauri/src/pty/CLAUDE.md）
//   DOC-B17b（.claude/CLAUDE.md）
//   DOC-B17c（.claude/test-inventory.md）
// 三文件零重叠，并行。
// Stage 特殊纪律（fix-loop 调用时 args.constraints 传同一句话）：
//   本 Stage 只改 markdown 文档，禁止改任何代码/测试文件。
// =====================================================================

export const meta = {
  name: 'stage02-docs',
  description: 'Stage 02: B17 文档同步——pty 红线 + 根编号索引 + 用例清单/豁免登记',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/conda-profile-fix/checklist.md 对应 ID 条目（先读再动手），引用文字一律照抄 checklist。
Stage 特殊纪律：本 Stage 只改 markdown 文档，禁止改任何代码/测试文件。
文档口径：所有描述须对照 src-tauri/src/pty/shell.rs Stage 01 修复后的真实代码（启动参数为 -NoLogo -NoExit -EncodedCommand，无 -NoProfile），不撒谎。`

// === Phase 1: 并行文档修改（三文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'doc-pty',
    prompt: `你负责 DOC-B17a（文件 src-tauri/src/pty/CLAUDE.md）：
先 Read docs/conda-profile-fix/checklist.md 的 DOC-B17a 条目，按其「修复步骤」1-2 执行：
1. 「Shell 白名单（SEC-01 / SEC-15）」段尾行整行替换为 checklist 给出的新行
2. 「外部坑/红线」列表追加 checklist 给出的 B17 红线行
动手前 Read 该文件确认现状行与 checklist 摘录一致；漂移以现状为准并报告。`,
  },
  {
    label: 'doc-root',
    prompt: `你负责 DOC-B17b（文件 .claude/CLAUDE.md）：
先 Read docs/conda-profile-fix/checklist.md 的 DOC-B17b 条目，在「需求编号索引」表 B16 行之后追加 checklist 给出的 B17 行（照抄）。
动手前 Read 该文件定位 B16 行；确认无既有 B17 行（防重复登记）。`,
  },
  {
    label: 'doc-inventory',
    prompt: `你负责 DOC-B17c（文件 .claude/test-inventory.md）：
先 Read docs/conda-profile-fix/checklist.md 的 DOC-B17c 条目，按其「修复步骤」1-4 执行：
1. 先实跑取数（登记纪律 TQ-CI-01，禁凭计算）：
   - cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1，汇总全部 test result 行的 passed 总数
   - 统计 src-tauri/src/pty/shell.rs 的 #[test] 属性数（grep 或 Read 计数）
2. 更新 shell.rs 行（计数 32 改为实跑属性数，描述按 checklist）
3. 同步 L1 段头与全量表头三处计数（以实跑数为准）
4. 豁免表追加 checklist 给出的 B17 人工验证行
动手前 Read 该文件相关行确认现状；cargo test 耗时长属正常，勿中止。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（为 DOC-B17c 计数核对供数；纯文档 Stage 无静态门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行以下命令：
1. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
报告：exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。报告末尾附全部 test result 行的 passed 总数汇总（供 DOC-B17c 计数核对）。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/conda-profile-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码/文档判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照真实代码核实，防文档撒谎。
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
