// =====================================================================
// Stage 06 杂项收敛（FE-10/11/18/26/27）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// 契约（写死）：全局字体栈 = "JetBrains Mono", "Cascadia Mono", Consolas,
//   "Microsoft YaHei UI", monospace（UI-201 唯一真值）
// =====================================================================

export const meta = {
  name: 'fix-stage06-misc-frontend',
  description: 'Stage 06 杂项收敛：字体栈统一 + GitShow 告警图标化 + 关窗 unlisten 兜底 + 焦点环接管',
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
并行纪律：不跑资源共享型测试——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  { label: "app-misc", prompt: `你负责 FE-10/26：
【FE-10】src/App.tsx:214-216 启动加载页：fontFamily "monospace" 改全局字体栈（见脚本头契约）；说明文字色由 INPUT_BORDER 误用改 DIM_FG（fg-3 档）。
【FE-26】registerCloseHandler 清理函数 unlisten Promise 未兜 reject：落点优先 src/ipc/window.ts:56-69 内部（unlisten 链尾补 .catch(() => {})——窗口已销毁场景吞错）；App.tsx:171 调用侧若因此无需改动则不动。
测试同步：加载页相关断言与 closeHandler 用例就近增补（App 相关用例在 src/__tests__/app.test.tsx；无合适位置则新建测试文件）。` },
  { label: "boundary-font", prompt: `你负责 FE-11：
【FE-11】src/lib/ErrorBoundary.tsx:63,125 两处 fontFamily: "monospace" 改全局字体栈（见脚本头契约；全屏 variant 与 inline variant 各一处）。
测试同步：src/__tests__/error-boundary.test.tsx 样式断言同步。` },
  { label: "gitshow-icon", prompt: `你负责 FE-18：
【FE-18】src/panels/gitshow/GitShowPanel.tsx:133 大文件警告的 ⚠ 字符改 lucide 图标：src/lib/icons.tsx 新增导出（lucide-react 三角告警图标——先查 node_modules/lucide-react 实际导出名，TriangleAlert 或别名 AlertTriangle；命名 IconAlertTriangle 或 IconTriangleAlert 与现有 IconXxx 风格一致）；GitShowPanel 改引用该图标组件（13px，色经语义 token，禁硬编码）。
测试同步：src/__tests__/gitshow-panel.test.tsx 大文件警告用例断言改 svg 图标存在性。` },
  { label: "explorer-outline", prompt: `你负责 FE-27：
【FE-27】src/features/explorer/ExplorerPanel.tsx:446-452 文件树容器删 outline: "none"（tabIndex={-1} 保留；全局 :focus-visible 环接管——鼠标点击不匹配 :focus-visible，视觉无变化；键盘编程聚焦时可见，UI-808）。
测试同步：src/__tests__/explorer-focus.test.tsx 增容器无 outline 抑制断言。` },
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
