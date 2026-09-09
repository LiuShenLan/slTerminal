// =====================================================================
// Stage 07 · 文档登记（DOC-01~11）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行；测试命令逐条串行。
// 本 Stage 纯文档 + 红测演练（演练 = 临时改 → git checkout 还原，零持久变更）。
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 07 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-07-docs-registry',
  description: 'S07 文档登记：compromises 注记回写 + 根 CLAUDE.md/test-exemptions 口径 + .bak 删除 + 红测演练',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
Stage 特殊纪律：本 Stage 只改文档与执行演练动作，**禁止改生产代码**（红测演练的临时改动必须 git checkout 还原，终态零持久变更）；历史执行档（docs/compromises-fix/）回写以「落地复核」注记形式追加，不改写历史断言原文（防篡改历史记录语义）。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行——每条注记的改写文案 checklist 已写死，照抄）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const agents = [
  { label: 'compromises-notes', prompt: `你负责 DOC-01、DOC-02（注记面）、DOC-05、DOC-06（1）、DOC-07、DOC-08、DOC-11（先读 checklist 各条目六段式原文）——全部落 docs/compromises.md：
【DOC-01】:21 CP-002 括注改「（jsonSchemaCm.ts 七导出——二处生产接线，余五导出仅测试直驱）」。
【DOC-02 注记面】:23 CP-003 注记末句「原便携 Node 22 隔离于 .temp/node22.bak 待全量 e2e 确认后删」→「原便携 Node 22 备份 .temp/node22.bak 已于全量 e2e 确认后删除（2026-09-09）」（.bak 删除动作归 claude-md-registry agent，勿动）。
【DOC-05】:34 CP-004 注记末尾补页内分屏消亡登记（文案照抄 checklist）。
【DOC-06(1)】:48 CP-011 注记「无裸 join 无界阻塞路径」→「生产+测试全仓零裸 join（守卫命令：rg "\\.join\\(\\)" src-tauri/src 仅命中 thread_join.rs 内守卫白名单一处）」。
【DOC-07】:38 CP-006 注记末尾补 keyset 根治句（文案照抄 checklist）。
【DOC-08】:107 CP-030 注记末尾补探针副作用登记句（文案照抄 checklist）。
【DOC-11】:55 CP-012、:84 CP-022、:40 CP-007 三行各补「2026-09-09 复核加固」句（文案照抄 checklist）。
自验：checklist 各条目验证节 grep 断言逐条过。` },
  { label: 'claude-md-registry', prompt: `你负责 DOC-09、DOC-10、DOC-02（删 .bak 动作）（先读 checklist 各条目六段式原文）：
【DOC-09】.claude/CLAUDE.md:77 括注去计数——「（lib_tests = src/lib.rs 显式 test target，721 例等价覆盖 --lib）」→「（lib_tests = src/lib.rs 显式 test target，等价覆盖 --lib 全量用例）」；同文件「核心原则」节「隔离优先」条后补串行纪律条（文案照抄 checklist DOC-09 步骤 2——L1/L2 禁并行，CP-007 基准负载敏感，2026-09-08 实测 410ms vs 180ms）。
【DOC-10】.claude/test-exemptions.md:70 旧 sha256 整文件口径句改哨兵键级口径（文案照抄 checklist——hooks/statusLine/env 存在性+值快照比对；statusline-backup 维持 sha256、hooks/ 整树）。
【DOC-02 删 .bak】若 .temp/node22.bak 存在则删除（untracked——纯文件系统动作，无 git 操作）；.temp/node22 预置通道保留（run-wdio.cjs:306-323 存活功能，头注不动）。
自验：rg "721" .claude/CLAUDE.md 零命中；.temp/node22.bak 不存在。` },
  { label: 'exec-doc-replay', prompt: `你负责 DOC-03、DOC-04、DOC-06（2/3）（先读 checklist 各条目六段式原文）：
【DOC-04】docs/compromises-fix/workflows/verify/stage-10.md:15 的 CP-033 行 B2 分支句后补「落地复核 2026-09-09」注记（文案照抄 checklist DOC-04 步骤 1）；docs/compromises-fix/checklist.md:1915 同款回写。回写形态 = 注记追加，不改写历史断言原文。
【DOC-06(2/3)】docs/compromises-fix/checklist.md:674（CP-005 验证节）守卫命令改 rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs"；:851（CP-011 验证节）命令改 rg "\\.join\\(\\)" src-tauri/src 仅命中 thread_join.rs 口径——均以落地复核注记形式追加（不改写历史断言原文）。
【DOC-03 红测演练】按序执行四步并记录每步命令 + exit code：
  1. 手改 src/panels/markdown/generated/katexInlineCss.ts 任意一字符（注释内加字符即可）；
  2. git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts → 应非 0（守卫抓漂移，红测阳性）；
  3. git checkout -- src/panels/markdown/generated/katexInlineCss.ts 还原；
  4. node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts → 应 0（生成幂等归位）。
  若任一步 exit code 与预期不符，停下报告（不要继续）。
报告必须含：演练四步的命令 + exit code 原文记录——主 agent 据此写 Stage commit body。
自验：终态 git status 确认 katexInlineCss.ts 零持久变更。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（纯文档 Stage 收窄门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行）：
1. npx tsc --noEmit
2. npx eslint src/
3. node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
（纯文档 Stage 收窄门禁——触碰面为 md 文件 + untracked 目录删除 + 演练还原零持久变更；tsc/eslint 防文档 agent 误碰代码；第 3 条 = 演练终态归位确认。收窄理由已登记 stages.md。）
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
注意：commit body 红测演练记录属主 agent 自查项（commit 在 verify 后发生）——你验证的是重构 agent 报告中的演练记录（由主 agent 转述如下）与 katexInlineCss.ts 终态零 diff。
重构 agent 报告：
---
${refactorResults.length > 0 ? refactorResults.join('\n---\n') : '（重构 agent 未返回）'}
---
以下为测试 agent 的验证执行结果，测试类断言据此判定（无需重跑）：
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
