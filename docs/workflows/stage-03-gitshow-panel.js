// =====================================================================
// Stage 03: gitshow 只读面板（CV-FE-03~05）
// =====================================================================
// 跨边界契约（写死）：
//   - 面板类型: "gitshow"；params: { panelId: string; filePath: string; oldPath?: string; repoPath: string }
//   - HEAD 取内容路径参数 = oldPath ?? filePath
//   - 错误占位文案: "该文件在 HEAD 中不存在"
//   - 阈值常量复用 editor 的 MAX_FILE_SIZE_BYTES / LARGE_FILE_WARN_BYTES（从 useCodeMirror 导出，禁新造）
// fix-loop 调用时 args.constraints 传 ""
// =====================================================================

export const meta = {
  name: 'stage03-gitshow-panel',
  description: 'Stage 03: gitshow 只读面板（HEAD 版本查看）（CV-FE-03~05）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
本 Stage 单 agent（panelRegistry.ts 单文件不可并行）。不跑测试，由全量测试 agent 单点跑。

跨边界契约（写死，不得偏离）：
- 面板类型 "gitshow"；params { panelId, filePath, oldPath?, repoPath }
- HEAD 取内容：gitFileAtHead(repoPath, oldPath ?? filePath)
- 错误占位文案："该文件在 HEAD 中不存在"
- 阈值常量从 useCodeMirror 导出复用（MAX_FILE_SIZE_BYTES / LARGE_FILE_WARN_BYTES），禁止新造数值`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "gitshow-panel",
    prompt: `你负责 CV-FE-03 / CV-FE-04 / CV-FE-05 全部 3 项：

【CV-FE-03】GitShowPanel 实现
- 新建 src/panels/gitshow/（index.ts、GitShowPanel.tsx）
- 组件结构照 EditorPanel.tsx 模式（container ref → state → hook）
- params: { panelId: string; filePath: string; oldPath?: string; repoPath: string }
- 内容加载：gitFileAtHead(repoPath, oldPath ?? filePath)（经 src/ipc/git，禁止直接 invoke）
- 三态：loading / content / error——error 态显示占位文案"该文件在 HEAD 中不存在"（样式照 HtmlPanel 错误态）
- CM6 只读：EditorState.readOnly.of(true) + EditorView.editable.of(false)；扩展：basicSetup + oneDark + 高度 theme + 字体（editorFontSize store）+ getLanguageExtension(filePath)（从 useCodeMirror.ts import）
- 大文件：内容 length 超 MAX_FILE_SIZE_BYTES 拒绝、超 LARGE_FILE_WARN_BYTES 警告——两常量当前是 useCodeMirror.ts 模块内 const，需在该文件加 export 后 import 复用（禁止另造数值）
- 颜色一律 theme/colors.ts token（EDITOR_BG 等）

【CV-FE-04】注册 gitshow 面板类型
- src/panels/index.ts 导出 GitShowPanel
- src/panelRegistry.ts：PANEL_GIT_SHOW = "gitshow"；panelRegistry.gitshow 注册（params 类型照现有写法）；PANEL_TYPES 追加
- FILE_PANEL_TYPES 追加 "gitshow" 与 "diff"（"diff" 由 Stage 04 实现，此处仅注册集合先行包含——先 Grep FILE_PANEL_TYPES 全部消费方确认无破坏性）
- isAlwaysRenderPanel 改显式白名单（terminal + htmlviewer）：gitshow/diff 不 always——always 动机是保 PTY/iframe browsing context，CM 重建无闪屏，大文件内存考量同 editor 排除理由（stages.md Stage 03 决策）
- 更新 src/__tests__/panel-registry.test.ts（三面板→四面板断言；FILE_PANEL_TYPES 断言）
- 更新 src/__tests__/workspace-file-panel-types.test.ts（FILE_PANEL_TYPES.size 断言 2→4 + has 断言；isAlwaysRenderPanel 现有断言 htmlviewer=true/editor=false/terminal=true/unknown=false 应全部保持绿）

【CV-FE-05】gitshow L2 测试
- 新建 src/__tests__/gitshow-panel.test.tsx
- mock ../ipc/git 的 gitFileAtHead；mock CM6 相关（照现有 editor 测试模式）
- 覆盖：loading 态 / 内容渲染 / 错误占位文案"该文件在 HEAD 中不存在" / oldPath 优先于 filePath 传入 gitFileAtHead / readOnly 配置存在`,
  },
];
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
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
