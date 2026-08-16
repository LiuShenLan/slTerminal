// =====================================================================
// Stage 09 规范修订与文档同步（SPEC-01/02 + DOC-01~11 + VER-01 注记）
// — ui-redesign-review-fix
// =====================================================================
// fix-loop 调用本 Stage 时 args.constraints 传：
//   「本 Stage 只改文档与代码注释（docs/、各 CLAUDE.md、.claude/test-inventory.md、
//     linear.ts 文件头注释、App.css:9 注释），禁止改逻辑代码」
// 规范修订口径（写死，照 checklist SPEC-01/02 原文）：
//   SPEC-01：UI-204 与 design.md 阶梯表「编辑器/终端 12.5–13px」改
//     「12.5–13px 为设计基准；终端/编辑器内容区默认 14px（用户 Ctrl+Wheel 可调 8–32）为登记例外」
//   SPEC-02：UI-305 与 design.md「树行 28」改「导航树行 28/会话行 30；
//     文件树（explorer）行 24px——紧凑列表档」
// =====================================================================

export const meta = {
  name: 'fix-stage09-spec-docs',
  description: 'Stage 09 规范修订与文档同步：字号/行高规范登记 + 文档失实修正 + 例外指向收敛 + 用例清单同步',
  phases: [
    { title: '并行修订' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/ui-redesign-review-fix/checklist.md 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】本 Stage 只改文档与代码注释（docs/、各 CLAUDE.md、.claude/test-inventory.md、linear.ts 文件头注释、App.css:9 注释），禁止改逻辑代码。
并行纪律：不跑资源共享型测试——只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行修订（agent 间文件零重叠）===
phase('并行修订')
const parallelAgents = [
  { label: "spec-revise", prompt: `你负责 SPEC-01/02 + DOC-10 的 explorer 部分：
【SPEC-01】docs/ui-redesign/requirements.md:30（UI-204 行）与 docs/ui-redesign/design.md:106-115（字号阶梯表「编辑器/终端 12.5–13px」行）按脚本头口径修订；src/App.css:9 注释口径同步（组件 chrome 字号阶梯不变，内容区默认 14px 例外一句注记）。代码不动（fontSize.ts:17 / terminal/theme.ts:12 / useCodeMirror.ts:139,296 / main.tsx:30 保持 14）。
【SPEC-02】requirements.md:42（UI-305 行）与 design.md:112,121 按脚本头口径修订（导航树 28/会话行 30 不变，文件树 24px 档登记）；src/features/explorer/CLAUDE.md 文件表 FileTree 行补行高 24 档登记。代码不动（FileTree.tsx:202 保持 24）。
【DOC-10 explorer 部分】src/features/explorer/CLAUDE.md 补 FE-27 注记（文件树容器 outline 抑制已删，焦点环由全局 :focus-visible 接管，UI-808）。` },
  { label: "claudemd-a", prompt: `你负责 DOC-03/04/05/06：
【DOC-03】src/stores/CLAUDE.md:36 侧栏默认态改三槽 nav/explorer/commit（对照 src/features/sideViews/sideBarState.ts:33-42 现状核实后写，DEFAULT_OPEN.top="nav"），删 projects/agent-status 四槽描述。
【DOC-04】src/features/shortcuts/CLAUDE.md:126 hooks 配置入口改「活动栏底部配置钮 → openHooksConfigFromActivityBar」（src/features/hooksConfig/openHooksConfig.ts），删侧栏右键菜单/SidebarTree 引用。
【DOC-05】src/ipc/CLAUDE.md:24 「六个 wrapper」改「七个 wrapper」（对照 src/ipc/window.ts 现状：onFocusChanged/requestUserAttention/setFocus/registerCloseHandler/minimizeWindow/toggleMaximizeWindow/closeWindow）。
【DOC-06】src/panels/CLAUDE.md:270 附近 index.ts 文件表补 HooksConfigPanel 导出行（对照 src/panels/index.ts 现状）。` },
  { label: "claudemd-b", prompt: `你负责 DOC-07/08：
【DOC-07】src/theme/CLAUDE.md:64 与 src/theme/schemes/linear.ts:9 的 tauri.conf.json:20 改 :21（一手证据：src-tauri/tauri.conf.json 的 backgroundColor 实在第 21 行——先 Read 核实再改；index.html:10 与 main.tsx:28 已核实无误不动）。linear.ts 只改文件头注释。
【DOC-08】src/workspace/CLAUDE.md:44 附近 index.ts 文件表导出项补全（对照 src/workspace/index.ts 现状：PANEL_TERMINAL/PANEL_EDITOR/PANEL_HTML_VIEWER/FILE_PANEL_TYPES/isValidPanelType/isAlwaysRenderPanel 等）；pageApis.ts 行补 FE-09 新增导出 findPanelForSession/findPageIdForPanelId + VER-01 注记（F8 段「panel.component 不存在」后补：view.contentComponent 为 dockview 公开类型成员 IDockviewPanelModel.contentComponent）。` },
  { label: "claudemd-c", prompt: `你负责 DOC-09/10（explorer/CLAUDE.md 归 spec-revise，不碰）：
【DOC-09】根 .claude/CLAUDE.md：① 硬约束 #6 的「既定例外见 ../src/panels/CLAUDE.md」指向修正——例外实际登记于 src/theme/CLAUDE.md（fail-safe 三处 + 终端 adapter）、src/features/explorer/CLAUDE.md（六色盘）、src/features/navTree/CLAUDE.md（项目蓝），措辞改为汇总指向；② 模块索引 agentHistory 行随 FE-25 更新（HistorySessionList/Row 已删，改历史会话数据层 + NavHistoryRow 表述）；③ 需求编号索引如需补本修复族条目（免登记家族则不动）。
【DOC-10】src/features/agentHistory/CLAUDE.md 随 FE-25 更新（:19 保留段删除、:58/:68/:72/:80-81 文件表与决策描述更新——先 Read 现状再改）；src/features/navTree/CLAUDE.md（删除项目改 confirmDialog 描述 + 反查上提 pageApis 登记 + 硬约束节 TerminalRegistry 引用表述更新）；src/lib/CLAUDE.md:61 附近 HistorySessionList 提及删除（解析调用点防御分层描述照现状）。` },
  { label: "inventory-sync", prompt: `你负责 DOC-01/02/11：
【DOC-01】.claude/test-inventory.md:370 附近删除已失实的 ⚠️ 警告段（history.e2e.ts 删除用例已经 a7b0e90 改 ConfirmDialog 形态），覆盖描述更新为 data-e2e="confirm-ok" 点击语义——先 Read 现段落与 e2e-tests/history.e2e.ts:587-619 核实再改。
【DOC-02】e2e-tests/CLAUDE.md:36 history.e2e.ts「8 条 active」改「7 条 active」、移除「孤儿行 ✗」条目（对照 e2e-tests/history.e2e.ts 实际 it 块数核实）；:38 附近 waitForPanelTabIcon 术语改 waitForPanelTabStatus（随 TE-08 更名——先 grep 确认更名已落地再改文档）。
【DOC-11】.claude/test-inventory.md 全量同步：本修复 Stage 01~08 的全部用例增删逐条登记（新增回归用例/迁移用例/删除用例；各 Stage 改动可查 git log 各 Stage commit）。` },
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-09.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 09 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/ui-redesign-review-fix/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照当前代码/文件核实，防文档撒谎。
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
