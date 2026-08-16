// =====================================================================
// Stage 04 — 自绘标题栏（TB-01~TB-06）
// 跨 agent 契约（写死）：
//   - ipc/window.ts 新增导出签名：minimizeWindow(): Promise<void> /
//     toggleMaximizeWindow(): Promise<void> / closeWindow(): Promise<void>
//   - TitleBar 组件路径 src/features/titleBar/TitleBar.tsx，无 props（自订阅 stores）
//   - 标题栏高度 34px、背景 TITLEBAR_BG token、底部发丝线 SEPARATOR_BG
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage04-titlebar',
  description: 'Stage 04: 自绘 34px 一体化标题栏（decorations:false + 窗口控制）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "titlebar-ui", prompt: `你负责 TB-01/TB-02/TB-04/TB-05：
【TB-01】src-tauri/tauri.conf.json windows 段新增 "decorations": false（其余键不动）。
【TB-02】新建 src/features/titleBar/TitleBar.tsx（无 props，自订阅 stores）：高 34px、背景 TITLEBAR_BG（theme/colors.ts token）+ 底部 1px 发丝线（SEPARATOR_BG token）；左段 app 标识（小 logo+slTerminal 文字 12px，fg-3 DIM_FG）；中段标题——活跃项目名加粗（500）+ 分隔「/」+ 活跃页面名（数据源自 src/stores/projects——先 Read 该 store 确认活跃项目/页面 selector，无现成 selector 时用现有 state 推导，禁止改 store）；右段三个自绘窗口钮（38x26，icons.tsx 的 IconMin/IconMax/IconCloseWin 12px，hover 底 #222227 即 ui.secondaryBg token，关闭钮 hover 底 #c04747——此色无 token 槽位，本组件内常量定义并注释「关闭危险色，设计定值」）；三钮点击调用 ipc/window 的 minimizeWindow/toggleMaximizeWindow/closeWindow（契约见脚本头，import 自 src/ipc/window）。
【TB-04】标题栏左/中段容器加 data-tauri-drag-region（三钮所在右段排除——按钮置于 drag region 外层或加 no-drag 样式，确保可点击）；中段双击调用 toggleMaximizeWindow。
【TB-05】src/App.tsx：ready 后骨架改列向 flex 列——首行 TitleBar、其余为 Workspace（原三栏结构包一层，先 Read App.tsx:34-234 确认现状再改，启动加载页不动）。` },
  { label: "titlebar-ipc", prompt: `你负责 TB-03/TB-06：
【TB-03】src/ipc/window.ts 新增三个 wrapper（照文件内既有 getCurrentWindow 模式）：minimizeWindow()/toggleMaximizeWindow()/closeWindow()，均 Promise<void>；closeWindow 实现 = getCurrentWindow().close()（触发 onCloseRequested，复用 P1-19 关窗杀 PTY 链路，禁止用 process.exit 或 destroy）；中文注释写明该链路依据。
【TB-06】新建 src/__tests__/title-bar.test.tsx：mock src/ipc/window（vi.mock 三 wrapper）与 stores/projects 种子数据；断言——三段结构渲染（标识/项目名·页面名/三钮）、点击三钮分别调用对应 wrapper 一次、中段 dblclick 调 toggleMaximizeWindow、项目页面名按 store 种子显示、容器含 data-tauri-drag-region 属性。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npx vite build
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
