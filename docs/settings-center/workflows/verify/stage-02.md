# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态提醒：本 Stage 后 panelRegistry.ts 未被触碰（PANEL_TYPES 仍含 hooksConfig）、配置钮未切换、pages.ts 仅注册 planBalance——按中间态判定，勿以终态误判。

## 断言清单

- **SC-FE-01a**：`src/features/settingsCenter/{types.ts, SettingsPageRegistry.ts, index.ts}` 三文件存在（Glob 命中）；types.ts 导出 SettingsPageGroup/SettingsPageProps/SettingsPage 三类型（Read 确认字段与 checklist 代码块一致：onDirtyChange/pageParams/onPageParamsChange）
- **SC-FE-01b**：SettingsPageRegistry 符合家族契约（硬约束 #13）——`register`（同 id 幂等覆盖）/ `getAll(group?)`（order ?? 注册序）/ `get(id)` / `_reset()` + `getSettingsPageRegistry()` 惰性导出（Read 确认五件齐全）
- **SC-FE-01c**：`src/__tests__/settings-page-registry.test.ts` 存在且绿（测试 agent 产出判定）
- **SC-FE-02a**：`src/workspace/pageApis.ts` 存在 `openSettingsPanel`（grep 命中）；语义式 Read 确认：panelId 形态 `settings-${pageId}`、addPanel component `"settings"`、title `"设置"`、深链 settingsPageId 注入 params.selectedPage、单例 focus 不新建
- **SC-FE-02b**：`src/features/settingsCenter/openSettings.ts` 存在 `openSettings`；语义式 Read 确认无项目分支 = `toast.show("warning", "请先创建项目")` + return（R1 修订——非静默 return）
- **SC-FE-02c**：`src/__tests__/open-settings.test.ts` 与 `src/__tests__/open-settings-panel.test.ts` 存在且绿（测试 agent 产出判定）
- **SC-FE-03a**：`src/panels/settings/{SettingsPanel.tsx, index.ts}` 存在；SettingsPanel 顶部含 `import "../../features/settingsCenter/pages"`（side-effect 注册触发点，grep 命中）
- **SC-FE-03b**：壳是 params 持久化单点（语义式：Read SettingsPanel 确认 selectedPage 切换与 pageParams 写入均经同一 persistParams 通道 = updateParameters + 显式 onLayoutChange(saveLayout(containerApi)) + updatePageLayout；不存在第二处直接 updateParameters 调用）
- **SC-FE-03c**：corrupted 警示条存在（grep `settings-corrupted-banner` 命中 SettingsPanel.tsx）；导航项 data-e2e 形态 `settings-nav-${id}`（grep `settings-nav-` 命中）
- **SC-FE-03d**：配色零硬编码（语义式：Read SettingsPanel.tsx 确认颜色全部经 theme/colors.ts token 引用，无十六进制/rgb 字面量——fail-safe 例外不适用本组件）
- **SC-FE-03e**：`src/__tests__/settings-panel.test.tsx` 存在且绿（测试 agent 产出判定）
- **SC-FE-04a**：`src/ipc/planBalance.ts` 存在 `setPlanBalanceInterval`（grep 命中），invoke 命令名逐字 `plan_balance_set_interval`、payload 键 `{ intervalSec }`（Read 确认）
- **SC-FE-04b**：`src/features/settingsCenter/pages.ts` 存在且仅注册 planBalance 一页（`{ id: "planBalance", title: "套餐余量", group: "global", order: 20 }`——Read 确认，本 Stage 无 keybindings/hooks 注册）
- **SC-FE-04c**：`src/panels/settings/pages/PlanBalancePage.tsx` 存在；语义式 Read 确认：非法输入行内红字不提交不 toast、合法提交后 refreshPlanBalance 闭环、命令 Err → toast + 保留用户输入
- **SC-FE-04d**：`src/__tests__/settings-plan-balance.test.tsx` 存在且绿；`src/__tests__/ipc-plan-balance-contract.test.ts` 含 setPlanBalanceInterval 四维契约且绿（测试 agent 产出判定）
- **中间态-01**：`grep -n "hooksConfig" src/panelRegistry.ts` 仍命中（本 Stage 未触碰 panelRegistry——中间态断言，命中即通过；零命中说明越界改动判 not_fixed）
- **中间态-02**：`grep -n "openHooksConfigFromActivityBar" src/features/sideViews/ActivityBar.tsx` 仍命中（配置钮未切换——中间态断言，同上）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
