# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SC-E2E-01a**：`e2e-tests/helpers.ts` 存在 `installSettingsPanelHelpers` 且 installAllE2eHelpers 已接线（grep 双命中）；四 helper 定义齐全：`__slterm_e2e_openSettings` / `__slterm_e2e_getSettingsPanelState` / `__slterm_e2e_getSettingsPanelCount` / `__slterm_e2e_switchSettingsPage`（grep 四命中）
- **SC-E2E-01b**：`src/__tests__/app.test.tsx` helper 契约测试含新 helper 存在性断言且绿（测试 agent 产出判定）
- **SC-E2E-02a**：`e2e-tests/settings.e2e.ts` 存在且含 10 用例（Read 确认用例 ①–⑩ 与 checklist SC-E2E-02 步骤 1 一一对应；corrupted 警示条不做 L4——spec 中无该用例属正确）
- **SC-E2E-02b**：`grep -n "hooksConfig" e2e-tests/hooks.e2e.ts e2e-tests/mockcli.e2e.ts` 零命中（component 字面量与清理分支均已切 settings）；选择器 `[data-e2e="hooks-config-panel"]` 与 `[data-e2e="hooks-json-editor"]` 保留使用（grep 命中——选择器不变属正确，删除反判 partial）
- **SC-E2E-02c**：hooks.e2e.ts / mockcli.e2e.ts 程序化 addPanel 形态 = `{ component: "settings", params: { panelId, selectedPage: "hooks" } }`（语义式：Read 两 spec 的 addPanel 调用点确认 params 含 selectedPage 深链）
- **SC-E2E-02d**：`npm run e2e` 全量绿（测试 agent 产出判定——settings.e2e 10 用例 + hooks.e2e + mockcli.e2e 适配后全过；用例④ 频率页 120 真实后端落盘通过即 SC-BE-03 三处注册静默失败盲区闭环）
- **L4 计数**：e2e 通过总数 = 40 + settings.e2e 新增 10 = 50（从测试 agent 的 wdio 输出取数；供 Stage 07 test-inventory 校准引用）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx vite build`
5. `npm run e2e`（= build:e2e VITE_E2E=1 + wdio 一体化，十分钟级耗时长属正常）
