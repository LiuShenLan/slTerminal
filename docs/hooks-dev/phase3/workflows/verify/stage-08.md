# Stage 08 逐项验证断言

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-FE-23**：`src/features/shortcuts/commandCatalog.ts` 的 `COMMAND_CATALOG` 包含 `global.openHooksConfig`；元数据完整（id/title/category/context/defaultKey/priority）；默认键为 `Ctrl+Shift+KeyH`（或执行期降级 `Ctrl+Alt+KeyH`）；默认键不是保留键。
- **P3-FE-24**：`src/features/shortcuts/globalCommands.ts` 的 `createGlobalShortcuts` 返回 `global.openHooksConfig` 命令；handler 为同页单例逻辑（语义式：Read 代码确认）：取 `useLayout.getState().activePageId` → id = `"hooksConfig-" + pageId` → `getPanel(id)` 命中 `focus()` 不新建、未命中 `addPanel({ id, component: "hooksConfig", title, params: { panelId: id } })`；无活跃页面/无 api 返回 `false` 透传；**无 `generatePanelId` 引用**（语义式：grep 确认）。
- **P3-FE-25**：`src/App.tsx` 调用点未改且无需改；确认 `...createGlobalShortcuts(...)` 展开已包含新命令。
- **P3-TE-17**：`src/__tests__/command-catalog.test.ts` 已同步：`EXPECTED_IDS` 含 `global.openHooksConfig`、长度预期为 10、补充该命令元数据与默认键非保留键断言；`src/__tests__/hooks-config-entry.test.ts` 存在：覆盖首次 `addPanel`（id 为 `hooksConfig-{pageId}`）、重复触发 `getPanel` 命中聚焦不新建、无 api/无活跃页面透传。

## 全量测试

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test -- command-catalog hooks-config-entry`
