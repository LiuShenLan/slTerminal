# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Dockview 面板系统——所有可托管到 Dockview 布局的面板组件及其生命周期管理。

当前面板类型：
- **terminal** — xterm.js + WebGL addon + fit addon，通过 PTY 连接后端 shell
- **editor** — CodeMirror 6 + git diff gutter，文件查看/编辑
- **html** — iframe + srcDoc HTML 浏览器式预览，sandbox 沙箱隔离

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

### 输出合帧策略（针对 Claude Code Ink 流式输出优化）

Claude Code 基于 Ink (React-in-terminal)，以约 60fps 全帧刷写 ANSI 序列，单次输出通常 64-200 字节（逐 token 级 ANSI 控制序列）。

**合帧管道**：`PTY 输出 → handlePtyOutput → 阈值分流 → 合帧缓冲 → 双定时器 → flushBuffer → xterm.js`

- **直写阈值 64 字节**：<64 字节（打字回显）直写终端，≥64 字节走合帧路径
- **Idle+Max 双定时器**：空闲 2ms 无新数据则 flush；最多 16ms 强制 flush 一次（防饥饿）。替代原纯 rAF 方案（对 Ink 高频小块输出合帧效果差）
- **Uint8Array 缓冲**：`pendingBufferRef: Uint8Array[]` 跳过 TextDecoder 中间步骤，合并后单次 `term.write(merged)`，减少 GC 压力 60-80%
- **DEC 2026 同步更新**：flushBuffer 中 `\x1b[?2026h` / `\x1b[?2026l` 包裹，xterm.js 6.0+ 原生支持，所有 grid 变更在单帧内原子渲染——消除撕裂
- **非焦点终端降频**：`visible=false` 时仅累积不 flush（上限 64KB），切回时立即回放。`visibleRef` 避免 `handlePtyOutput` 依赖 `visible` 导致 PTY 回调重建
- **交替缓冲 resize**：`pty.resize()` 只发 SIGWINCH 给 ConPTY/子进程，不改变 xterm.js `term.rows`/`term.cols`。网格尺寸必须由客户端 `fitAddon.fit()` → `term.resize()` 更新。因此交替缓冲中也**必须调 `fit()`** 同步 xterm.js 网格——若跳过，Ink SIGWINCH 后新尺寸输出会渲染到旧网格造成永久撕裂。交替缓冲 reflow 的短暂错位（≤1 帧）由 TUI 下一帧全量重绘覆盖

### Resize X/Y 分离 debounce + NaN 防御

针对 Claude Code Ink TUI 在 resize 后画面错位问题（调查 #3），ResizeObserver 回调采用分层策略：

- **NaN guard**：`proposeDimensions()` 在 WebGL 渲染器未就绪时可能返回 `cols/rows=NaN`（xtermjs#4338），`Number.isFinite()` 守卫防止 NaN 传入 `pty.resize()`
- **X/Y 分离**：仅行数变化（高度拖拽，廉价）→ 立即 `fit()` + `pty.resize()`；列数变化（宽度拖拽，需 re-wrap）→ 100ms debounce。`prevDimsRef` 跟踪上次尺寸区分变化类型
- **resize 前丢弃缓冲**：`cancelPendingFlush()` 在 resize 前清除 `idleTimer`/`maxTimer` + 丢弃 `pendingBufferRef`（不渲染），防止旧尺寸 PTY 数据在新视口中错位。区别于 `flushBuffer()`——后者用 DEC 2026 包裹渲染缓冲数据，与紧随其后的 `fit()` 几何变更会在同一帧产生撕裂。debounce 窗口内再次 discard 处理期间新积压数据
- **无变化跳过**：尺寸与前次完全一致时直接返回，避免无意义 fit

### OSC 52 剪贴板拦截（调查 #4）

Claude Code `/copy` 命令通过 OSC 52 序列（`ESC ] 52 ; c ; <base64> BEL`）写入系统剪贴板。
xterm.js 6.0+ 核心解析器内建 OSC 52 handler，但无 addon 时静默丢弃。`useXterm.ts` 在 `term.open()` 后注册自定义 handler：

- **仅写入**：不响应读请求（`Pd=?`），安全策略对齐行业实践（Windows Terminal/iTerm2/Alacritty）
- **仅系统剪贴板**：`c` 选择器，忽略 primary/secondary
- **焦点门控**：`visibleRef.current === false` 时忽略，防止后台 Tab 静默改剪贴板
- **Payload 上限 1MB**：防止 DoS
- **CJK 正确解码**：`atob` → `Uint8Array` → `TextDecoder.decode("utf-8")`，支持中文等非 ASCII 内容
- **零新依赖**：直接 import `src/ipc/clipboard` 的 `writeText`，与 `Ctrl+Shift+C` 共用同一写入路径
- **现有路径不受影响**：`Ctrl+Shift+C/V` 走 `keyboard.ts` → `writeText`，OSC 52 走独立 handler → 同一 `writeText`

