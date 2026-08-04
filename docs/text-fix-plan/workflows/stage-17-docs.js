// =====================================================================
// Stage 17 文档同步
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-17.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改文档（.claude/ 与子路径 CLAUDE.md），零代码变更；e2e-tests/CLAUDE.md 归 doc-inventory 独占，doc-modules 不碰
// =====================================================================

export const meta = {
  name: 'stage17-docs',
  description: 'test-inventory 全量校正 + 豁免清单 + 定位声明同步',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1——本 Stage 不涉，仅作提示
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改文档，零代码变更（门禁为收尾跑测试确认零代码副作用）。文档必须反映所有代码 Stage 完成后的最终状态——用例数以各 Stage 实际变更后为准（静态 grep 口径：逐域 grep it(/#[test] 计数）。并行 agent 文件零重叠（doc-inventory 独占 .claude/test-inventory.md、e2e-tests/CLAUDE.md、src-tauri/src/pty/CLAUDE.md；doc-modules 碰其余子路径 CLAUDE.md + 根）。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'doc-inventory',
    prompt: `你负责 DOC-01、DOC-02、DOC-03，触碰文件：.claude/test-inventory.md、e2e-tests/CLAUDE.md、src-tauri/src/pty/CLAUDE.md（豁免段收编 Stage 02 草稿）。逐 ID 对照 checklist 原文实施：

【DOC-01】既定豁免清单文档化。范围：reader_loop 残余不可抽分支（PTY-12 产出）、spawn_conpty_child 纯 Win32 调用部分（PTY-08 产出）、lib.rs run()、ActivityBar 拖拽 mock 理想化（SVC-14 产出）、E2E_ENABLED=false 生产分支（IHE-04 互补）、L3 WebGL/mouse tracking（15-#16）、L4 真实 OS 按键、HTML postMessage 真实 WebView2 行为。在 .claude/test-inventory.md + 对应模块 CLAUDE.md 统一登记豁免表（项目/豁免原因/当前兜底层级），与 00-summary 5.3 表对齐。src-tauri/src/pty/CLAUDE.md 收编 Stage 02 的豁免标注草稿。

【DOC-02】定位声明（半端到端 / 网格状态 / helper 契约）。范围：①L3 = 网格状态正确性非渲染正确性（E2E-04 产出）；②L4 键盘/拖拽/恢复 = 半端到端/部分端到端（E2E-11 产出）；③L2 jsdom postMessage 模拟（IHE-03 产出）；④term.input 间接验证；⑤app.test/e2e-create-project = E2E helper 行为契约；⑥editor.test.tsx 浅层定位。e2e-tests/CLAUDE.md + test/terminal README 或文件头 + test-inventory.md 补定位声明。

【DOC-03】test-inventory.md 全量校正。①stale 条目清理（hooks 模块"notification 权限声明"等）；②各 Stage 完成后同步用例数（新增/删除/拆分/改名全量反映——以磁盘实际 grep 计数为准）；③登记 DOC-01 豁免表与 DOC-02 定位声明。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'doc-modules',
    prompt: `你负责 DOC-04，触碰文件：除 .claude/test-inventory.md、e2e-tests/CLAUDE.md、src-tauri/src/pty/CLAUDE.md 外的其余子路径 CLAUDE.md + 根 .claude/CLAUDE.md（如需）。子路径清单（16 个 claudeMdFiles，config.json 已补 claude_history）：.claude/CLAUDE.md、src/ipc/CLAUDE.md、src/panels/CLAUDE.md、src/stores/CLAUDE.md、src/workspace/CLAUDE.md、src/lib/CLAUDE.md、src/features/explorer/CLAUDE.md、src/features/fileViewers/CLAUDE.md、src/features/shortcuts/CLAUDE.md、src/features/sidebar/CLAUDE.md、src-tauri/src/hooks/CLAUDE.md、src-tauri/src/notify/CLAUDE.md、src-tauri/src/fs/CLAUDE.md、src-tauri/src/git/CLAUDE.md、src-tauri/src/claude_history/CLAUDE.md。逐 ID 对照 checklist 原文实施：

【DOC-04】子路径 CLAUDE.md 测试模式章节同步。测试拆分（GIT-12/SVC-14/E2E-09）、新增测试文件（HKC-08/IHE-02/IHE-06 helper）、测试模式变化（命令层 block_on 模式/EventEmitter trait/ScanRootGuard）、git CLI 最低版本声明（GIT-08 产出）同步到对应模块 CLAUDE.md。git 模块 CLAUDE.md 补 git CLI 最低版本声明（Stage 03 GIT-08 产出）；claude_history/CLAUDE.md 补 ScanRootGuard 测试模式（Stage 05 HFN-06 产出）；notify/CLAUDE.md 补 EventEmitter trait（Stage 05 HFN-03 产出）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（收尾确认文档 Stage 零代码副作用）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npm test
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-17.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage17 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-17.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照当前代码核实不撒谎。
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
