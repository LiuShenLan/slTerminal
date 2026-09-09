// =====================================================================
// Stage 02 · 预览链路（SEC-01/02/03 + FE-06）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行（用户并发限制约束）；测试命令逐条串行。
// 安全红线（写死，agent 不得偏离）：
//   - iframe sandbox 禁 allow-same-origin（CVE-2024-35222）
//   - 主窗口 CSP 终态 script-src 'self' 无 data:（csp-config.test.ts 锁死）——
//     本 Stage 的 CSP meta 只加在预览域宿主页 HOST_PAGE（独立 webview），
//     禁触碰主窗口 CSP
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 02 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-02-preview-chain',
  description: 'S02 预览链路：validate_label 放宽 + HOST_PAGE CSP meta + PreviewFrame catch 可观测 + 注入纪律测试锁',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
安全红线：iframe sandbox 禁 allow-same-origin（CVE-2024-35222）；主窗口 CSP（tauri.conf.json/csp-config.test.ts 锁死面）禁触碰——CSP meta 只加预览域宿主页 HOST_PAGE。
L1 定向红线（TQ-COV-06）：cargo 定向测试一律 \`cargo test --test lib_tests <filter> -- --test-threads=1\`，禁裸 filter/--lib。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const agents = [
  { label: 'preview-label-csp', prompt: `你负责 SEC-01 + SEC-02（先读 checklist 两条目六段式原文；SEC-02 文档同步你只承担 src-tauri/src/CLAUDE.md 一处，其余三处归另一 agent，勿动）：
【SEC-01】src-tauri/src/preview.rs:51-68 validate_label 字符判定加 \`b == b':' || b == b'/'\` 放行（与 tauri-runtime-2.11.3 window.rs:534 合法字符集对齐）；测试 label_accepts_legal_panel_ids 加正例 "preview-page-1:html-2"/"preview-a/b"，label_rejects_illegal_forms 移除 "preview-a/b"。L4 用例面归 injection-lock agent，勿动 html.e2e.ts。
【SEC-02】preview.rs HOST_PAGE 常量（:449-458）<meta charset> 后插 CSP meta，content 值逐字照抄 checklist：default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:；新增 L1 用例 host_page_carries_domain_csp（断言 HOST_PAGE 含该 meta）；src-tauri/src/CLAUDE.md 登记预览域 CSP 形态与理由（按 checklist SEC-02 文档同步节口径）。
自验：cargo test --test lib_tests preview -- --test-threads=1 绿（定向形态红线内）；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check 绿。` },
  { label: 'preview-frame', prompt: `你负责 SEC-03 + SEC-02 前端文档面（先读 checklist SEC-03/SEC-02 六段式原文；SEC-02 文档同步你只承担 src/panels/docViewer/CLAUDE.md、src/panels/CLAUDE.md、.claude/adr.md 三处，src-tauri/src/CLAUDE.md 归另一 agent，勿动）：
【SEC-03】src/panels/docViewer/PreviewFrame.tsx：sync catch（:224-226）加一次性旗标 syncWarned（首次 warn 后置位，sync 成功时复位，避免刷屏）；close catch（:243-245）直接 console.warn（一次性事件无刷屏面）。src/__tests__/html-panel.test.tsx 新增用例（按 checklist SEC-03 测试同步节——sync 失败两次只 warn 一次 + 成功后复位再失败再 warn；close 失败 warn）。
【SEC-02 文档面】三处文档按 checklist SEC-02 文档同步节口径登记预览域宿主页 CSP meta 形态。
自验：npx vitest run html-panel 绿。` },
  { label: 'injection-lock', prompt: `你负责 FE-06 + SEC-01 L4 用例面（先读 checklist FE-06/SEC-01 六段式原文）：
【FE-06】src/__tests__/doc-viewer-injection.test.ts 段组合 describe 内新增三组合矩阵用例（无 extra / fragmentNav / linkRouter+scrollReport），各断言注入产物 out.match(/<script>/g) 与 /<\\/script>/g 均 toHaveLength(1)——照抄 checklist FE-06 代码块。
【SEC-01 L4】e2e-tests/html.e2e.ts 新增页前缀形态用例（panelId 形如 preview-page-1:html-2 走真实 label 路径——照 checklist SEC-01 测试同步节）；不跑 L4（全量测试 phase 统一跑）。
自验：npx vitest run doc-viewer-injection 绿。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行；含 L4——预览承重链路）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行）：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
7. npx tauri build --debug --no-bundle
8. node e2e-tests/run-wdio.cjs --spec html.e2e.ts
第 8 条前提：slTerminal 窗口须前台聚焦（TQ-E-10 探针）；探针 fast-fail 则原样报告不自行重试。cargo 系命令共享 target 目录锁排队属正常勿中止。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。npm test 报告最终统计行原文。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