### attachCustomKeyEventHandler — 键盘事件冗余拦截

xterm.js 6.1.0-beta 升级后，ShortcutRegistry 窗口级 capture 路径在真实 WebView2 中可能因上下文栈问题失效（`focusin` 事件在 xterm.js 6.1.0 DOM 重构后未能正确冒泡到容器元素）。为提供双重保障，`term.open()` 后调用 `term.attachCustomKeyEventHandler()`，在 xterm.js 内部 keydown 处理之前直接拦截：

- **Ctrl+Shift+C**：调 `writeText(selection)` 复制选区到系统剪贴板
- **Ctrl+Shift+V**：调 `readText()` → `term.paste(text)` 粘贴
- **Ctrl+Enter**：xterm.js 传统 handler 不区分 Ctrl+Enter 与普通 Enter（均编码为 `\r`），此处拦截并直接 `pty.write(0x0a)` 到 PTY（Ctrl+J 等价行为）。Kitty 协议虽已启用（`vtExtensions.kittyKeyboard=true`）但需应用端主动发 `CSI>1u` 激活——不可靠
- **其他按键**：返回 `true` 交由 xterm.js 默认处理

handler 必须检查 `event.type === 'keydown'`（xterm.js 对每次 keydown 均调用，keyup/keypress 忽略）。返回 `false` = 不交给 xterm.js，`true` = 透传。

### Kitty 键盘协议（CSI u）被动启用

`theme.ts` 的 `terminalOptions` 设置 `vtExtensions: { kittyKeyboard: true }`，允许子进程（如 Claude Code）通过 `CSI>1u`（Disambiguate 模式）激活差异化编码。协议为被动模式：终端声明能力后，应用需主动 push flags。若应用未激活，`KeyboardService.useKitty` 返回 `false`，回退传统 handler。

### OSC 8 超链接

xterm.js 6.0.0 原生支持 OSC 8 解析渲染。`useXterm.ts` 在 `term.open()` 后设置 `term.options.linkHandler.activate`，通过 `@tauri-apps/plugin-opener` 的 `openUrl()` 打开系统默认浏览器。已前置安装且 capabilities 放行，零新依赖。`hover`/`leave` 回调一期不做。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：导出 TerminalPanel、EditorPanel、HtmlPanel |
| `terminal/index.ts` | TerminalPanel 及 terminalOptions 导出 |
| `terminal/TerminalPanel.tsx` | 终端面板 React 组件：获取 Windows build 号 → useXterm → 加载遮罩 |
| `terminal/useXterm.ts` | xterm.js 完整生命周期 hook：创建实例、WebGL/detect、PTY spawn、ResizeObserver（尺寸变化 → cancelPendingFlush 丢弃缓冲 → fit 同步网格 → pty.resize 转发 SIGWINCH）、字体大小动态调节、Ctrl+Wheel 监听、输出 Idle+Max 双定时器合帧 + DEC 2026 同步更新 + Uint8Array 零 GC 缓冲、非焦点终端降频、OSC 52 剪贴板拦截、attachCustomKeyEventHandler（Ctrl+Shift+C/V 复制粘贴 + Ctrl+Enter 换行）、OSC 8 linkHandler、React StrictMode 双挂载守卫、E2E 钩子 |
| `terminal/keyboard.ts` | 终端快捷键命令工厂：`createTerminalShortcuts` 生成 `ShortcutCommand[]`，由 `src/features/shortcuts/ShortcutRegistry` 匹配分发 |
| `terminal/theme.ts` | xterm.js 暗色主题选项（JetBrains 配色），硬约束 #6 单点。`drawBoldTextInBrightColors` 显式声明为 `true`，消除对 xterm.js 默认值的隐式依赖（仅影响 ANSI 16 色粗体→亮色映射，不影响 True Color）。`vtExtensions: { kittyKeyboard: true }` 启用 Kitty 键盘协议被动支持 |
| `terminal/TerminalRegistry.ts` | 模块级 `Map<panelId, RegisteredTerminal>`，跨页面切换时供查询/reattach |
| `editor/index.ts` | EditorPanel 导出 |
| `editor/EditorPanel.tsx` | 编辑器面板 React 组件：container 就位 → useCodeMirror |
| `editor/useCodeMirror.ts` | CodeMirror 6 生命周期 hook：创建 EditorView、语言扩展、字体大小动态调节、Ctrl+Wheel 监听、Ctrl+S 保存/G3 另存为、外部文件改动监听、脏状态跟踪 |
| `editor/gitGutter.ts` | CodeMirror 6 gutter 扩展：DiffHunk → RangeSet<GutterMarker> 映射、setDiffMarkers StateEffect、diffMarkersField StateField、SpacerMarker 固定宽度防光标错位 |
| `html/index.ts` | HtmlPanel 导出 |
| `html/HtmlPanel.tsx` | HTML 预览面板：fs.readFile → iframe srcDoc 渲染，三态（loading/loaded/error），sandbox 沙箱，cancelled 防竞态 |

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
