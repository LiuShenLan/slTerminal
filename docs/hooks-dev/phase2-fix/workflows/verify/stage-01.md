# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FIX-FE-01**：`src/workspace/pageApis.ts` 存在，且导出 `registerPageApi` / `unregisterPageApi` / `getPageApi` / `switchToPageShared` / `switchToPageAndFocus` 五个函数（grep `export` 枚举确认）。
- **FIX-FE-01**：`switchToPageShared` 的实现满足时序契约——`await setProjectRoot`（或项目无 rootPath 时跳过）在 `setActivePage` 之前；`getPageApi(pageId)` 命中时将 `window.__dockviewApi` 重指向（须 Read 函数体确认顺序，非仅 grep 存在性）。
- **FIX-FE-01**：`switchToPageAndFocus` 先 `await switchToPageShared(pageId)`，再轮询 `getPageApi(pageId)?.getPanel(panelId)`（有限次数，含间隔等待），命中后 `focus()`（须 Read 确认）。
- **FIX-FE-01**：`Workspace.tsx` 中不存在组件级 `pageApiMapRef`（grep `pageApiMapRef` 零命中）；`switchToPage`/`onDeletePage`/`handlePageApiReady` 三处均经 pageApis 模块函数（Read 确认）。
- **FIX-FE-01**（语义式）：全仓 `window.__dockviewApi =`（赋值，非读取）仅出现在三站点——`Workspace.tsx` 的 `switchToPage`（可经 switchToPageShared 间接）、`onDeletePage`、`handlePageApiReady`（含 pageApis.ts 内 switchToPageShared 一处）；`useClaudeNotifications.ts`、`AgentStatusView.tsx`、`helpers.ts` 中不得出现赋值（grep 全仓枚举逐处确认）。
- **FIX-FE-01**（语义式）：`useClaudeNotifications.ts` 的 `routeToPanel` 中不存在任何直接调用 `setActivePage` / `setProjectRoot` 的代码（须 Read 函数体确认）；routeToPanel 调用 `switchToPageAndFocus`。
- **FIX-FE-01**：`findPanelTitle` 经 `getPageApi(...)` 查询（Read 确认），不再直接读 `window.__dockviewApi`。
- **FIX-FE-01**：`e2e-tests/helpers.ts` 的 `__slterm_e2e_switchToPage` 委托 `switchToPageShared`（grep `switchToPageShared` 命中），不再自行 `setActivePage`（Read 函数体确认）。
- **FIX-FE-01**：`notifications.test.ts` 存在 routeToPanel 守卫用例——触发 toast onClick 后断言共享切换被调用且 routeToPanel 不直接调 `setActivePage`（grep `switchToPageAndFocus` 命中 + 用例绿）。
- **FIX-FE-02**（语义式）：`AgentStatusView.tsx` 的 `handleFocus` 中不存在 `props.switchToPage` 调用与 `__dockviewApi` 读取（须 Read 函数体确认）；handleFocus 调用 `switchToPageAndFocus` 且为 async/await（grep 命中）。
- **FIX-FE-02**：`agent-status-view.test.tsx` 点击用例断言 `switchToPageAndFocus` 被调用（grep 命中 + 用例绿）。
- **FIX-FE-10**：`useClaudeNotifications.ts` 去重注释与实现一致——不含「60s」字样（grep `60s` 零命中），注释描述键去重 + 200 条截断（Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `npx vite build`（helpers.ts 在 tsc include 外，构建级兜底）
