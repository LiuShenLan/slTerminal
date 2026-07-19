# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-FE-03**：`src/panels/gitshow/GitShowPanel.tsx` 存在；CM 配置含 `EditorState.readOnly.of(true)` 与 `EditorView.editable.of(false)`（grep 命中）；内容加载调用 `gitFileAtHead(repoPath, oldPath ?? filePath)`（Read 确认 oldPath 优先）；错误态显示文案 `"该文件在 HEAD 中不存在"`（grep 命中）；阈值使用 `MAX_FILE_SIZE_BYTES` / `LARGE_FILE_WARN_BYTES` 且从 `useCodeMirror` import（Read 确认非本地新造数值字面量——grep `10_000_000` 在 gitshow 目录零命中）
- **CV-FE-03 硬约束**：gitshow 目录内 grep `invoke` 零命中（IPC 只经 `src/ipc/`）；颜色引用 `theme/colors.ts` token，无硬编码色值（grep `#[0-9a-fA-F]{6}` 在 gitshow 目录零命中，token 文件除外）
- **CV-FE-04**：`src/panelRegistry.ts` 含 `PANEL_GIT_SHOW = "gitshow"`；`panelRegistry` 含 gitshow 键；`PANEL_TYPES` 含 "gitshow"；`FILE_PANEL_TYPES` 含 "gitshow" 与 "diff"（Read 确认）；`src/panels/index.ts` 导出 GitShowPanel
- **CV-FE-05**：`src/__tests__/gitshow-panel.test.tsx` 存在且覆盖：loading / 内容渲染 / 错误占位文案 / oldPath 优先传参 / readOnly（Read 确认测试语义）；全量测试通过
- **回归**：`src/__tests__/panel-registry.test.ts` 断言与四类型现状一致（terminal/editor/htmlviewer/gitshow）；diff 于 Stage 04 注册，本 Stage 断言中不应出现。`src/__tests__/workspace-file-panel-types.test.ts` 的 `FILE_PANEL_TYPES.size` 断言已更新为 4 且通过；该文件 `isAlwaysRenderPanel` 断言（htmlviewer=true / editor=false / terminal=true / unknown=false）全部保持绿。`isAlwaysRenderPanel` 已为显式白名单（terminal + htmlviewer）——Read `src/panelRegistry.ts` 确认 gitshow/diff 不在 always 集合（Stage 03 决策：CM 重建无闪屏 + 大文件内存考量同 editor）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
