# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

L2 前端单元/集成测试集中目录。本文件只记录测试架构层面的约定、mock 策略与编号登记；豁免登记见 `.claude/test-exemptions.md`。

## 关键约定

### 全局 mock 策略（setup.ts）

`setup.ts` 在全局注册三类默认 mock，避免每个测试文件重复桩写：

- `../ipc/notify`：`onFsEvent` 返回 no-op 取消函数，`startWatch`/`stopWatch` resolve。
- `../ipc/agentHooks`：`onAgentEvent` no-op；`inject`/`getInjectionStatus` 返回未注入状态。
- `@tauri-apps/api/window`：`getCurrentWindow` 返回单例 mock，含 `onFocusChanged` / `requestUserAttention`。
- `../ipc/planBalance`：`getPlanBalance`/`refreshPlanBalance` resolve 空数组，`onPlanBalanceUpdated` 返回 no-op 取消函数（F10）。
- `../ipc/backgroundTasks`：`listBackgroundTasks` resolve 两任务默认配置（planBalance/sessionRefresh 全六键），`setBackgroundTaskConfig` resolve `[]`，`onBackgroundTasksUpdated` 返回 no-op 取消函数（F12——下游 nav-tree/agent-history-hook 测试经真实 useAgentHistory → 调度器 activate 会触达 `listBackgroundTasks`，全局 mock 必须先于消费到位）。

需要真实实现的测试须在文件顶部用 `vi.mock("...", async (importOriginal) => importOriginal<...>())` 覆盖（参考 `ipc-contract.test.ts` 先例）。

### jsdom 补齐

`setup.ts` 补齐 `crypto.getRandomValues`、`ResizeObserver`、`matchMedia`、`document.fonts`、`HTMLCanvasElement.getContext`、`Range.getClientRects`，供 xterm.js / Dockview / CodeMirror 6 在 jsdom 下不抛异常。

### 共享工厂通过 globalThis 暴露

`vi.hoisted()` 运行在 ESM import 之前，无法 import 外部模块。`setup.ts` 将 `__createFsMocks` / `__createGitMocks` / `__createNotifyMocks` 注册到 `globalThis`，供 `vi.hoisted()` 回调内直接取用。

### 关键 mock 模式

- **ExplorerPanel 集成**：必须种子 `useProjects` + `useLayout` + `window.__dockviewApi`。
- **FileTree 独立渲染**：直接传 `nodes` / `gitStatusMap` props，不依赖 IPC。
- **useXterm**：编排层，需 mock 6 个子 hook 才能隔离测试（`useTerminalInstance` / `usePtyOutput` / `usePtyResize` / `useClipboardHandler` / `useCommandDetection` / webgl）。共享工厂见 `helpers/xterm-test-utils.ts`。
- **右键菜单**：`fireEvent.contextMenu` 触发；StrictMode 双渲染会导致重复元素，取 `getAllByText` 首个。
- **页签右键菜单（自研，无探针）**：目标 = `.dv-tab` 内 `data-e2e="tab-close-*"` 按钮的父级（DefaultTab 内容根——它是 `.dv-tab` 的子级，对 `.dv-tab` 自身派发冒泡不经过 DefaultTab div）；菜单渲染于容器内，`[role="menuitem"]` 查询。dockview 8.1 free core 无 contextMenuService，勿再引入 fake service 探针（workspace/CLAUDE.md「页签右键菜单自研」）。
- **fake timers**：`fs-event` 200ms / `file-saved` 300ms debounce 需 fake timers 跨过。

## 编号登记

### AC-4 / AC-5

- **AC-4**：mock profile 全链路契约（OSC 133 命中、eventToStatus 调用、历史聚合、hub 双向分派、恢复注入）。mock 夹具与桩编辑器位于 `helpers/mockCliProfile.ts`。
- **AC-5**：通用层八路径字面量守卫，禁止硬编码 `"claude"` 字符串、claude 事件名、`~/.claude` 路径。合法引用只能经 `profiles/claude/` 导出常量，且 `src/features/cliProfiles/profiles/claude/` 整目录为目录级豁免。
