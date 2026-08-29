# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SC-FE-08a**：`src/panels/settings/SettingsPanel.tsx` 存在切项目自动关闭 effect（grep 命中 `activePageId` 订阅）
- **SC-FE-08b**：四分支齐全（语义式：Read effect 确认——① ownPageId = params.panelId 去 settings- 前缀 + projects 反查 ownProjectId；② activePageId === null → 不动（注释注明防连锁误关）；③ 双非空且不同 → isSettingsDirty → confirmDialog 取消则不关（注释注明尊重用户选择）→ api.close()；④ 初始挂载时不一致 → 直接 api.close() 静默（无 confirm——新挂载不可能 dirty））
- **SC-FE-08c**：`src/__tests__/settings-panel-autoclose.test.tsx` 存在且 5 用例绿（切项目关 / 同项目切页不关 / 初始不一致静默关 / activePageId null 不关 / dirty confirm 取消不关——测试 agent 产出判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
