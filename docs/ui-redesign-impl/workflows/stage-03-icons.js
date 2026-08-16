// =====================================================================
// Stage 03 — 图标体系（IC-01~IC-09）
// 跨 agent 契约（写死，并行 agent 不各自推断）：
//   - StatusDot API：props { status: "working"|"attention"|"done"|"error", size?: number（默认 7） }
//     色映射 working→#86bb7a / attention→#d6b25e / done→#6b675f / error→#d9706b
//     色值经 theme/colors.ts facade token 引用（AGENT_STATUS_USAGE_COLORS.low=working 绿、
//     GIT_FILE_COLORS.modified=attention 黄、ui.placeholderFg=done 灰、ui.errorFg=error 红——
//     icon-base agent 定稿 token 取用并写入组件注释，引用方只管 props）
//   - icons.tsx 集中导出（lucide-react 封装，统一 15px/1.5px 描边/currentColor，紧凑处 12-13px）：
//     IconNav（导航树）/IconFiles（文件）/IconCommit/IconConfig（配置齿轮）/IconChevronRight/
//     IconChevronDown/IconRefresh/IconSearch/IconHistory（时钟）/IconClose/IconMin/IconMax/
//     IconCloseWin/IconPlus/IconFolder（描边款）/IconEmptyBox（空态）——名称以 icon-base 落盘为准，
//     引用方 import 前先 Read src/lib/icons.tsx 确认导出名（允许 Read，禁止自行新增导出）
//   - tabIcon 链路：updateParameters 键 tabIcon 改 tabStatus（status 字符串）；tabLogo 不动
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage03-icons',
  description: 'Stage 03: lucide 线性图标体系 + 状态圆点替代 emoji + FileIcon 六色盘',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/ui-redesign-impl/checklist.md 对应 ID 条目（先读再动手）。
