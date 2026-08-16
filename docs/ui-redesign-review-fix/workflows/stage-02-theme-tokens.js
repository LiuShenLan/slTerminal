// =====================================================================
// Stage 02 配色单点收敛（FE-07/08/20/21）— ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）
// 跨 agent 契约（写死，执行 agent 不各自推断）：
//   1. ui 段新标量：titlebarCloseHover: "#c04747"（types.ts UiTokens 加槽位 +
//      linear.ts 加值 + colors.ts facade 导出 TITLEBAR_CLOSE_HOVER_BG）
//   2. ROOT_CSS_VARS 扩 4 键（colors.ts）：--sl-focus-border ← ui.focusBorder（#6e9ff2）、
//      --sl-scrollbar-slider / --sl-scrollbar-slider-hover / --sl-scrollbar-slider-active
//      ← terminal.scrollbarSliderBackground/Hover/Active（rgba(255,255,255,0.10/0.20/0.28)）
//      ——值与现硬编码逐字相同，零视觉变化
//   3. 测试计数同步：scheme-registry.test.ts 标量 26→27；colors.test.ts ROOT_CSS_VARS
//      键集合 2→6 + token 集合；theme/index.ts:3 注释 34→35（归 theme-token agent）
// =====================================================================

export const meta = {
  name: 'fix-stage02-theme-tokens',
  description: 'Stage 02 配色单点收敛：titlebarCloseHover token + ROOT_CSS_VARS 扩键 + TitleBar 订阅拆分 + 文件夹 gitStatus',
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

// === Phase 1: 并行修复（agent 间文件零重叠；token/键名见脚本头契约）===
phase('并行修复')
const parallelAgents = [
  { label: "theme-token", prompt: `你负责 FE-07/08 的方案侧：
【FE-07】src/theme/schemes/types.ts UiTokens 加槽位 titlebarCloseHover（含区域级消费注释：自绘标题栏关闭钮 hover 底，UI-301 定值）→ schemes/linear.ts ui 段加 titlebarCloseHover: "#c04747" → colors.ts facade 导出 TITLEBAR_CLOSE_HOVER_BG。
【FE-08】colors.ts ROOT_CSS_VARS 扩 4 键：--sl-focus-border ← ui.focusBorder；--sl-scrollbar-slider / --sl-scrollbar-slider-hover / --sl-scrollbar-slider-active ← terminal.scrollbarSliderBackground/Hover/Active（取 getActive() 两段，值见脚本头契约）。
测试同步：src/__tests__/scheme-registry.test.ts 标量计数 26→27；src/__tests__/colors.test.ts token 集合 + ROOT_CSS_VARS 键集合 2→6 + 新键值映射断言；src/theme/index.ts:3 注释计数 34→35。` },
  { label: "titlebar-fix", prompt: `你负责 FE-07 组件侧 + FE-21：
【FE-07】src/features/titleBar/TitleBar.tsx:22 关闭 hover 硬编码 "#c04747" 改引用 facade token TITLEBAR_CLOSE_HOVER_BG（自 src/theme import）。
【FE-21】同文件 :50-67 订阅整个 projects 对象改轻量 selector（activePageId + 按 activePageId 推导标题的窄订阅，或标题派生上提 store selector——按最小改动定；行为不变：标题文案与切换响应不变）。
测试同步：src/__tests__/title-bar.test.tsx 增「无关项目变更不触发重渲染/标题不变」用例。` },
  { label: "appcss-vars", prompt: `你负责 FE-08 的样式侧：
【FE-08】src/App.css 四处硬编码改 var() 引用：:59 rgba(255,255,255,0.10) → var(--sl-scrollbar-slider)；:63 rgba(255,255,255,0.20) → var(--sl-scrollbar-slider-hover)；:66 rgba(255,255,255,0.28) → var(--sl-scrollbar-slider-active)；:75 #6e9ff2 → var(--sl-focus-border)。:5 与 :72-73 的「ROOT_CSS_VARS 仅两键，var() 不可行」注释重写（键已扩，如实描述四键来源）。键名见脚本头契约（theme-token agent 同 Stage 在 colors.ts 落地，你只管用）。` },
  { label: "fileicon-git", prompt: `你负责 FE-20：
【FE-20】src/features/explorer/FileIcon.tsx:104-115 文件夹分支忽略 gitStatus：isDir 分支 color 由 statusColorMap[gitStatus] ?? EXPLORER_COLORS.fg 取值（与文件分支同一映射，无状态回退 fg）。
测试同步：src/__tests__/file-icon.test.tsx 增文件夹 + gitStatus 着色用例（含无状态回退分支）。` },
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
