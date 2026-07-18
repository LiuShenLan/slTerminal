# Debug 修复清单（依据 refactor-regressions.md）

> 2026-07-18 · 输入：`docs/debug/refactor-regressions.md`（两个根因均已确证，含代码证据）
> 已敲定决策：故障A 用**方案A1**（setProjectRoot 前置为切换前置条件）；防复发沉淀纳入；回归测试纳入

## P0 — 故障B：PTY 契约断裂（症状③④）

| ID | 位置 | 修复要点 |
|----|------|---------|
| DBG-1 | `src/ipc/pty.ts:27-49` | `write`/`resize`/`kill` 三 wrapper 签名补 `panelId`，invoke payload 加 `panelId`（JS `panelId` ↔ Rust `panel_id` 由 Tauri 自动转换）。签名统一 `write(sessionId, panelId, data)`、`resize(sessionId, panelId, cols, rows)`、`kill(sessionId, panelId)` |
| DBG-2 | 8 处调用点 | `useXterm.ts:155`（writeToPty）、`useXterm.ts:250`（E2E write helper）、`useXterm.ts:337`（onData）、`useXterm.ts:357`（cleanup kill）、`useXterm.ts:405`（字号 resize）、`usePtyResize.ts:89`（行变化）、`usePtyResize.ts:102`（列变化）、`App.tsx:86`（关窗 kill 全部——`forEach((entry, panelId) =>` 取 Map key）。panelId 均为作用域现成值，零新增依赖 |
| DBG-3 | `src/__tests__/ipc-contract.test.ts:72-113, 145-169` | 三条参数断言用例 + 三条异常传播用例同步新签名，期望 payload 显式含 `panelId` |
| DBG-4 | `src/__tests__/ipc-contract.test.ts`（新增 describe） | 契约守卫：对 `pty_write`/`pty_resize`/`pty_kill` 断言 payload 键集合精确匹配（任何未来单边加/减键即红），防再次契约断裂 |

## P0 — 故障A：sandbox × effect 时序（症状①②，方案A1）

| ID | 位置 | 修复要点 |
|----|------|---------|
| DBG-5 | `src/workspace/Workspace.tsx:84-94` | `switchToPage` 改 async：从 `useProjects.getState()` 查 pageId 所属项目 `rootPath` → `await setProjectRoot(rootPath)`（失败 `console.error` 降级、仍继续切换）→ `setActivePage(pageId)`。SEC-01 effect（:197-212）保留兜底不动 |
| DBG-6 | `src/App.tsx:60-63` | localStorage 恢复 lastPage 前，同样先 `await setProjectRoot(对应项目 rootPath)` 再 `setActivePage(lastPage)` |
| DBG-7 | `src/features/explorer/useFileTree.ts:54-56, 229-232` | `loadDirectory` catch 与 effect 内 `gitStatus` catch 加 `console.error`（可观测性，消除静默吞错） |
| DBG-8 | `e2e-tests/helpers.ts:127-158, 187-189` | `__slterm_e2e_createProject`：`await setProjectRoot(dirPath)` 移到 `setActivePage` **之前**（函数改 async，返回 `Promise<string>`）；`__slterm_e2e_switchToPage`：增 rootPath 查找 + `await setProjectRoot`（改 async） |
| DBG-9 | `src/__tests__/`（workspace 相关） | 适配 async `switchToPage`；新增顺序断言用例：`setProjectRoot` resolve 先于 `setActivePage` 生效（调用顺序/状态时序断言） |
| DBG-10 | `src/__tests__/explorer-sandbox-race.test.tsx`（新增） | 回归用例：mock `setProjectRoot` 为可控 pending promise，断言其 resolve 前 `readDir` 未被调用；resolve 后文件树正常加载 |

## P2 — 文档同步（最后 Stage）

| ID | 位置 | 修复要点 |
|----|------|---------|
| DBG-11 | 6 处文档 | `src/ipc/CLAUDE.md`（pty 命令签名含 panelId 说明）、`src/panels/CLAUDE.md`（同步调用点描述）、`src/workspace/CLAUDE.md`（switchToPage 前置时序写入架构决策）、`e2e-tests/CLAUDE.md`（helper async 化 + setProjectRoot 先行）、`src-tauri/src/fs/CLAUDE.md`（记录 sandbox 对前端加载时序的要求：消费方须保证 project_root 已设置）、`.claude/test-inventory.md`（新增用例计数） |

## Stage 划分映射

| Stage | 包含 ID | commit 前缀 |
|-------|---------|------------|
| Stage 1 — PTY panelId 契约同步 | DBG-1, DBG-2, DBG-3, DBG-4 | `fix:` |
| Stage 2 — setProjectRoot 前置 + 可观测性 | DBG-5, DBG-6, DBG-7, DBG-8, DBG-9, DBG-10 | `fix:` |
| Stage 3 — 文档同步 | DBG-11 | `docs:` |
