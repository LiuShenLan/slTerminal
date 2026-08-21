// =====================================================================
// Stage 10：文档同步（FE-31、DOC-11、DOC-12、DOC-13、DOC-14、TE-15）——固定最后 Stage
// 编排：并行 3（文件零重叠：A=editor/panels 文档；B=ipc/sideViews 文档；C=adr/test-inventory/根索引/hooksConfig）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-10.md
// =====================================================================

export const meta = {
  name: 'stage10-docs',
  description: 'S10 editor CLAUDE.md 新建 + 用例数校正 + Phase 2 决策/债务登记（FE-31、DOC-11~14、TE-15）',
  phases: [
    { title: '并行文档' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确位置与改写口径）。文档描述须对照当前代码核实，禁止凭记忆写数字。`

// === Phase 1: 并行文档（agent 间文件零重叠）===
phase('并行文档')
const parallelAgents = [
  {
    label: 'A-editor-docs',
    prompt: `你负责【FE-31】新建 src/panels/editor/CLAUDE.md +【DOC-13】panels/CLAUDE.md 4 处终端用例数校正。先读 \`docs/review-phase2-fix/checklist.md\` 第 10 节 FE-31/DOC-13 条目。

触碰文件：\`src/panels/editor/CLAUDE.md\`（新建）、\`src/panels/CLAUDE.md\`

【FE-31】步骤：
1. 新建 \`src/panels/editor/CLAUDE.md\`（模板：职责 → 架构决策（关键约束）→ 文件表 → 测试模式）：迁入 \`src/panels/CLAUDE.md\` 的编辑器专属节——大文件不虚拟化 FE-31/D3 决策全文、Compartment 语言/换行、滚动委托 .cm-scroller、Ctrl+S 注册表、CM6 主题层叠、useCodeMirror/gitGutter/EditorPanel/keyboard/activeEditor 文件表、编辑器测试模式表（先读 panels/CLAUDE.md 现文，迁移保持原意，用例数以 \`.claude/test-inventory.md\` 为准核对）
2. \`src/panels/CLAUDE.md\` 被迁走节改一行交叉引用（\`详见 @editor/CLAUDE.md\`），面板通用决策保留

【DOC-13】步骤（以 \`.claude/test-inventory.md\` 为真值源，先 grep 查实数再改）：
3. \`src/panels/CLAUDE.md\` 测试模式节：\`detect-webgl.test.ts\` 3→4、\`terminal-instance.test.ts\` 7→6、\`use-xterm-lifecycle.test.ts\` 80→86、\`terminal.test.tsx\` 19→27

完成后报告：新建文件结构摘要 + panels/CLAUDE.md 改动清单。`,
  },
  {
    label: 'B-ipc-sideviews-docs',
    prompt: `你负责【DOC-11】ipc/CLAUDE.md 删 setFocus + 用例数 9 +【DOC-12】ipc-agent-history-contract 用例数 14→18 +【DOC-14】sideViews/CLAUDE.md 用例数统一。先读 \`docs/review-phase2-fix/checklist.md\` 第 10 节 DOC-11/DOC-12/DOC-14 条目。

触碰文件：\`src/ipc/CLAUDE.md\`、\`src/__tests__/ipc-agent-history-contract.test.ts\`（仅头注释）、\`src/features/sideViews/CLAUDE.md\`

步骤：
1. 【DOC-11】\`src/ipc/CLAUDE.md:24\` window.ts 行「七个 wrapper」→「六个 wrapper」，删 \`setFocus\` 描述；测试模式节 \`ipc-window-contract.test.ts（10 用例…）\` → \`（9 用例…）\`（先 Read 该文件确认 window.ts 实际导出数再落笔）
2. 【DOC-12】\`src/ipc/CLAUDE.md\` 测试模式节 \`ipc-agent-history-contract.test.ts（14 用例…）\` → 18；\`src/__tests__/ipc-agent-history-contract.test.ts\` 文件头注释 14 → 18（真值源 \`.claude/test-inventory.md:100\`，先 grep 核实）
3. 【DOC-14】\`src/features/sideViews/CLAUDE.md\` 测试模式节：\`sideBarState.test.ts\` 53→54、\`activityBar.test.tsx\` 38→40（真值源 test-inventory:226-227，先 grep 核实）

完成后报告：五处改动清单。`,
  },
  {
    label: 'C-ledger',
    prompt: `你负责【TE-15】json-schema-library 双 major 债务登记 + 全程登记收口（D12~D20 决策入 ADR、新用例入 test-inventory、新 SEC 编号入根索引）。先读 \`docs/review-phase2-fix/checklist.md\` 第 0 节（D12~D20 决策表）与第 10 节 TE-15 条目。

触碰文件：\`.claude/adr.md\`、\`.claude/test-inventory.md\`、\`.claude/CLAUDE.md\`、\`src/features/hooksConfig/CLAUDE.md\`

步骤：
1. \`.claude/adr.md\` 登记四块：①D12~D20 决策表（照 checklist 第 0 节原文）；②TE-07 执行结果（读 S02 commit message/进度表结论——含任何妥协方案）；③TE-15 债务：「json-schema-library 9.x/11.x 双 major 并存——codemirror-json-schema@0.8.1 锁 9.x（上游约束），主声明 11.6.2；运行时两实例并存无冲突（JSON Schema 校验各自独立），待上游升级消解（TE-15）」；④ADR-0009 FE-31 行登记点链接确认指向 \`src/panels/editor/CLAUDE.md\`（FE-36 语义修订顺带一笔：MAX_PAGES 跨项目全局计数）
2. \`.claude/test-inventory.md\` 登记 S03~S09 执行期新增的测试用例（读各 Stage commit 或 grep 测试文件核实新用例名与计数，同步受影响文件的用例数——真值源职责）
3. \`.claude/CLAUDE.md\` 需求编号索引补三行：SEC-15（shell 白名单 fallback 收窄：双侧失败才回退，单侧拒绝）、SEC-16（set_project_root tokio::Mutex 串行化）、SEC-17（hooks user 层写入后端审计日志）
4. \`src/features/hooksConfig/CLAUDE.md\` schema 节补一句 TE-15 同义引用

纪律：数字/用例名一律以代码与 test-inventory 现文为准，禁凭记忆。
完成后报告：四文件改动清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（全门禁终跑）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
2. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
3. npx tsc --noEmit
4. npx eslint src/
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npm run test:l3
8. npx knip --production
9. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 10 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照真实代码/test-inventory 核实，防文档撒谎。
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
