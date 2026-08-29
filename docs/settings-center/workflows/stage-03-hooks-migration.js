// =====================================================================
// Stage 03 — hooks 迁入 + 原子切换（SC-FE-05..07）⚠️ 高风险 Stage
// =====================================================================
// 改动项: SC-FE-05 hooks 页迁入+编辑器归域 / SC-FE-06 panelRegistry 原子切换+入口切换
//         / SC-FE-07 dirty 汇聚守卫
// 分工: 3 agent 并行（A=迁移+测试路径 / B=HooksSettingsPage+dirty 守卫 / C=注册表+入口切换）
//       文件零重叠证明见 stages.md 分工表
// 门禁: tsc + eslint + npm test（全量）
// fix-loop 调用约束: args.constraints 传
//   "高风险 Stage：迁移只改 import 路径不改逻辑；选择器 data-e2e 全部保留；
//    编辑器逻辑零行为变更"
// =====================================================================

export const meta = {
  name: 'stage03-hooks-migration',
  description: 'Stage 03: hooks 编辑器归域 configEditor/ + hooksConfig 面板类型退役切 settings + dirty 汇聚守卫（F11 高风险）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中你负责的条目全文，严格按「修复步骤」执行（代码块为照抄级，禁止自行另设计）。
【Stage 特殊纪律】git mv 保留历史；迁移只改 import 路径不改逻辑；data-e2e 选择器全部保留（hooks-config-panel / hooks-json-editor / hooks-cli-\${id}）。重构阶段只做编译级检查（npx tsc --noEmit），npm test 由全量测试 agent 统一跑——并行 agent 禁止各自跑 npm test。`

// === Phase 1: 并行重构（3 agent 文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'migrate-editor-files', prompt: `你负责 SC-FE-05 的迁移部分（agent A）：

1. git mv 编辑器 10 文件 + schema/（逐条执行，禁 xcopy/复制粘贴——保留 git 历史）：
   git mv src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/useHooksConfig.ts src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/GuiMode.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/JsonMode.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/EventTree.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/HandlerForm.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/MatcherTester.tsx src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/configModel.ts src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/eventsCatalog.ts src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/panels/hooksConfig/matcherEngine.ts src/features/cliProfiles/profiles/claude/configEditor/
   git mv src/features/hooksConfig/schema src/features/cliProfiles/profiles/claude/configEditor/schema
   （configEditor/ 目录先建；schema/index.ts 自包含零改行——仅 import json-schema-library + ./claude-code-settings.json）
2. 删除 src/panels/hooksConfig/{HooksConfigPanel.tsx, index.ts}（hub 由 agent B 重建为 HooksSettingsPage，barrel 唯一消费方 sync 测试由 agent B 改造）；删除后 src/panels/hooksConfig/ 目录应为空——连目录删除
3. 迁移后跨目录 import 改造（checklist SC-FE-05 步骤 2 的 19 行改造表逐行照抄——./ 同目录 import 全部不变，仅跨目录 ../../ 变 ../../../../../，schema 两处变 ./schema，eventsCatalog.ts:4 注释路径同步）
4. src/features/cliProfiles/profiles/claude/index.ts:16 import 改 ./configEditor/ClaudeHooksConfigEditor；头部「依赖方向合法化」注释更新（不再跨 panels 引用）
5. 测试路径更新（agent A 名下 8+2 文件，逐文件按 checklist SC-FE-05 测试同步段行号改 import）：
   src/__tests__/hooks-config-catalog.test.ts:20 / hooks-config-matcher.test.ts:9 / hooks-config-model.test.ts:12 / hooks-config-handlerform.test.tsx:11-13 / hooks-config-gui.test.tsx:13-16 / hooks-config-jsonmode.test.tsx:105-108 / hooks-config-schema.test.ts:12 / cli-profile-claude.test.ts:34 / no-claude-literals.test.ts:50（EXEMPT_DIRS 删 "src/panels/hooksConfig" 条目——configEditor 已被 :46 父目录豁免覆盖，无需新增）/ hooks-config-sync.test.tsx:87-88（仅 mock 路径两行；:92-93 barrel/形态改造归 agent B，勿动）
自查：npx tsc --noEmit 暂可红（agent B/C 未完成的引用属预期）——只做语法级自检，最终编译以全量测试 agent 为准。` },
  { label: 'hooks-settings-page', prompt: `你负责 SC-FE-05 的 HooksSettingsPage 部分 + SC-FE-07（agent B）：

1. 新建 src/panels/settings/pages/HooksSettingsPage.tsx = hub 改造（照 HooksConfigPanel.tsx 既有结构——CLI 选择行/编辑器槽分派/dirty 守卫照搬 :232-287/:139-209 先例）：
   - props 改 SettingsPageProps（从 ../../features/settingsCenter/types import type）——编辑器 dirty 经 onDirtyChange 直传壳
   - selectedCli 改读 pageParams?.selectedCli、写经 onPageParamsChange({ selectedCli })（壳单点持久化——删除原 handleLayoutPersist/persistSelectedCli 自持久化逻辑）
   - 根容器保留 data-e2e="hooks-config-panel"（选择器语义继承）；选择行 data-e2e=hooks-cli-\${p.id} 保留
2. src/features/settingsCenter/pages.ts 追加注册 { id: "hooks", title: "Hooks 配置", group: "project", order: 100 }
3. SC-FE-07 dirty 汇聚守卫：
   - 新建 src/features/settingsCenter/dirtyRegistry.ts：Map<panelId, boolean> + setSettingsDirty/isSettingsDirty/clearSettingsDirty（壳挂载注册、卸载 clear）
   - src/panels/settings/SettingsPanel.tsx 壳增强：dirtyMap state（pageId→dirty）；页 onDirtyChange → setState + 同步 dirtyRegistry；导航项 dirty 圆点（7px 中性色 token——不用 F3 四态色）；切配置页守卫（当前页 dirty → askGuard 前置 + confirmDialog 文案「当前配置页有未保存的修改，切换将丢弃这些修改。」→ 取消不切换/确认清 dirty 切换；含 finally setTimeout 复位 askGuard，照 HooksConfigPanel.tsx:174-209 先例）
   - src/workspace/PageDockviewHost.tsx:458 × 关闭守卫：onClick 包 async——panel.view.contentComponent === "settings"（panel.component 不存在红线）且 isSettingsDirty(panel.id) → confirmDialog 确认才 api.close()，否则直关
4. 测试（agent B 名下）：
   - src/__tests__/hooks-config-panel.test.tsx → 改名 settings-hooks-page.test.tsx 并改造 renderPanel helper（panel props 形态 → SettingsPageProps 形态；updateParameters/toJSON 断言改断 onPageParamsChange 回调）
   - src/__tests__/hooks-config-sync.test.tsx :92-93 改造（barrel import ../panels/hooksConfig → ../panels/settings/pages/HooksSettingsPage；useHooksConfig 路径同步 configEditor/；mockApi/mockContainerApi hub panel props 形态 → SettingsPageProps 形态）
   - src/__tests__/mock-cli-profile.test.tsx :46,256（import HooksConfigPanel → HooksSettingsPage + mock JsonMode 路径同步）
   - 新建 src/__tests__/settings-panel-dirty.test.tsx（切页 confirm 确认/取消/非 dirty 直切/圆点显隐）
   - 新建 src/__tests__/settings-dirty-registry.test.ts（set/is/clear）
   - src/__tests__/workspace-defaulttab.test.tsx 加 × 拦截用例（settings dirty → confirm 取消不关、确认关；非 settings 面板不经守卫）
自查：npx tsc --noEmit 暂可红（agent A/C 并行中）——只做语法级自检。` },
  { label: 'registry-entry-switch', prompt: `你负责 SC-FE-06（agent C）：

1. src/panelRegistry.ts 原子切换：
   - :12 import 改 SettingsPanel（from "./panels/settings"）
   - :67-69 registry 键 hooksConfig: withPanelBoundary(HooksConfigPanel...) 改 settings: withPanelBoundary(SettingsPanel as React.FC<{ params: { panelId: string } }>)
   - :73-80 PANEL_TYPES "hooksConfig" → "settings"（原位替换保持末位——既有测试断言末位）
   - :39-48 FE-22 惰性 displayName getter 注释中 HooksConfigPanel 提及改 SettingsPanel（同形 TDZ 循环）
   - isAlwaysRenderPanel 不加入 settings（决策写死：同 editor/gitshow/diff——状态在 params/store，不新增 always 内存开销）——不改 :101-103
2. src/features/sideViews/ActivityBar.tsx：:26 import 改 { openSettings }（from "../settingsCenter/openSettings"）；:258 调用改 void openSettings();
3. src/workspace/pageApis.ts：删 openHooksConfigPanel 函数（:130-156）；:11 注释引用 features/hooksConfig/CLAUDE.md 改指 features/settingsCenter/CLAUDE.md
4. 删除 src/features/hooksConfig/openHooksConfig.ts（删除后 features/hooksConfig/ 目录应已空——agent A 已迁走 schema/；若仍有残余文件报告主 agent，不自作主张）
5. 测试（agent C 名下）：
   - src/__tests__/panel-registry.test.ts：六键含 settings / PANEL_TYPES toEqual 末位 settings / length 6
   - src/__tests__/layout-serde.test.ts 9a：c1 改 settings + 新增「旧 hooksConfig 面板被白名单过滤」用例（loadLayout 含 hooksConfig 面板 → fromJSON 参数中该面板已删除）
   - src/__tests__/workspace-file-panel-types.test.ts：hooksConfig 断言行改 settings、size 4 不变（先核实现状断言再适配）
   - src/__tests__/activityBar.test.tsx:36：mock 路径 ../features/hooksConfig/openHooksConfig → ../features/settingsCenter/openSettings（mock 导出名同步 openSettings）
   - 删除 src/__tests__/open-hooks-config.test.ts 与 src/__tests__/open-hooks-config-panel.test.ts（SC-FE-02 新测试已取代）
自查：npx tsc --noEmit 暂可红（agent A/B 并行中）——只做语法级自检。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
中间态提醒：本 Stage 后 PANEL_TYPES = [terminal, editor, htmlviewer, gitshow, diff, settings]（length 6 末位 settings）；e2e-tests/ 下 hooks.e2e.ts/mockcli.e2e.ts 的 component:"hooksConfig" 残留属预期（Stage 06 才适配），不判 failed。
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

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
