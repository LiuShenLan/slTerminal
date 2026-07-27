// =====================================================================
// Stage 01：文档对账与行为文档化（phase1-fix）
// =====================================================================
// 配套：checklist docs/hooks-dev/phase1-fix/checklist.md（PF-DOC-01 ~ 06）
//       verify   docs/hooks-dev/phase1-fix/workflows/verify/stage-01.md
// fix-loop 调用时 args.constraints 应传：
//   "纯文档修正 Stage：禁改任何运行时代码（src/lib/claudeStatus.ts 仅允许顶部注释）；禁止改动 docs/hooks-dev/phase1/ 历史文档"
// =====================================================================

export const meta = {
  name: 'stage1-docs-fix',
  description: 'Stage 01：test-inventory 对账 + 模块 CLAUDE.md 修正 + 中断行为/信号瞬态文档化 + config.json 补登记',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】本 Stage 为纯文档修正：禁改任何运行时代码（src/lib/claudeStatus.ts 仅允许顶部注释）；禁止改动 docs/hooks-dev/phase1/ 历史文档。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'doc-inventory',
    prompt: `你负责 PF-DOC-01（.claude/test-inventory.md 用例数对账）。先读 docs/hooks-dev/phase1-fix/checklist.md 的 PF-DOC-01 条目与「基线数字」表。
只允许修改一个文件：.claude/test-inventory.md。
步骤：
1. 全量实查（禁照抄基线）：L1 全部在列文件逐一跑 grep -c '#\\[test\\]' <file>（清单见 test-inventory.md L1 表，另加 hooks/signal.rs 与 hooks/watcher.rs 两个新文件）；L2 跑 grep -cE '^\\s*(it|test)\\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx 求和；L3 跑 grep -cE '^\\s*(it|test)\\(' test/terminal/*.test.ts 求和。Windows 下可用 git bash 或 PowerShell 等价命令。
2. 按实查修正：
   - hooks/mod.rs 行：用例 10 → 8；覆盖描述改如实（4 条 DTO serde camelCase + 4 条 parse_signal_file 冒烟；注明全分支在 signal.rs、watcher 生命周期在 watcher.rs）
   - hooks/inject.rs 行：12 → 20；覆盖描述按实际修正
   - 新增 hooks/signal.rs 行（9：parse_signal_file 全分支 + serde 往返）与 hooks/watcher.rs 行（6：is_signal_file ×4 + watcher 生命周期 ×2），插入位置与现有 hooks 行相邻
   - pty/spawn.rs 行：29 → 28
   - L1 标题行（约第 9 行）：文件数 13 → 15、总计按实查求和
   - 全量总计行（约第 5 行）= L1+L2+L3+E2E（E2E 保持 17 不实查），日期更新为 2026-07-27
3. 基线参考（仅核对用，实查优先）：hooks 8/9/6/20 总 43、spawn.rs 28、L1 294、L2 1415、L3 116、全量 1842。若实查与基线不符，以实查为准并在报告中明确说明差异。
完成后报告：每个文件的实查计数清单 + 修改行摘要。`,
  },
  {
    label: 'doc-module',
    prompt: `你负责 PF-DOC-02 ~ PF-DOC-06。先读 docs/hooks-dev/phase1-fix/checklist.md 对应条目（PF-DOC-03 为并入留痕项，无独立改动，不触碰任何文件）。
只允许修改以下 4 个文件：
1. src-tauri/src/hooks/CLAUDE.md（PF-DOC-02 + PF-DOC-04）：
   - 测试分布表改 mod 8 / signal 9 / watcher 6 / inject 20，总计 41 → 43，各行覆盖描述与实查一致
   - 架构决策节补「信号文件瞬态特性 + dev 注入路径」段，三要素：① process_signal_file 处理后即删（signal.rs:49-79）+ debounce 50ms → 目录常态为空是设计行为，观察需用文件系统监视工具；② dev 注入路径：npm run tauri dev 启动后 devtools 控制台执行 await window.__slterm_e2e_injectHooks()（e2e-tests/helpers.ts:296-300）；③ 状态查询/卸载：__slterm_e2e_getHookInjectionStatus() / __slterm_e2e_uninstallHooks()
2. src/panels/CLAUDE.md（PF-DOC-05）：「F3 页签四态指示」节补「中断场景已知行为」段，三要素：① Ctrl+C 中断时 Claude Code 不发射任何 hook 事件（Stop=完成响应、StopFailure=API 错误，docs/hooks/D1/01-hooks-official-docs.md:36-37）→ 页签滞留 ⚡；② 下一事件（UserPromptSubmit/Stop 等）覆盖自愈；③ 中断回提示符约 60s 无操作 → idle_prompt Notification → 自动转 🟡（docs/hooks/D1/01-hooks-official-docs.md:163）
3. src/lib/claudeStatus.ts（PF-DOC-05）：仅顶部注释追加假设记录（禁动任何代码）——Ctrl+C 中断无 hook 事件，working 无出边为已知行为，依赖下一事件覆盖或 idle_prompt 约 60s 衰减
4. .claude/skills/systematic-changes-plan/config.json（PF-DOC-06）：claudeMdFiles 数组追加 "src-tauri/src/hooks/CLAUDE.md"（插入 src-tauri/src/notify/CLAUDE.md 附近），保持 JSON 合法（改完用 node -e 跑 JSON.parse 验证）
完成后报告：每文件改动摘要。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
// 豁免说明：本 Stage 无运行时行为变更（文档 + 单行注释），门禁仅 tsc + eslint
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/lib/claudeStatus.ts
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase1-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, testResult, verifyResult }
