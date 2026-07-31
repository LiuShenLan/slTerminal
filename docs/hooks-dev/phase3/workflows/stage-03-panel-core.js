// =====================================================================
// Stage 03 Workflow — 面板骨架、注册与数据 hook
// =====================================================================
// 跨边界契约：
//   - 面板类型常量: PANEL_HOOKS_CONFIG = "hooksConfig"；组件: HooksConfigPanel
//   - store 字段: disabledHooks: DisabledHookKey[]（~/.slterminal/settings.json 的 disabledHooks 段）
//   - useHooksConfig 读 hooks 子树；Null 视为 {}；read 返回 Err → 损坏错误态
//   - 轻量重读：切层 / 面板 focusin 重读；dirty 时 dialog.ask 确认丢弃
// =====================================================================

export const meta = {
  name: 'stage-03-panel-core',
  description: 'hooksConfig 面板骨架、注册、useHooksConfig、禁用状态 store',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。串行阶段：store 先完成，panel-core 后启动。`

// === Phase 1: 并行重构（store）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-store',
    prompt: `你负责 P3-FE-18 与 P3-TE-07。

【P3-FE-18】新建 src/stores/hooksConfig.ts：
- 状态：disabledHooks: DisabledHookKey[]（类型从 src/types/hooksConfig.ts 导入）、loaded: boolean。
- loadFromDisk()：调用 src/ipc/settings 的 loadSettings()，读取 disabledHooks 并 sanitize（只保留数组，元素为 { layer, event, matcher, command } 四元组）。
- saveToDisk()：调用 saveSettings({ disabledHooks })（后端浅合并，不擦 fontSize/keybindings 等其他段）。
- disableHook(key) / enableHook(key) / isDisabled(key)。
- 变更后 2s debounce 保存 + loaded 守卫（照 keybindings.ts/fontSize.ts 模式）。
- 导出 cancelPendingSave() 供 App.tsx 关窗冲刷。
- 注意：本 store 不在 App init 中加载，由 useHooksConfig 在面板挂载时调用 loadFromDisk。

【收尾】在 src/stores/index.ts 追加 export。

【P3-TE-07】新建 src/__tests__/hooks-config-store.test.ts：
- 覆盖：默认值、loadFromDisk sanitize（合法/脏数据/缺失分支）、disable/enable/isDisabled、fake timers 验证 2s debounce 保存 payload 键集合精确匹配 { disabledHooks }、loaded 守卫防启动空写、saveSettings 失败静默吞错。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（依赖 store 完成）===
phase('串行重构')
const sequentialResults = []
const sequentialAgents = [
  {
    label: 'frontend-panel-core',
    prompt: `你负责 P3-FE-01/02/03/04/15/18b 与 P3-TE-08。

【P3-FE-01】新建 src/panels/hooksConfig/ 目录与入口文件 index.ts。

【P3-FE-02】实现 src/panels/hooksConfig/HooksConfigPanel.tsx：
- 顶部工具栏：层级切换器（user/project/local，标注优先级 local>project>user）、模式切换（GUI | JSON）占位、保存按钮、注入状态条占位。
- 中部渲染模式容器，Stage 03 先显示占位文案（JsonMode/GuiMode 后续 Stage 实现）。
- 状态：loading / content / error 三态（照 gitshow/diff 模式）+ 配置损坏错误态（readHooksConfig 返回 Err 时显示"配置文件损坏，请先修复"，与无配置 Null 区分开）。
- 使用 theme/colors.ts token，禁止硬编码色值（硬约束 #6）。

【P3-FE-15】实现 src/panels/hooksConfig/useHooksConfig.ts：
- 从 useProjects + useLayout 推导当前活跃项目 rootPath；rootPath 为空时 project/local 层禁用（仅 user 层可用）。
- 状态：layer、configJson（hooks 子树）、guiModel、dirty、error、loading。
- 加载：调用 readHooksConfig(layer, rootPath?)，null 视为 {}；面板挂载时调用 useHooksConfigStore.getState().loadFromDisk()。
- 保存：先 JSON+Schema 校验（本 Stage 占位，Stage 06 补全），再调用 writeHooksConfig。
- 轻量重读（外部修改检测）：切层 / 面板聚焦（focusin）时重新 readHooksConfig；dirty 时用 src/ipc/dialog 的 ask 弹窗提示（照编辑器外部修改先例，不用 window.confirm），用户确认丢弃才覆盖。切换 layer 时若 dirty 同样提示。

【P3-FE-03/04】修改 src/panelRegistry.ts 与 src/panels/index.ts：
- 新增 PANEL_HOOKS_CONFIG = "hooksConfig" 常量。
- panelRegistry 追加 hooksConfig: HooksConfigPanel 映射。
- PANEL_TYPES 数组末尾追加 PANEL_HOOKS_CONFIG。
- 不加入 FILE_PANEL_TYPES 与 isAlwaysRenderPanel。
- panels/index.ts 追加 export { HooksConfigPanel } from "./hooksConfig";。

【P3-FE-18b】修改 src/App.tsx：
- 从 src/stores/hooksConfig 导入 cancelPendingSave（重命名避免与 projects 的 cancelPendingSave 冲突，照 cancelSideBarSave 先例）。
- 在 registerCloseHandler 的保存序列中，与其他 store 一并调用。

【P3-TE-08】新建 src/__tests__/hooks-config-panel.test.tsx 并更新 src/__tests__/panel-registry.test.ts：
- hooks-config-panel.test.tsx 覆盖：PANEL_TYPES 包含 hooksConfig、isValidPanelType 识别、HooksConfigPanel 渲染三态 + 损坏错误态、层级切换器存在。
- panel-registry.test.ts 同步：panelRegistry 预期 6 个键、PANEL_TYPES.length === 6、toEqual 精确数组断言末尾追加 "hooksConfig"、既有索引断言顺延、新增 hooksConfig 专项断言。`
  }
];
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  sequentialResults.push(r)
}

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test -- hooks-config-store hooks-config-panel panel-registry
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
