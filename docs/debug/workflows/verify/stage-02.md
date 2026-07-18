# Stage 2 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DBG-5**：`src/workspace/Workspace.tsx` 的 `switchToPage` 为 async；Read 确认函数体内 `await setProjectRoot(...)` 语句在 `setActivePage(pageId)` 调用**之前**；`setProjectRoot` 带 catch 降级（console.error 后继续，不中止切换）；原 early-return（同页不切换）与 `__dockviewApi` 更新逻辑保留。
- **DBG-5a**（语义式）：`Workspace.tsx:197-212` 附近的 SEC-01 effect（`prevRootRef` + `setProjectRoot` 兜底）**未被删除或改动语义**（Read 确认仍存在且按 activePageId 触发）。
- **DBG-6**：`src/App.tsx` 启动恢复段中 `setActivePage(lastPage)` 之前存在 `await setProjectRoot(...)`（rootPath 来自 projects store 查找）；带 catch 降级。
- **DBG-7**：`src/features/explorer/useFileTree.ts` 的 `loadDirectory` catch 与 effect 内 `gitStatus` catch 均含 `console.error`（grep `console.error` 命中 ≥2 处）；catch 后行为不变（`return []` / gen 检查后 setGitStatusMap）。
- **DBG-8**：`e2e-tests/helpers.ts` 中 `__slterm_e2e_createProject` 为 async 且 `await setProjectRoot(...)` 在 `setActivePage(...)` **之前**；`__slterm_e2e_switchToPage` 为 async 且含 rootPath 查找 + `await setProjectRoot(...)`；Window 接口声明同步（`Promise<string>`/`Promise<void>`）。
- **DBG-8a**（语义式）：`src/` 与 `e2e-tests/` 内除 `Workspace` SEC-01 effect 与 `projects` store 内部外，**不存在**未前置 `setProjectRoot` 的 `setActivePage(` 生产/E2E 调用点（grep `setActivePage(` 逐命中 Read 确认上下文）。
- **DBG-9**：存在 switchToPage 时序断言用例（`src/__tests__/workspace-switch-order.test.tsx` 或并入文件）：覆盖 ①setProjectRoot pending 时 activePageId 不变、resolve 后变更；②setProjectRoot reject 时仍完成切换且 console.error 被调用。测试全绿。
- **DBG-10**：存在 `src/__tests__/explorer-sandbox-race.test.tsx`：mock setProjectRoot 可控 promise，断言 resolve 前 `readDir` 调用次数为 0、resolve 后文件树渲染节点。测试全绿。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `npx vite build`（验证 `e2e-tests/helpers.ts` 打包图——该文件不在 tsc include 内）
