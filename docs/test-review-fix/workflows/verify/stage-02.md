# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-A-02**：`src/__tests__/workspace.test.tsx`、`workspace-multi-instance.test.tsx`、`workspace-switch-order.test.tsx`、`workspace-page-dockview.test.tsx` 四文件均含 `originalResizeObserver` 保存与 `afterAll` 恢复（grep `originalResizeObserver` 命中 4 文件）；`use-xterm-integration.test.ts` 含 `afterAll` 中 getContext `mockRestore`（grep `mockRestore` 命中）。
- **TQ-A-03**：`src/__tests__/setup.ts` 含 `Range.prototype.getClientRects` stub（grep 命中）；`npm test` 运行输出中不再出现 `getClientRects is not a function`（以全量测试输出判定）。
- **TQ-B-02**：`src/__tests__/sideBar.test.ts`、`sideBarArea.test.tsx`、`activityBar.test.tsx` 三文件均含 `vi.mock("../features/sideViews/sideViewDefs", () => ({}))`（grep 命中 3 文件）；三文件 beforeEach 内 `_reset()` 后存在 `getAll().length === 0` 防御断言（Read 确认）。
- **TQ-B-10**：`src/__tests__/helpers/workspace-setup.ts` 的 `resetProjectStores()` 内同时重置 useSideBar 与 useKeybindings（Read 确认）；`commit-view.test.tsx`、`nav-tree.test.tsx`、`explorer-crud-success.test.tsx` 的 beforeEach 改为调用该共享重置（Read 确认，私有 resetStore 已删除或改为薄包装）。
- **TQ-B-14**：`src/__tests__/nav-history-row.test.tsx` afterEach 含 CliProfileRegistry 的 `_reset()` 调用（grep `_reset` 命中）。
- **TQ-B-15**：`src/__tests__/commit-open-file.test.ts` 与 `explorer-crud-success.test.tsx` 的 afterEach 均含 `delete` `__dockviewApi`（grep `__dockviewApi` 命中两文件的删除语句）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