【本 Stage 特殊纪律】icons.tsx/StatusDot.tsx 由 icon-base agent 并行创建——引用前先 Read 确认导出名与 props，Read 不到就按脚本头契约写，禁止自行新建同名组件。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "icon-base", prompt: `你负责 IC-01/IC-02：
【IC-01】执行 npm install lucide-react；新建 src/lib/icons.tsx——集中封装 lucide-react 图标，统一默认 15px、strokeWidth 1.5、currentColor（紧凑处允许调用方传 size 12-13）；导出清单见脚本头契约（IconNav/IconFiles/IconCommit/IconConfig/IconChevronRight/IconChevronDown/IconRefresh/IconSearch/IconHistory/IconClose/IconMin/IconMax/IconCloseWin/IconPlus/IconFolder/IconEmptyBox，lucide 选型对照：导航=FolderTree 或 ListTree、文件=Folder、Commit=GitBranch 或 GitCommitHorizontal、配置=Settings 齿轮、时钟=Clock、空态=Inbox 或 FolderOpen——执行期对照 lucide-react 导出实查定名）；src/lib/index.ts barrel 同步导出。
【IC-02】新建 src/lib/StatusDot.tsx：props { status: "working"|"attention"|"done"|"error", size?: number 默认 7 }；圆形 div，色映射 working→绿/attention→黄/done→灰/error→红（色值经 theme/colors.ts facade token 引用——绿=AGENT_STATUS_USAGE_COLORS.low、黄=GIT_FILE_COLORS.modified、灰=PLACEHOLDER_FG、红=ERROR_FG，取用后写入组件注释说明映射依据）；无描边/光晕/动画；src/lib/index.ts barrel 同步导出。` },
  { label: "file-icon", prompt: `你负责 IC-04：
【IC-04】重构 src/features/explorer/FileIcon.tsx：文件夹=描边款 SVG（取 icons.tsx IconFolder）；文件=描边+小色块款自绘 SVG，色限六色盘 #7fa8e8/#d6b25e/#93b573/#d9706b/#b48ce0/#6fbfc4；扩展现有扩展名分组逻辑（ts/js/rs/py/json/md/html/css/配置/默认）映射到六色（执行期定映射表写入组件注释：如 ts/tsx→蓝、js/jsx→黄、py→绿、rs→红、json→黄、md→紫、html→青、css→蓝、配置→灰青、默认→灰）；gitStatus 着色保留（GIT_FILE_COLORS token 逻辑不变）；props 签名保持兼容（FileTree 调用方零改动——先 Read FileTree.tsx 与 ExplorerPanel 确认全部消费点）。
同步更新 src/__tests__/file-icon.test.tsx（emoji 断言改 SVG/色系断言）。` },
  { label: "status-chain", prompt: `你负责 IC-03/IC-08（状态行 emoji 部分）：
【IC-03】状态 emoji 链路改 StatusDot（F3 映射逻辑零改动，仅渲染层）：
1. src/lib/agentStatus.ts:20-33 STATUS_EMOJI 常量删除；status 类型（working/attention/done/error）与 eventToStatus 委托保留不动；
2. src/panels/terminal/useCommandDetection.ts:80-83：onTabStateChange 载荷 icon 字段改 status（值=attention 等 status 字符串），TabState 类型同步；
3. src/panels/terminal/useXterm.ts:409-414：同上（icon→status）；
4. src/panels/terminal/TerminalPanel.tsx:84-102：handleTabStateChange 写 updateParameters 键 tabIcon 改 tabStatus（status 字符串）；
5. src/workspace/PageDockviewHost.tsx：DefaultTab 的 tabIcon 渲染分支改读 tabStatus 渲染 StatusDot（订阅 onDidParametersChange 逻辑不变；原 emoji span 分支与 img URL 分支——保留 img 分支语义若存在，先 Read PageDockviewHost.tsx:239-303 确认两分支现状，tabLogo 16x16 img 分支绝对不动）；
6. src/features/agentStatus/AgentStatusRow.tsx:86 与 src/features/agentHistory/HistorySessionRow.tsx:51,86：状态 emoji 改 StatusDot。
同步更新状态链路相关测试（grep tabIcon/⚡/🟡/✅/❌ 于 src/__tests__/ 命中文件，断言改 tabStatus/StatusDot 形态）。` },
  { label: "misc-emoji", prompt: `你负责 IC-05/IC-06/IC-07/IC-08/IC-09：
【IC-05】树箭头 ▶/▼ 改 icons.tsx 的 IconChevronRight/IconChevronDown（12px fg-3——色经 SIDEBAR_COLORS/EXPLORER_COLORS arrow 槽位 token）：src/features/sidebar/SidebarTree.tsx:174,256、src/features/explorer/FileTree.tsx（含 :211 ⏳ 改线性 spinner 或三点——执行期定，写入注释）、src/features/agentStatus/AgentStatusView.tsx 折叠区箭头、src/features/agentHistory/HistorySessionList.tsx:441 组标题箭头。
【IC-06】src/features/sideViews/sideViewDefs.ts 4 视图 icon 字段 emoji（📋📁🔀🤖）改 icons.tsx 组件（SideViewDef 类型 icon 字段形态随之改——先 Read sideViewRegistry.ts 与 ActivityBar.tsx 确认 icon 渲染方式，组件化后 ActivityBar 渲染同步适配；色 fg-3 默认、hover fg-1、active accentFg token）。
【IC-07】src/features/notifications/useAgentNotifications.ts:41-43 CATEGORY_EMOJI 删除（通知标题纯文本——直接拼接类别名，无图标字符）。
【IC-08】src/features/agentHistory/HistorySessionRow.tsx:108 ✗ 改 icons.tsx IconClose（12px，色 ERROR_FG）——若 status-chain agent 已改该文件行区域，你只改 ✗ 字符处，其余不动。
【IC-09】新建 src/__tests__/emoji-scan.test.ts：遍历读 src/ 全部 .ts/.tsx 源文件（排除 __tests__ 目录自身），断言无装饰 emoji 字面量——集合：📁📂📋🤖🌿⭐🟠⚡✅❌🕐💾📄✏️🗑➕🔍⚙️🔄🖖📜🐍📝🌐🎨📦⏳🔐✗▶▼；白名单机制：仅允许逐文件显式登记（初始为空），命中即 fail 并报告文件与字符。` },
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
6. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
