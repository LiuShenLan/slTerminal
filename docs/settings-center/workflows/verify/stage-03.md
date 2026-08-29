# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态提醒：e2e-tests/ 下 hooks.e2e.ts / mockcli.e2e.ts 的 `component:"hooksConfig"` 残留属预期（Stage 06 才适配），不判 failed；本 Stage 后 PANEL_TYPES length 6 末位 settings。

## 断言清单

- **SC-FE-05a**：`grep -rn "panels/hooksConfig" src/ src-tauri/` 零命中；`grep -rn "features/hooksConfig" src/` 零命中（含注释——eventsCatalog.ts:4 注释已改指 cliProfiles/CLAUDE.md）
- **SC-FE-05b**：`src/panels/hooksConfig/` 目录不存在（Glob 零命中）；`src/features/hooksConfig/` 目录不存在（Glob 零命中——schema 已迁、openHooksConfig.ts 已删、CLAUDE.md 本 Stage 可暂存待 Stage 07 删，若存在不判 failed 但登记）
- **SC-FE-05c**：编辑器 10 文件 + schema/ 存在于 `src/features/cliProfiles/profiles/claude/configEditor/`（Glob 命中 11 项：ClaudeHooksConfigEditor.tsx / useHooksConfig.ts / GuiMode.tsx / JsonMode.tsx / EventTree.tsx / HandlerForm.tsx / MatcherTester.tsx / configModel.ts / eventsCatalog.ts / matcherEngine.ts / schema/index.ts）
- **SC-FE-05d**：`src/features/cliProfiles/profiles/claude/index.ts` 的 configEditor import 指向 `./configEditor/ClaudeHooksConfigEditor`（Read 确认）；跨目录 import 深度正确（语义式：Read configEditor/ 内任一文件确认 `../../../../../` 前缀、schema 引用为 `./schema`）
- **SC-FE-05e**：`src/panels/settings/pages/HooksSettingsPage.tsx` 存在；props 为 SettingsPageProps 形态（Read 确认 onDirtyChange/pageParams/onPageParamsChange，不再有 handleLayoutPersist 自持久化）；根容器保留 `data-e2e="hooks-config-panel"`（grep 命中）
- **SC-FE-05f**：pages.ts 含 hooks 注册 `{ id: "hooks", group: "project", order: 100 }`（Read 确认）
- **SC-FE-05g**：FB-22 测试波及面全落地（语义式：13 文件逐一核对——8 路径更新文件 grep 旧路径零命中；hooks-config-panel.test.tsx 不存在且 settings-hooks-page.test.tsx 存在；no-claude-literals.test.ts EXEMPT_DIRS 无 `"src/panels/hooksConfig"` 条目；全量 npm test 绿）
- **SC-FE-06a**：`grep -n "hooksConfig" src/panelRegistry.ts` 零命中；PANEL_TYPES 末位 `"settings"` 且 length 6（Read 确认 = [terminal, editor, htmlviewer, gitshow, diff, settings]）
- **SC-FE-06b**：`isAlwaysRenderPanel` 未加入 settings（Read panelRegistry.ts 确认仍仅 terminal+htmlviewer——决策写死项）
- **SC-FE-06c**：`grep -rn "openHooksConfig" src/` 零命中；`src/features/hooksConfig/openHooksConfig.ts` 不存在；ActivityBar.tsx import `openSettings`（grep 命中 `../settingsCenter/openSettings`）
- **SC-FE-06d**：layout-serde.test.ts 新增「旧 hooksConfig 面板被白名单过滤」用例且绿（grep 该测试文件含 hooksConfig 字样的过滤用例——此处测试文件内残留 hooksConfig 字面量合法，是过滤对象）
- **SC-FE-07a**：`src/features/settingsCenter/dirtyRegistry.ts` 存在且导出 setSettingsDirty/isSettingsDirty/clearSettingsDirty（grep 命中三函数）
- **SC-FE-07b**：切配置页守卫（语义式：Read SettingsPanel 确认当前页 dirty → askGuard 前置 + confirmDialog「当前配置页有未保存的修改，切换将丢弃这些修改。」→ 取消不切换/确认清 dirty 切换，含 finally setTimeout 复位）；导航项 dirty 圆点存在
- **SC-FE-07c**：× 关闭守卫（语义式：Read PageDockviewHost.tsx DefaultTab × 钮 onClick 确认 = `panel.view.contentComponent === "settings"`（非 `panel.component`）且 `isSettingsDirty(panel.id)` → confirmDialog 确认才 close；非 settings 面板直关不拦截）
- **SC-FE-07d**：settings-panel-dirty.test.tsx / settings-dirty-registry.test.ts / workspace-defaulttab.test.tsx × 拦截用例绿（测试 agent 产出判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
