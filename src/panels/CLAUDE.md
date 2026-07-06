# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Dockview 面板系统——所有可托管到 Dockview 布局的面板组件及其生命周期管理。

当前面板类型：
- **terminal** — xterm.js + WebGL addon + fit addon，通过 PTY 连接后端 shell
- **editor** — CodeMirror 6 + git diff gutter，文件查看/编辑

## 架构决策

### 终端：每次挂载新建 Terminal 实例

xterm.js 不支持 `term.open()` 二次调用（GitHub Issue #4978）。因此每次面板 mount 都创建新 Terminal 实例，卸载时 dispose。多 Dockview 实例 + CSS 显隐方案在 `workspace/` 层解决跨页面终端存活问题，本层不处理页面切换复用。

### WebGL 优先 + DOM 兜底

`useXterm.ts` 的 `detectWebgl()` 预检 WebGL2 可用性。可用则加载 WebglAddon（监听 `onContextLoss` 自动回退 DOM 渲染器），不可用则 DOM 渲染器兜底。仅焦点终端持有 WebGL context——`onContextLoss` 触发后 dispose addon 释放资源。

### PTY spawn 等待布局就绪

`useXterm` 挂载后不立即 spawn PTY，而是 rAF 轮询容器 `offsetWidth > 0`（最多 30 帧 / 500ms），然后 fit + proposeDimensions 获取实际字符尺寸，以真实 `cols × rows` 调用 `pty.spawn()`。超时回退 80×24。

### 编辑器：Compartment 模式切换语言

文件扩展名 → `getLanguageExtension()` 返回对应 CodeMirror 语言扩展。语言扩展通过 `Compartment.reconfigure()` 热切换，不丢失文档状态。

### Ctrl+C 保留为中断

`keyboard.ts` 的 `createTerminalShortcuts` 不注册 Ctrl+C 命令——`ShortcutRegistry` 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY，claude 用它取消操作。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：导出 TerminalPanel、EditorPanel |
| `terminal/index.ts` | TerminalPanel 及 terminalOptions 导出 |
| `terminal/TerminalPanel.tsx` | 终端面板 React 组件：获取 Windows build 号 → useXterm → 加载遮罩 |
| `terminal/useXterm.ts` | xterm.js 完整生命周期 hook：创建实例、WebGL/detect、PTY spawn、ResizeObserver、字体大小动态调节、Ctrl+Wheel 监听、输出 requestAnimationFrame 合帧、React StrictMode 双挂载守卫、E2E 钩子 |
| `terminal/keyboard.ts` | 终端快捷键命令工厂：`createTerminalShortcuts` 生成 `ShortcutCommand[]`，由 `src/features/shortcuts/ShortcutRegistry` 匹配分发 |
| `terminal/theme.ts` | xterm.js 暗色主题选项（JetBrains 配色），硬约束 #6 单点 |
| `terminal/TerminalRegistry.ts` | 模块级 `Map<panelId, RegisteredTerminal>`，跨页面切换时供查询/reattach |
| `editor/index.ts` | EditorPanel 导出 |
| `editor/EditorPanel.tsx` | 编辑器面板 React 组件：container 就位 → useCodeMirror |
| `editor/useCodeMirror.ts` | CodeMirror 6 生命周期 hook：创建 EditorView、语言扩展、字体大小动态调节、Ctrl+Wheel 监听、Ctrl+S 保存/G3 另存为、外部文件改动监听、脏状态跟踪 |
| `editor/gitGutter.ts` | CodeMirror 6 gutter 扩展：DiffHunk → RangeSet<GutterMarker> 映射、setDiffMarkers StateEffect、diffMarkersField StateField、SpacerMarker 固定宽度防光标错位 |

## 硬约束

- **#5 面板封闭**：新增面板类型流程为 `panels/<newtype>/` 创建目录 → 实现面板组件 → 在 `workspace/panelRegistry.ts` 注册 → `PANEL_TYPES` 追加类型名
- **#6 配色单点**：所有颜色从 `theme/colors.ts` token 引用（如 `GIT_GUTTER_COLORS`），禁止硬编码色值。终端配色是历史遗留的独立主题定义
- **前端不碰 OS**：面板组件和 hooks 中所有系统调用（PTY、文件读写、剪贴板、git diff）必须经 `src/ipc/` 层调用，禁止直接 `invoke`
- **IPC 边界**：`terminal/useXterm.ts` 通过 `ipc/pty` 调 spawn/write/resize/kill；`editor/useCodeMirror.ts` 通过 `ipc/fs` 读写文件、`ipc/git` 获取 diff、`ipc/dialog` 弹另存为；`terminal/keyboard.ts` 命令 handler 通过 `ipc/clipboard` 读写剪贴板；`useShortcutContext` hook 管理快捷键注册与焦点上下文

## 添加新面板类型的步骤

1. 在 `src/panels/` 下创建 `newtype/` 目录，含 `index.ts`、`NewTypePanel.tsx` 和必要的 hooks
2. 在 `src/panels/index.ts` 添加 `export { NewTypePanel } from "./newtype";`
3. 在 `src/workspace/panelRegistry.ts` 的 `panelRegistry` 对象中注册组件映射
4. 在 `PANEL_TYPES` 数组中追加 `"newtype"`
5. 如涉及新 IPC 命令，在 `src-tauri/capabilities/` 显式放行
