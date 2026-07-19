// =====================================================================
// Stage 07: 文档同步（CV-DOC-01~03）
// =====================================================================
// 纪律：先读 git log / git diff 再动笔，禁凭记忆写文档
// fix-loop 调用时 args.constraints 传 "本 Stage 只改 *.md 文档，禁止改代码"
// =====================================================================

export const meta = {
  name: 'stage07-docs',
  description: 'Stage 07: commit 视图模块文档同步 + 用例数更新（CV-DOC-01~03）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
本 Stage 只改 *.md 文档，禁止改代码。
铁律：先执行 git log --oneline -10 与 git diff 查看本特性全部实际改动（前 6 个 Stage 的 commit），逐文件对照真实代码后再写文档，禁止凭记忆或凭计划文档描述写实现细节。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "docs-sync",
    prompt: `你负责 CV-DOC-01 / CV-DOC-02 / CV-DOC-03 全部 3 项（文档一致性需单一视角，单 agent）：

【CV-DOC-01】新模块文档
- 新建 src/features/commit/CLAUDE.md：职责 / 状态机 / 分派映射表（状态→panelType+suffix）/ 文件表 / 测试模式（照 src/features/explorer/CLAUDE.md 结构）
- 更新 src/panels/CLAUDE.md：面板类型清单加 gitshow + diff；架构决策补两条（gitshow 只读 CM / diff 占位对齐+滚动同步+双侧 gutter）；文件表追加；测试模式补 diff-alignment / diff-panel / gitshow-panel 测试文件

【CV-DOC-02】既有模块文档更新
- src/ipc/CLAUDE.md：git.ts 行加 git_file_at_head 命令
- src-tauri/src/git/CLAUDE.md：新增 git_file_at_head 命令说明（含 HEAD 不存在错误约定）、GitStatusEntry.oldPath、recurse_untracked_dirs、测试数更新
- src/features/sideViews/CLAUDE.md：sideViewDefs 注册视图从 2 条改为 3 条（+commit）
- src/workspace/CLAUDE.md：titleManager 段落补 suffix 语义（后缀保留 + findExistingEditor suffix 匹配规则）

【CV-DOC-03】根文档更新
- .claude/CLAUDE.md：模块表加 src/features/commit 行；panels 行描述更新（5 面板）
- .claude/test-inventory.md：更新各层级用例数——L2 按本 Stage 全量测试的 npm test 统计行取数；L1/L3/L4 按源文件静态计数（L1: grep \`#[test]\` 计数；L3/L4: grep \`it(\` 计数），不依赖 cargo/wdio 实跑输出，不凭空填`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
报告最后附上 npm test 的最终用例统计行（Test Files / Tests 数字），供文档 agent 核对 test-inventory。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/commit-view/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言必须对照真实代码核实，防文档撒谎。
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
