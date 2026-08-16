// =====================================================================
// Stage 07 — 浮层统一（OV-01~OV-05）
// 跨边界契约（写死，overlay-base 产出由其余三家引用）：
//   - ConfirmDialog：confirmDialog(opts: { title?: string; message: string;
//     kind?: "warning"|"error"|"info"; confirmText?: string; cancelText?: string;
//     danger?: boolean }): Promise<boolean>（组件路径 src/lib/ConfirmDialog.tsx，
//     经 src/lib/index.ts barrel 导出；挂载点由 overlay-base 负责置入 App.tsx 根部）
//   - toast：toast.show(type: "success"|"warning"|"error", message: string): void
//     （src/lib/toast.tsx，barrel 导出；容器由 overlay-base 置入 App.tsx 根部）
//   - 纯告警场景（无取消语义）用 toast.show 或 confirmDialog 单钮——按各调用点语义定
// fix-loop 调用本 Stage 时 args.constraints 传：无（空串）
// =====================================================================

export const meta = {
  name: 'stage07-overlay',
  description: 'Stage 07: ConfirmDialog/toast 统一浮层 + ask() 全替换 + 菜单规范',
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
【本 Stage 特殊纪律】ConfirmDialog/toast 由 overlay-base agent 并行创建——引用方按脚本头契约 API 写调用，禁止自行新建同名组件；import 不到属正常（并行落盘），最终全量测试时齐备。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑测试）===
phase('并行重构')
const parallelAgents = [
  { label: "overlay-base", prompt: `你负责 OV-01：
新建 src/lib/ConfirmDialog.tsx（UI-801/803）：遮罩 rgba(0,0,0,0.55)（SHADOW_MENU token）；卡片 #1a1a1e 底（SIDEBAR_BG token）+ rgba(255,255,255,0.09) 1px 描边（CONTEXT_MENU_BORDER token）+ 圆角 8px + 阴影 0 8px 32px rgba(0,0,0,0.35)（SIDEBAR_COLORS.contextMenuShadow token）；主按钮 #6e9ff2 底（FOCUS_BORDER token）+ #0c1220 字（ON_ACCENT_FG token），danger 时主按钮 #d9706b 底（ERROR_FG）+ #ece9e4 字（SIDEBAR_FG）；次按钮 #222227 底（SECONDARY_BG token）+ #ece9e4 字；API = confirmDialog(opts): Promise<boolean>（契约见脚本头），ESC/遮罩点击 = false。
新建 src/lib/toast.tsx（UI-804）：右上堆叠容器；单条 = 语义色 12% 底 + 1px 语义描边 + fg-1 文字 + 圆角 8px（success→AGENT_STATUS_USAGE_COLORS.low、warning→GIT_FILE_COLORS.modified、error→ERROR_FG，底色用对应 12% rgba 派生并注释）；自动消失（success 3s/警告 4s/错误 5s——执行期定写入注释）；API = toast.show(type, message)。
两组件挂载点置入 src/App.tsx 根部（App.tsx 仅你触碰）；src/lib/index.ts barrel 同步导出。
新建 src/__tests__/confirm-dialog.test.tsx 与 toast.test.tsx（渲染规格/按钮回调/自动消失 fake timers）。` },
  { label: "overlay-commit-history", prompt: `你负责 OV-02/OV-04 的 commit+history 域：
【OV-02】ask() 调用点替换为 confirmDialog（契约见脚本头）：src/features/commit/commitContextMenu.ts:53 与 :72（回滚/删除确认，danger: true）、src/features/agentHistory/HistorySessionList.tsx:365（删除会话确认，danger: true）；import 自 src/lib barrel，删 ipc/dialog ask 引用。
【OV-04】右键菜单视觉统一（UI-802）：src/features/commit/CommitFileList.tsx 菜单与 src/features/agentHistory/ 菜单渲染处——项 28px 高、圆角 5px、hover #222227（SIDEBAR_COLORS.hover token）、危险项 #d9706b（ERROR_FG token）、边框 0.09（CONTEXT_MENU_BORDER token）、阴影（contextMenuShadow token）；historyContextMenu.ts 为策略层（菜单项定义）不动视觉，仅改渲染组件。
同步更新 src/__tests__/ 本域相关测试（commit-context-menu-ui.test.tsx 等，ask mock 改 confirmDialog mock）。` },
  { label: "overlay-explorer", prompt: `你负责 OV-02/OV-04/OV-05 的 explorer 域：
【OV-02】ask() 调用点替换为 confirmDialog：src/features/explorer/ExplorerPanel.tsx:109、src/features/explorer/FileTree.tsx:307 与 :357（删除确认，danger: true）。
【OV-04】src/features/explorer/FileTree.tsx 右键菜单视觉统一（项 28px/圆角 5px/hover token/危险项 ERROR_FG/边框阴影 token——同 UI-802）。
【OV-05】src/features/explorer/ExplorerPanel.tsx:400-434 错误横幅核对：token 已 Stage 01 换值（ERROR_BANNER_BG/BORDER/FG），关闭 × 图标改 icons.tsx IconClose。
同步更新 src/__tests__/ 本域相关测试。` },
  { label: "overlay-hooks-workspace", prompt: `你负责 OV-02/OV-03/OV-04 的 hooksConfig+workspace 域：
【OV-02】ask() 调用点替换：src/panels/hooksConfig/HooksConfigPanel.tsx:184（未保存修改确认）、src/panels/hooksConfig/useHooksConfig.ts:149（确认）与 :194 与 :201（纯告警——改 toast.show("error", ...) 或单钮 confirmDialog，按语义定写入注释）。
【OV-03】src/workspace/TerminalRenameDialog.tsx 与 src/features/agentHistory/SessionActionDialog.tsx 统一浮层规范（UI-801）：l3 底 SIDEBAR_BG/0.09 描边/圆角 8px/阴影 token/按钮规格（主 FOCUS_BORDER+ON_ACCENT_FG、次 SECONDARY_BG+SIDEBAR_FG）。
【OV-04】src/workspace/PageDockviewHost.tsx 页签右键菜单视觉统一（项 28px/圆角 5px/hover token/危险项 ERROR_FG/边框阴影 token）。
【OV-02 收尾】src/ipc/dialog.ts 删除 ask 导出（open/save 保留——文件对话框原生保留）；grep 全仓确认 ask 引用已清零后删除；src/__tests__/dialog-e2e-hook.test.ts 同步（ask 相关用例改 confirmDialog 或删除，逐用例判断写理由）。
同步更新本域其它相关测试。` },
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/ui-redesign-impl/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
