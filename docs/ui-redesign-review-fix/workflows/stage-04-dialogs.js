// =====================================================================
// Stage 04 对话框与通知形态（FE-12/13/14/28/29）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// 红线（写死）：ConfirmDialog 的 data-e2e="confirm-ok"/"confirm-cancel"/"confirm-dialog-mask"
//   选择器零变更——e2e-tests/history.e2e.ts:602 依赖
// =====================================================================

export const meta = {
  name: 'fix-stage04-dialogs',
  description: 'Stage 04 对话框与通知形态：圆角档收敛 + ConfirmDialog 焦点陷阱 + ToastHost aria-live',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/ui-redesign-review-fix/checklist.md 对应 ID 条目（先读再动手）。
红线：ConfirmDialog 的 data-e2e 选择器（confirm-ok/confirm-cancel/confirm-dialog-mask）零变更。
并行纪律：不跑资源共享型测试——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  { label: "confirm-dialog", prompt: `你负责 FE-12/28：
【FE-12】src/lib/ConfirmDialog.tsx：主/次按钮 borderRadius 4 改 6（UI-306 按钮档）；删次按钮 :162 的 border: 1px solid CONTEXT_MENU_BORDER（UI-803 只规定底色/字色）。
【FE-28】同文件焦点管理：挂载后确认按钮 focus()（或 autoFocus）；Tab/Shift+Tab 在取消/确认两钮间循环（焦点陷阱，不逃出弹窗）；Enter 经按钮原生提交确认、Esc/遮罩既有语义不动。data-e2e 选择器零变更（红线见 PREAMBLE）。
测试同步：src/__tests__/confirm-dialog.test.tsx 视觉断言同步（圆角/无次钮描边）+ 增挂载聚焦/Enter 确认/Tab 循环用例。` },
  { label: "rename-dialog", prompt: `你负责 FE-13：
【FE-13】src/workspace/TerminalRenameDialog.tsx：:141 输入框 borderRadius 4 改 8（UI-306 输入框档）；:173,:188 确定/取消钮 4 改 6。
测试同步：src/__tests__/terminal-rename-dialog.test.tsx 视觉断言同步。` },
  { label: "session-dialog", prompt: `你负责 FE-14：
【FE-14】src/features/agentHistory/SessionActionDialog.tsx:122,144 按钮 borderRadius 4 改 6。
测试同步：src/__tests__/agent-history-action-dialog.test.tsx 视觉断言同步。` },
  { label: "toast-a11y", prompt: `你负责 FE-29：
【FE-29】src/lib/toast.tsx:93-125 ToastHost 容器加 role="status" aria-live="polite"。
测试同步：src/__tests__/toast.test.tsx 增属性断言。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
