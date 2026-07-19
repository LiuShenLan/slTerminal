# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

`src/stores/` 是 Zustand 状态管理层——项目唯一的全局状态真值来源。每个 store 覆盖一类状态域，面板只订阅（`useXxx((s) => s.field)`），不自存状态。PTY 进程映射在 `panels/terminal/TerminalRegistry`（模块级 Map）管理（硬约束 #8）。

## Store 清单

### `layout.ts` — 布局状态

- 极简 store：仅跟踪 `activePageId: string | null`，通过 `setActivePage` 设置。
- **不持有布局数据**——布局序列化数据（Dockview `toJSON/fromJSON`）存在 `projects.ts` 的 `OperationPage.layout` 字段。

### `fontSize.ts` — 字体大小设置

- 终端/编辑器独立字体大小（`terminalFontSize` / `editorFontSize`），默认 14，范围 [8, 32]。
- setter 内部 clamp，变更通过 Zustand `subscribe` + 2s debounce 自动保存到 `~/.slterminal/settings.json`。
- `loadFromDisk()` 在 App 启动时调用，先于项目数据加载。
- `loaded` 守卫防止启动加载阶段触发空写。
- IP 调用：通过 `src/ipc/settings` 的 `loadSettings` / `saveSettings` 读写磁盘。

### `keybindings.ts` — 快捷键自定义绑定（用户覆盖层）

- `overrides: Record<commandId, keystrokeString | null>`——用户对命令的重绑（`null` = 解绑，缺省 = 用默认键）。持久化于 `~/.slterminal/settings.json` 的 `keybindings` 段。
- 操作：`setBinding(id, ks|null)` / `clearBinding(id)` / `resetAll`。
- `loadFromDisk` 读 `saved.keybindings` 并 **sanitize**（只保留值为 `string|null` 的项，丢弃脏数据）。`loaded` 守卫防启动空写；变更 2s debounce 保存。
- **独立写入靠后端浅合并**：`save_settings` 只写 `{ keybindings }` slice，后端 `settings.rs` 浅合并 top-level 键，不擦除 `fontSize` 等其他段（故 `fontSize.ts` 无需改动，两 store 各写各的互不覆盖）。
- overrides 经 `App.tsx` 的 `wireKeybindings(getShortcutRegistry(), useKeybindings)` 注入 `ShortcutRegistry.setOverrides` 构建绑定表。
- 与 `src/features/shortcuts` 的关系：本 store 只存覆盖数据，校验/降级/绑定表构建在注册表侧（`isReserved` + `effectiveKeystroke`）。

### `sideBar.ts` — 侧栏视图状态

- 状态形状：`zones: Zones`（按钮归属——`{top: string[], bottom: string[]}`）、`open: OpenState`（各半区打开的视图 id——`{top: string|null, bottom: string|null}`）、`width: number`（侧栏区宽度，默认 250，范围 [160, 500]）、`splitRatio: number`（上下分割比例，默认 0.5，范围 [0.1, 0.9]）。
- 默认态：`DEFAULT_ZONES = {top:["projects","explorer"], bottom:[]}`、`DEFAULT_OPEN = {top:"projects", bottom:null}`。
- 操作方法：`toggleView(id)` / `moveButton(id, zone, index)` 委托 `sideBarState` 纯函数；`setWidth` / `setSplitRatio` 内部 clamp。
- 持久化照 `fontSize.ts` 模式：`loadFromDisk` 读 `~/.slterminal/settings.json` 的 `sideBar` 段 → `sanitizeSideBar`（校验+clamp）→ `reconcileZones`（对齐注册表）→ 置 `loaded:true`；变更后 2s debounce → `saveSettings({sideBar})`（后端浅合并，不擦 fontSize/keybindings 段）。
- 导出 `cancelPendingSave()` 供 App 关窗冲刷。

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
| `projects` | `projects.test.ts` | 41 | Project/Page CRUD、持久化（loadFromDisk/saveToDisk）、version 递增、ID 生成、subscribe+debounce 持久化链（`_resetPersistence()` 测试辅助） |
| `sideBar` | `sideBar.test.ts` | 19 | 默认值/toggle/move 经 store 委托纯函数、setWidth/setSplitRatio clamp、loadFromDisk 5 分支（合法/脏数据/缺失/异常/reconcileZones）、loaded 守卫、2s debounce saveSettings({sideBar}) payload 键集合精确匹配、saveSettings 失败静默吞错 |
| `layout` | `layout.test.ts` | 4 | activePageId 设置/清空/重复 |
| `fontSize` | `fontSize.test.ts` | 16 | 默认值、clamp、loadFromDisk（多种分支）、debounce 持久化 |
| `keybindings` | `keybindings.test.ts` | 16 | 默认空、setBinding/clearBinding/resetAll、loadFromDisk（合法/sanitize 脏值/缺失/非对象/异常）、loaded 守卫、debounce → saveSettings({keybindings}) |

### 测试模式

- **使用真实 store 而非 mock**：Zustand `create()` 创建的真实 store，`beforeEach` 中 `.setState()` 重置到初始状态
- **`getState()` 直接操作**：测试不依赖 React 渲染，通过 `.getState()` 操作和断言——避免 jsdom 无关的环境差异
- **持久化测试**（projects / fontSize）：mock `src/ipc/fs` 或 `src/ipc/settings` 返回模拟数据，验证 save/load 往返 + 异常降级 + `loaded` 守卫
- **debounce 测试**：用 `vi.useFakeTimers()` + `vi.advanceTimersByTime()` 控制时间，验证 debounce 窗口内的批量保存。projects 的 `_resetPersistence()` 测试辅助函数（仅 `#[cfg(test)]` 可见）重置 `initialized` 守卫和 timer，支持连续多个 debounce 测试
- **运行**：`npm test`（Vitest，含在 L2 全量中）

## 新增 Store 规则

1. 新 store 文件放在 `src/stores/` 下。
2. 在 `index.ts` 中 re-export store hook 和类型。
3. 持久化按数据类型委托对应 IPC 模块：settings 类（`fontSize`/`keybindings`）走 `src/ipc/settings`（`~/.slterminal/settings.json`）；项目数据类（`projects`）走 `src/ipc/fs`（`slterminal-projects.json`）。不在 store 内直接调用 Tauri `invoke`。
4. 避免跨 store 的隐式依赖——store 之间如需协调，在上层组件或命令中完成。
