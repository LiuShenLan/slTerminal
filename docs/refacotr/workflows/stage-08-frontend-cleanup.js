// Stage 8 — 前端低危清理（约束 #6）（FE-11、FE-13、FE-20、FE-21、FE-24）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 5 个并行 agent，文件不重叠（对齐 refactoring-stages.md Stage 8 分工表）
// token 命名约定（写死，theme-agent 必须严格使用，sidebar/filetree-agent 按名引用）：
//   APP_BG_PRIMARY（#1e1e2e）、APP_BG_SECONDARY（#2b2b3c）、SHADOW_MENU（rgba(0,0,0,0.5)）
//   SIDEBAR_COLORS（侧栏节点色组）、GIT_FILE_COLORS（文件树 git 状态色组，GIT_GUTTER_COLORS 同款风格）

export const meta = {
  name: 'stage8-frontend-cleanup',
  description: 'Stage8: 配色单点化（约束 #6）+ 死代码删除 + 组件结构清理',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7（前端 agent 不涉及，声明备查）。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）；架构约束 #6 配色单点见根 .claude/CLAUDE.md。
并行说明：5 个 agent 同时工作。theme-agent 与你并行在 colors.ts 新增 token——若你的代码引用的 token 暂未导出，属预期；你只跑 vitest 自查（不单独跑 tsc），全量 tsc 由后续统一测试阶段执行。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 src/theme/colors.ts 与 src/App.css（FE-24 token 主体）：

1. colors.ts 新增以下 token（命名严格如下，不得改名——sidebar/filetree-agent 按此引用）：
   - APP_BG_PRIMARY = '#1e1e2e'、APP_BG_SECONDARY = '#2b2b3c'、SHADOW_MENU = 'rgba(0,0,0,0.5)'
   - SIDEBAR_COLORS 组：SidebarTree 现有硬编码色值提取（读 src/features/sidebar/SidebarTree.tsx:82 附近确认实际色值）
   - GIT_FILE_COLORS 组：FileTree git 状态色提取（读 src/features/explorer/FileTree.tsx:71 附近确认实际色值，风格对齐现有 GIT_GUTTER_COLORS）
2. App.css（5-8 行附近）的硬编码背景色/阴影改为引用 colors.ts token——先查 colors.ts 现有 token（如 GIT_GUTTER_COLORS）的消费方式（inline style / CSS 变量注入），照同一模式接入，不引入新机制
3. 若 colors.ts 有测试，补新 token 存在性与格式断言

改完跑：npx tsc --noEmit && npx eslint src/theme/ 通过。
`, { label: 'theme-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/panelRegistry.ts 的 1 项改动：

【FE-11】isAlwaysRenderPanel 冗余分支简化（panelRegistry.ts:59-61）
- 读 checklist FE-11 条目与现有代码，简化冗余分支（保持行为等价）
- 补/改 panelRegistry.test.ts 断言行为不变

改完跑：npx vitest run panelRegistry 全绿。
`, { label: 'registry-agent' }),

  () => agent(`
${PREAMBLE}

你负责死代码删除（FE-13）：

删除清单（checklist FE-13 确定，逐项执行）：
1. src/features/shortcuts/forwardGlobalShortcuts.ts — 整个文件删除（HtmlPanel 已走注入脚本 postMessage 路径，此为旧路径死代码）
2. src/features/shortcuts/index.ts — 删除对应 re-export
3. src/__tests__/forward-global-shortcuts.test.ts — 专属测试文件删除（postMessage 路径已有 html-panel 测试覆盖）
4. grep -rn "forwardGlobalShortcuts\\|attachGlobalShortcutForwarder" src/ 确认无其他引用残留（若有引用先报告，不强行删）

改完跑：npx tsc --noEmit && npx eslint src/ && npm test 全绿。
`, { label: 'deadcode-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/features/sidebar/SidebarTree.tsx 的 2 项改动：

【FE-20】内联子组件每次渲染重建（SidebarTree.tsx:115-294）
- Toolbar/ProjectRow/PageRow 等内联定义的子组件提取到模块顶层；props 显式传递
- 不改组件结构、交互逻辑与样式

【FE-24】硬编码颜色接入 token（约束 #6）
- 文件内硬编码色值改为引用 src/theme/colors.ts 的 SIDEBAR_COLORS token 组（token 由 theme-agent 并行新增，若暂未导出属预期——你只跑 vitest，不单独跑 tsc）

改完跑：npx vitest run sidebar 全绿。
`, { label: 'sidebar-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/features/explorer/FileTree.tsx 的 2 项改动：

【FE-21】paddingLeft 魔法数字（FileTree.tsx:388-599）
- 裸表达式 8 + (depth + 1) * 16 提取为 ICON_WIDTH/ARROW_WIDTH/INDENT 常量（模块顶层，附中文注释说明几何含义）
- 不改渲染结果

【FE-24】git 状态色接入 token（约束 #6）
- git 状态着色（untracked/added/modified/deleted）硬编码色值改为引用 src/theme/colors.ts 的 GIT_FILE_COLORS token 组（token 由 theme-agent 并行新增，若暂未导出属预期——你只跑 vitest，不单独跑 tsc）

改完跑：npx vitest run explorer 全绿。
`, { label: 'filetree-agent' }),
])

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-08.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 8 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
