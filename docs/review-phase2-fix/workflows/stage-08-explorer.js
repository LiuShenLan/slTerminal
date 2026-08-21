// =====================================================================
// Stage 08：explorer/navTree 增强（FE-39 验证、FE-40、FE-41）
// 编排：并行 2（文件零重叠：A=FileTree.tsx；B=useFileTree.ts）；FE-39 零改动仅 verify 断言
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-08.md
// =====================================================================

export const meta = {
  name: 'stage08-explorer',
  description: 'S08 选中滚动跟随 + 已删目录行清理（FE-40/41；FE-39 验证已固化）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 npm test（全量测试 agent 单点跑）；编译级自查用 \`npx tsc --noEmit\`。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A-scroll-follow',
    prompt: `你负责【FE-40】FileTree 虚拟化选中滚动跟随（D20 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 8 节 FE-40 条目（含可照抄的 useLayoutEffect 代码块与全部锚点行号）。

触碰文件：\`src/features/explorer/FileTree.tsx\`、\`src/__tests__/explorer-virtualization.test.tsx\`

步骤：
1. \`src/features/explorer/FileTree.tsx\` 的 visibleRows 计算（约 :646）之后插入 checklist 条目中的 useLayoutEffect 代码块：selectedPath 变化且对应行索引不在 [start, end] 窗口内 → scrollRef scrollTop 定位（ROW_HEIGHT 既有常量 :56）
2. \`src/__tests__/explorer-virtualization.test.tsx\` 增用例：构造大行数树 + mock 容器高度（jsdom clientHeight=0 退化为全量渲染，需按现有「高度测得」用例模式 mock）→ 程序式设视口外 selectedPath → 断言 scrollTop 被设置
3. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + 新增用例名。`,
  },
  {
    label: 'B-stale-dir-row',
    prompt: `你负责【FE-41】refreshSubtreeAt 目标已删空目录行移除（D20 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 8 节 FE-41 条目（含可照抄的 targetMissing 分支代码块）。

触碰文件：\`src/features/explorer/useFileTree.ts\`、\`src/__tests__/use-file-tree.test.ts\`

步骤：
1. \`src/features/explorer/useFileTree.ts\` 的 \`refreshSubtreeAt\`（约 :230-298）：\`const fresh = await loadDirectory(targetPath);\`（约 :258）改为 checklist 条目中的代码——自行 try/catch 直调 readDir 链路区分「目标已删除」（抛错）与「空目录」（返回 []）；目标已删且非根路径时从父层递归移除该目录行并 return true；根被删走原 mergeLayer 空合并路径（现状语义不动）
2. \`loadDirectory\` 本体不动（dirErrors 容错语义保留给其他调用方）
3. \`src/__tests__/use-file-tree.test.ts\` 增用例：vfs 中删除目标目录后触发 refreshSubtreeAt → 断言该目录行从树中消失
4. \`npx tsc --noEmit\` 通过

完成后报告：改动摘要 + 新增用例名。`,
  },
]
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
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 8 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
