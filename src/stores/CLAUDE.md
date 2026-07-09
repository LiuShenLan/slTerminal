# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

`src/stores/` 是 Zustand 状态管理层——项目唯一的全局状态真值来源。每个 store 覆盖一类状态域，面板只订阅（`useXxx((s) => s.field)`），不自存状态（硬约束 #8）。

## Store 清单

### `sessions.ts` — 会话状态（硬约束 #8 会话元数据单点）

- `SessionInfo { sessionId, panelId, cwd?, isActive }`，以 `panelId` 为 key 存入 `Record<string, SessionInfo>`。此为前端会话元数据，不包含 PTY 进程引用（与后端 PtySession 概念不同——参见 `CONTEXT.md`）。
- 操作：`setSession` / `removeSession` / `setActive`。`setActive` 对不存在的 panelId 是幂等的（直接返回原 state）。
- 面板类型只订阅 `sessions[ownPanelId]`，不自存 session 数据。
- 终端面板的 PTY 进程映射（`Map<sessionId, PtyProcess>`）不走此 store，在面板内部用模块级 `Map` 管理以绕过 React 渲染循环。

### `layout.ts` — 布局状态

- 极简 store：仅跟踪 `activePageId: string | null`，通过 `setActivePage` 设置。
- **不持有布局数据**——布局序列化数据（Dockview `toJSON/fromJSON`）存在 `projects.ts` 的 `OperationPage.layout` 字段。

### `fontSize.ts` — 字体大小设置

- 终端/编辑器独立字体大小（`terminalFontSize` / `editorFontSize`），默认 14，范围 [8, 32]。
- setter 内部 clamp，变更通过 Zustand `subscribe` + 2s debounce 自动保存到 `~/.slterminal/settings.json`。
- `loadFromDisk()` 在 App 启动时调用，先于项目数据加载。
- `loaded` 守卫防止启动加载阶段触发空写。
- IP 调用：通过 `src/ipc/settings` 的 `loadSettings` / `saveSettings` 读写磁盘。

### `projects.ts` — 项目/操作页面数据模型与持久化

- 二级模型：`Project` → `OperationPage[]`。面板由 Dockview 管理，不在此 store。
- 所有变更操作（CRUD + rename + switchToPage + updatePageLayout）自动递增 `project.version`。
- **持久化**：
  - 启动时调用 `loadAllProjects()` 从 `slterminal-projects.json` 恢复。
  - 变更通过 Zustand `subscribe` + 2s debounce 自动调用 `saveAllProjects()` 保存。
  - 初始化标记 `markPersistenceReady()` 必须在 `loadFromDisk` 之后调用，防止首次加载触发空写。
  - 关闭钩子中调用 `cancelPendingSave()` 避免竞态。
- **ID 生成**：`createProjectId()` / `createPageId()` 使用 `nextId()`（组合时间戳+自增计数器），确保单会话内唯一。
- **展开状态**：`expandedNodes` 跟踪侧栏树的折叠/展开状态，随 Project/Page 增删联动清理。
- **IP 调用**：通过 `src/ipc/fs` 的 `readFile` / `writeFile` 读写磁盘，符合硬约束 #1（前端不通 OS）。

## 消费模式

- **React 组件**通过 selector 订阅：`useProjects((s) => s.projects)`，Zustand 自动追踪 selector 返回值变化触发重渲染。
- **非 React 代码**（事件处理、清理钩子）用 `.getState()` 读写当前快照：`useProjects.getState().removePage(projId, pageId)`。
- 两类访问方式可混用——selector 用于渲染绑定，`getState()` 用于一次性副作用。

## 测试

### 测试文件

| Store | 测试文件 | 用例数 | 覆盖范围 |
|-------|---------|--------|---------|
| `sessions` | `sessions.test.ts` | 10 | setSession/removeSession/setActive、幂等性、多 session 共存 |
| `projects` | `projects.test.ts` | 37 | Project/Page CRUD、持久化（loadFromDisk/saveToDisk）、version 递增、ID 生成 |
| `layout` | `layout.test.ts` | 4 | activePageId 设置/清空/重复 |
| `fontSize` | `fontSize.test.ts` | 16 | 默认值、clamp、loadFromDisk（多种分支）、debounce 持久化 |

### 测试模式

- **使用真实 store 而非 mock**：Zustand `create()` 创建的真实 store，`beforeEach` 中 `.setState()` 重置到初始状态
- **`getState()` 直接操作**：测试不依赖 React 渲染，通过 `.getState()` 操作和断言——避免 jsdom 无关的环境差异
- **持久化测试**（projects / fontSize）：mock `src/ipc/fs` 或 `src/ipc/settings` 返回模拟数据，验证 save/load 往返 + 异常降级 + `loaded` 守卫
- **debounce 测试**：用 `vi.useFakeTimers()` + `vi.advanceTimersByTime()` 控制时间，验证 debounce 窗口内的批量保存
- **运行**：`npm test`（Vitest，含在 L2 全量中）

## 新增 Store 规则

1. 新 store 文件放在 `src/stores/` 下。
2. 在 `index.ts` 中 re-export store hook 和类型。
3. 如需持久化，委托给 `src/ipc/fs`，不在 store 内直接调用 Tauri `invoke`。
4. 避免跨 store 的隐式依赖——store 之间如需协调，在上层组件或命令中完成。
