# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

L2 前端单元/集成测试集中目录。本文件只记录测试架构层面的约定、mock 策略与编号登记；具体用例数与测试文件清单以 `.claude/test-inventory.md` 为准。

## 关键约定

### 全局 mock 策略（setup.ts）

`setup.ts` 在全局注册三类默认 mock，避免每个测试文件重复桩写：

- `../ipc/notify`：`onFsEvent` 返回 no-op 取消函数，`startWatch`/`stopWatch` resolve。
- `../ipc/agentHooks`：`onAgentEvent` no-op；`inject`/`getInjectionStatus` 返回未注入状态。
- `@tauri-apps/api/window`：`getCurrentWindow` 返回单例 mock，含 `onFocusChanged` / `requestUserAttention`。
- `../ipc/planBalance`：`getPlanBalance`/`refreshPlanBalance` resolve 空数组，`onPlanBalanceUpdated` 返回 no-op 取消函数（F10）。

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
- **fake timers**：`fs-event` 200ms / `file-saved` 300ms debounce 需 fake timers 跨过。

## 编号登记

### MC-8 多 CLI profile 重构——测试文件更名映射

Stage 01–07 产生的测试文件更名/合并。旧名全部退役、磁盘零残留；语义逐项并入新文件。

| 旧文件（退役） | 新文件（现行） |
|----------------|----------------|
| `tab-title-registry.test.ts` | `cli-profile-registry.test.ts` |
| `tab-rules.test.ts` | `cli-profile-claude.test.ts` |
| `cli-icons.test.ts` | `cli-profile-registry.test.ts` + `cli-profile-claude.test.ts` |
| `claude-status.test.ts` | `agent-status-lib.test.ts` + `cli-profile-claude.test.ts` |
| `ipc-hooks-contract.test.ts` | `ipc-agent-hooks-contract.test.ts` |
| `ipc-claude-history-contract.test.ts` | `ipc-agent-history-contract.test.ts` |
| `claude-history-model.test.ts` | `agent-history-model.test.ts` |
| `claude-history-hook.test.tsx` | `agent-history-hook.test.tsx` |
| `claude-history-restore.test.ts` | `agent-history-restore.test.ts` |
| `claude-history-row.test.tsx` | `agent-history-row.test.tsx` |
| `claude-history-view.test.tsx` | `agent-history-view.test.tsx` |
| `claude-history-action-dialog.test.tsx` | `agent-history-action-dialog.test.tsx` |

### AC-4 / AC-5

- **AC-4**：`mock-cli-profile.test.tsx` 覆盖 mockcli 全链路契约（OSC 133 命中、eventToStatus 调用、历史聚合、hub 双向分派、恢复注入）。mock 夹具与桩编辑器位于 `helpers/mockCliProfile.ts`。
- **AC-5**：`no-claude-literals.test.ts` 对通用层八路径执行字面量守卫，禁止硬编码 `"claude"` 字符串、claude 事件名、`~/.claude` 路径。合法引用只能经 `profiles/claude/` 导出常量，且 `src/features/cliProfiles/profiles/claude/` 整目录为目录级豁免。

### TQ-CI-05

全局 mock 清单变更须同步登记「全局 mock 清单」小节；新增测试依赖真实 IPC 时须按 opt-out 方式覆盖。
