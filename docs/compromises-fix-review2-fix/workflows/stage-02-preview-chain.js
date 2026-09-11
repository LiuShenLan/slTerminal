// =====================================================================
// Stage 02 预览链路（SEC-01、SEC-04）
// 串行形态：用户并发限制约束——agents for 循环顺序 await，不用 parallel()；
// 全量测试命令逐条串行（CP-007 基准负载敏感）。
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-02-preview-chain',
  description: 'Stage 02 预览链路：CSP 测试补 script/style 断言 + sync 失败注释文案失实修正',
  phases: [
    { title: '串行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点唯一真值源 = docs/compromises-fix-review2-fix/checklist.md 对应 ID 条目（先读再动手）——条目为六段式（位置/现状/修复步骤含可照抄代码块/测试同步/文档同步/验证），修复步骤段的代码块照抄适配，不自行设计；每项完成后跑条目「验证」段断言自查。`

// === Phase 1: 串行修复（用户并发限制——顺序 await，禁 parallel）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'preview-csp-test',
    prompt: `你负责 SEC-01（先读 checklist 对应条目再动手）：
【SEC-01】src-tauri/src/preview.rs 的 host_page_carries_domain_csp 用例（:604-611）追加 script-src/style-src 两断言（checklist 内 Rust 代码块照抄）；既有四断言不动。
触碰文件仅限：src-tauri/src/preview.rs。
自查：cargo test --test lib_tests preview -- --test-threads=1（L1 定向红线——禁 --lib/裸 filter）绿。`,
  },
  {
    label: 'previewframe-comment',
    prompt: `你负责 SEC-04（先读 checklist 对应条目再动手）：
【SEC-04】src/panels/docViewer/PreviewFrame.tsx:236-246 注释与 warn 文案改写（checklist 修复步骤段措辞照抄）——旧口径「下轮轮询自愈」失实（等值早退不重发 sync）。
前置自查：rg 下轮轮询自愈 src/__tests__/html-panel.test.tsx——若 sync warn 用例断言含旧文案片段则同步适配（条件触碰，未命中则不动测试文件）。
触碰文件仅限：src/panels/docViewer/PreviewFrame.tsx（+ 条件命中时 src/__tests__/html-panel.test.tsx）。`,
  },
]
const refactorResults = []
for (const a of agentsSeq) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行执行，禁并行——CP-007 基准负载敏感）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行，禁止并行**（CP-007 基准负载敏感，并行抢核曾致误红）：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --test lib_tests preview -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
