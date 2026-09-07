# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：串行链 017→036→042 顺序不可换。

## 断言清单

- **CP-017**：`grep -c "setSettingsDirty(panelId, false)" src/panels/settings/SettingsPanel.tsx` = 0；`grep -n "clearSettingsDirty" src/workspace/tabClose.ts` ≥1；`src/panelRegistry.ts` 的 `isAlwaysRenderPanel` 白名单含 settings（Read 确认四类型）；`src/panels/settings/SettingsPanel.tsx` SC-FE-08 确认分支补了 `clearSettingsDirty`（Read 确认）；`npx vitest run settings-panel-dirty tab-close open-settings-panel workspace-file-panel-types` 绿（随全量覆盖）。
- **CP-036**：`grep -n "forEach((p) => p.api.close())" src/workspace/PageDockviewHost.tsx` = 0；`grep -n "closeTabsGuarded" src/workspace/PageDockviewHost.tsx src/workspace/tabClose.ts` 各 ≥1；「关闭其他/关闭全部」两 action 均走 closeTabsGuarded（Read 确认）；PageDockviewHost.tsx 原 :278-279 遗留注释已删；**页删除路径复核点**：S07 收尾时已复核 Workspace 删页路径并落结论（守卫或显式不守卫登记），证据写入 commit body 或 workspace/CLAUDE.md。
- **CP-042**：`grep -n "for (let i = 0; i < 50; i++)" src/workspace/pageApis.ts` 命中数 = 1（仅剩 switchToPageAndFocus 一处）；`grep -n "PAGE_API_READY_EVENT" src/workspace/pageApis.ts` ≥3；`grep -n "toast.show" src/workspace/pageApis.ts` ≥2；`openSettingsPanel` 的 addPanel 含 `renderer: "always"`（Read 确认，即 CP-017 最终形态）；`npx vitest run open-settings-panel workspace-page-apis` 绿（随全量覆盖）。
- **CP-016**：`grep -n "getViewState" src/features/sideViews/SideBarArea.tsx` = 2 处；`grep -c "commitViewState" src/features/explorer/useFileTree.ts` ≥ 4；`SideViewRegistry` 含 viewStates Map + getViewState/setViewState 且 `_reset()` 同步清空（Read 确认）；`ExplorerPanel.tsx` 透传 viewState/onViewStateChange 两 prop（Read 确认）；`npx vitest run sideViewRegistry use-file-tree sidebar-area` 绿（随全量覆盖）。
- **CP-037**：`grep -n 'mode !== "preview" ? cmContainerRef' src/panels/markdown/MarkdownPanel.tsx` = 0；`grep -n 'visible={mode !== "preview"}' src/panels/markdown/MarkdownPanel.tsx` ≥1；`npx vitest run markdown-panel` 绿（随全量覆盖）；**S10 复核注记**：预览迁 webview 后保活形态复核已登记于 S10 任务卡（stages.md S10 实现要点）。
- **CP-019**：`grep -n "requestAnimationFrame" src/panels/terminal/useXterm.ts` = 0；`grep -n "ResizeObserver" src/panels/terminal/useXterm.ts` ≥2；清理段含 `spawnObserver.disconnect()` 与 `clearTimeout(spawnTimeoutId)`（Read 确认）；`npx vitest run use-xterm-lifecycle` 绿（随全量覆盖）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
