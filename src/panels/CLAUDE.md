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

### 编辑器：Compartment 模式切换自动换行（Alt+Z）

`Alt+Z` 触发 `editor.toggleWordWrap` 命令，通过 `wordWrapRef`（`useRef<boolean>`）跟踪当前状态、`wrapCompartment`（`Compartment`）热切换 `EditorView.lineWrapping` 扩展，不丢失文档状态。默认关闭，每编辑器实例独立，不持久化（同 VS Code 行为）。handler 经 `EditorActions.toggleWordWrap()` → `getActiveEditor()` 派发到聚焦编辑器。

### 编辑器：滚动委托 CM .cm-scroller

旧方案外层 div `overflow: auto` 是实际滚动容器，`.cm-scroller` 无溢出（`.cm-editor` `height: auto`=内容高 → `.cm-scroller` `height: 100%`=内容高 → 无溢出 → 无滚动条）。横向滚动条在外层 div 底部，长内容时需垂直滚到底才能看到。

修复分两层：
- **容器** `overflow: clip`（非 `hidden`）：`hidden` 是 CSS 滚动容器→吸收鼠标滚轮事件不传递。`clip` 裁剪但不创建滚动容器→滚轮穿透到 `.cm-scroller`
- **`.cm-editor` 高度**：`EditorView.theme({ "&": { height: "100%" } })` 给予明确高度 → `.cm-scroller` `height: 100%` 约束为视口高度 → 内容溢出 → 滚动条出现。CM6 base theme 已设 `.cm-scroller { overflowX: auto }`，CSS 规范强制 `overflowY: auto`

### 编辑器 Ctrl+S 迁入 ShortcutRegistry

`editor.save`（Ctrl+S）不再走 CodeMirror keymap。命令在 `App.tsx` 一次性注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发到聚焦编辑器；`useCodeMirror` 经 `usePanelFocus("editor", container, activate, deactivate)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CM 内部 keymap（capture/bubble 分阶段共存）。`save` 动作用 `handleSaveRef` 保持最新引用（`handleSave` 依赖 panelId 会变）。

### HTML 面板全局键转发（iframe 键盘桥）

HTML 内容在 `<iframe sandbox="allow-scripts allow-same-origin" srcDoc>` 中，iframe 内 `keydown` 不冒泡到父 `window`，焦点进入 iframe 后全局快捷键（含 Ctrl+W）本会失效。因 `allow-same-origin` 与父同源，`HtmlPanel` 在 iframe `onLoad` 时经 `attachGlobalShortcutForwarder(iframe.contentDocument)`（`features/shortcuts/forwardGlobalShortcuts.ts`）给子文档挂 capture keydown 监听：命中全局绑定（`exportContextBindings("global")`，现为 Ctrl+W）→ preventDefault + 合成 keydown 重放到父 window → `global.closeTab` 关活跃面板（即该 HTML 面板）。重载/卸载时 detach。仅重放命中全局绑定的键，页面其余键不受影响。

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

### attachCustomKeyEventHandler — 委托式 fallback

xterm.js 6.1.0-beta 升级后，ShortcutRegistry 窗口级 capture 路径在真实 WebView2 中可能因 `focusin` 未正确冒泡而使 terminal context 未激活。为双重保障，`term.open()` 后 `term.attachCustomKeyEventHandler()` **委托进注册表**（不再硬编码键位）：

```typescript
term.attachCustomKeyEventHandler((event) => {
  if (event.type !== "keydown") return true;           // keyup/keypress 透传
  const consumed = getShortcutRegistry().resolve(event, "terminal");
  if (consumed) { event.preventDefault(); return false; } // 命令已处理，不交给 xterm.js
  return true;                                          // 未命中 → 透传（Ctrl+C 等控制字符发往 PTY）
});
```

- 单一真值源：`resolve(event, "terminal")` 用与 window capture 相同的绑定表解析，`Ctrl+Shift+C/V`、`Ctrl+Enter` 均为**可重绑的注册命令**（`terminal.copy`/`terminal.paste`/`terminal.newline`），不再各处硬编码。命令 handler 经 `getActiveTerminal()` 派发到聚焦终端——多终端下 A 的 xterm handler 触发时 active=A（A 聚焦），作用正确。
- `Ctrl+Enter`（`terminal.newline`）：handler 经 `writeToPty` 写 `\n`（0x0a）到 PTY（Ctrl+J 等价，Ink 据此换行不提交）。
- 无双触发：window capture 命中即 `stopPropagation`，事件到不了 xterm，委托层不触发；仅 capture 失效时委托层用 forceContext 兜底。

### Kitty 键盘协议（CSI u）被动启用

`theme.ts` 的 `terminalOptions` 设置 `vtExtensions: { kittyKeyboard: true }`，允许子进程（如 Claude Code）通过 `CSI>1u`（Disambiguate 模式）激活差异化编码。协议为被动模式：终端声明能力后，应用需主动 push flags。若应用未激活，`KeyboardService.useKitty` 返回 `false`，回退传统 handler。

### OSC 8 超链接

xterm.js 6.0.0 原生支持 OSC 8 解析渲染。`useXterm.ts` 在 `term.open()` 后设置 `term.options.linkHandler.activate`，通过 `src/ipc/shell` 的 `openUrl()` 打开系统默认浏览器。已前置安装且 capabilities 放行，零新依赖。`hover`/`leave` 回调一期不做。

### OSC 133 命令边界检测 + 页签标题/图标动态切换

`shell-integration.ps1` 的 Enter hook 在命令执行前发射 OSC 133 C（`ESC ] 133;C;<命令行> ST`），`prompt()` 在命令退出后发射 OSC 133;D（退出码）。`useXterm.ts` 注册 `term.parser.registerOscHandler(133, ...)` 解析 C/D 序列：

- **OSC 133 C**：提取命令行文本 → `TabTitleRegistry.match(command)` 查找规则 → 匹配时通过 `onTabStateChange` 回调通知 `TerminalPanel`
- **OSC 133 D**：命令退出 → `onTabStateChange({ active: false })` → `TerminalPanel` 恢复原标题和图标

**`TabTitleRegistry`**（Registry Pattern 单例）：管理 `command → { title, icon }` 映射。新增命令只需在 `tabRules.ts` 追加 `tabTitleRegistry.register(...)`，不修改核心逻辑。

**初始化重置**：PTY spawn 成功后调 `onTabStateChange({ active: false })`，覆盖持久化残留。`PtyEvent::Exit` 时若 `isCommandRunningRef` 为 true 同样重置。

**仅限于 pwsh/powershell**——shell integration 脚本仅在 PowerShell 注入，cmd.exe 无此能力。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：导出 TerminalPanel、EditorPanel、HtmlPanel |
| `terminal/index.ts` | TerminalPanel 及 terminalOptions 导出 |
| `terminal/TerminalPanel.tsx` | 终端面板 React 组件：获取 Windows build 号 → useXterm → 加载遮罩 |
| `terminal/useTerminalInstance.ts` | Terminal 实例 + WebGL/FitAddon 生命周期 + StrictMode 守卫 |
| `terminal/usePtyOutput.ts` | PTY 输出合帧（Idle+Max 双定时器 + DEC 2026）+ 非焦点降频 |
| `terminal/usePtyResize.ts` | ResizeObserver X/Y 分离 debounce + NaN 守卫 |
| `terminal/useClipboardHandler.ts` | OSC 52 剪贴板拦截 + CJK 解码 + 焦点门控 |
| `terminal/useCommandDetection.ts` | OSC 133 命令边界检测 + TabTitleRegistry 匹配 |
| `terminal/useFontSizeBridge.ts` | 字体大小 Zustand 订阅 + Ctrl+Wheel |
| `terminal/webgl.ts` | `detectWebgl()` + `setupWebglWithRetry()` 纯函数 |
| `terminal/useXterm.ts` | 编排层（~410行），组合上述 7 个 hook，对外接口兼容 TerminalPanel |
| `terminal/keyboard.ts` | 终端快捷键命令工厂：`createTerminalShortcuts()`（无参）经 `commandFromMeta` 生成 `terminal.copy/paste/newline`，App 一次性注册；handler 经 `getActiveTerminal()` 派发到聚焦终端。Ctrl+C 不注册（透传 SIGINT） |
| `terminal/activeTerminal.ts` | 模块级"聚焦终端"指针：`setActiveTerminal`/`clearActiveTerminal`（仅匹配时清）/`getActiveTerminal`。终端聚焦时设为 active，命令 handler 据此派发 |
| `terminal/theme.ts` | xterm.js 暗色主题选项（JetBrains 配色），硬约束 #6 单点。`drawBoldTextInBrightColors` 显式声明为 `true`，消除对 xterm.js 默认值的隐式依赖（仅影响 ANSI 16 色粗体→亮色映射，不影响 True Color）。`vtExtensions: { kittyKeyboard: true }` 启用 Kitty 键盘协议被动支持 |
| `terminal/TerminalRegistry.ts` | 模块级 `Map<panelId, RegisteredTerminal>`，跨页面切换时供查询/reattach |
| `terminal/TabTitleRegistry.ts` | 命令→标题/图标映射注册表单例（Registry Pattern）。`match(command)` 精确匹配，`register(rule)` 注册规则，`_reset()` 测试用 |
| `terminal/tabRules.ts` | 规则注册文件——side-effect import 向 `tabTitleRegistry` 注册 `claude` 规则。后续新增命令在此追加 `register(...)` |
| `editor/index.ts` | EditorPanel 导出 |
| `editor/EditorPanel.tsx` | 编辑器面板 React 组件：container `overflow: clip`（裁剪不吸收滚动事件，委托 `.cm-scroller` 管理滚动；`.cm-editor` `height: 100%` 约束 scroller 高度产生溢出）→ useCodeMirror |
| `editor/keyboard.ts` | 编辑器快捷键命令工厂：`createEditorShortcuts()`（无参）经 `commandFromMeta` 生成 `editor.save`、`editor.toggleWordWrap`，App 一次性注册；handler 经 `getActiveEditor()` 派发 |
| `editor/activeEditor.ts` | 模块级"聚焦编辑器"指针：`setActiveEditor`/`clearActiveEditor`（仅匹配时清）/`getActiveEditor` |
| `editor/useCodeMirror.ts` | CodeMirror 6 生命周期 hook：创建 EditorView、`.cm-editor` `height: 100%` theme（约束 scroller 高度产生溢出→滚动条）、语言扩展、字体大小动态调节、自动换行 Compartment 热切换（Alt+Z）、Ctrl+Wheel 监听、Ctrl+S 保存（`usePanelFocus("editor")` + `setActiveEditor`，无路径则另存为）、Tab 缩进/Shift+Tab 反缩进（`keymap.of([indentWithTab])`）、Ctrl+F 搜索/撤销/重做仍归 CM keymap、外部文件改动监听、脏状态跟踪 |
| `editor/gitGutter.ts` | CodeMirror 6 gutter 扩展：DiffHunk → RangeSet<GutterMarker> 映射、setDiffMarkers StateEffect、diffMarkersField StateField、SpacerMarker 固定宽度防光标错位 |
| `html/index.ts` | HtmlPanel 导出 |
| `html/HtmlPanel.tsx` | HTML 预览面板：fs.readFile → iframe srcDoc 渲染，三态（loading/loaded/error），sandbox 沙箱，cancelled 防竞态；iframe onLoad 挂 `attachGlobalShortcutForwarder`（全局键从 iframe 转发到父 window），卸载/重载 detach |

## 硬约束

- **#5 面板封闭**：新增面板类型流程为 `panels/<newtype>/` 创建目录 → 实现面板组件 → 在 `panelRegistry.ts` 注册 → `PANEL_TYPES` 追加类型名
- **#6 配色单点**：所有颜色从 `theme/colors.ts` token 引用（如 `GIT_GUTTER_COLORS`），禁止硬编码色值。终端配色是历史遗留的独立主题定义
- **前端不碰 OS**：面板组件和 hooks 中所有系统调用（PTY、文件读写、剪贴板、git diff）必须经 `src/ipc/` 层调用，禁止直接 `invoke`
- **IPC 边界**：终端子 hook（`useTerminalInstance`/`usePtyOutput`/`usePtyResize`）通过 `ipc/pty` 调 spawn/write/resize/kill；`editor/useCodeMirror.ts` 通过 `ipc/fs` 读写文件、`ipc/git` 获取 diff、`ipc/dialog` 弹另存为；`terminal/keyboard.ts` 命令 handler 通过 `ipc/clipboard` 读写剪贴板；`usePanelFocus` hook 管理焦点上下文与聚焦实例跟踪（命令在 App 一次性注册）。`terminal/TerminalPanel.tsx` 通过 `onTabStateChange` 回调桥接 useXterm → Dockview API（`api.setTitle` / `api.updateParameters`），不引入新 IPC 命令

## 测试模式

测试文件位于 `src/__tests__/`，命名规则：`terminal*.test.ts(x)`、`editor*.test.ts(x)`、`html-panel.test.tsx`、`keyboard.test.ts`。L3 测试位于 `test/terminal/`。

### 技术栈

- Vitest（jsdom 环境）+ React Testing Library（`renderHook` / `render`）
- `@tauri-apps/api/mocks` 的 `mockIPC` 拦截 Tauri IPC
- 不使用 Playwright / Cypress / 真实浏览器

### useXterm 测试模式（111 条用例，拆分 2 文件——`use-xterm-output.test.ts` 37 条 + `use-xterm-lifecycle.test.ts` 74 条）

useXterm 是编排层——mock 7 个子 hook 才能隔离测试：

| Mock 模块 | 原因 | 关键验证 |
|-----------|------|---------|
| `../useTerminalInstance` | Terminal 实例生命周期 | 捕获 `Terminal` 构造实例到 `capturedTerminal`，供测试验证 `write`/`open`/`dispose` 调用 |
| `../usePtyOutput` | PTY 输出合帧管道 | 模拟 `handlePtyOutput`/`flushBuffer`/`cancelPendingFlush`，注入 PTY 数据验证合帧分流 |
| `../usePtyResize` | ResizeObserver + debounce | `proposeDimensions` 返回模拟 cols/rows，验证 X/Y 分离 debounce |
| `../useClipboardHandler` | OSC 52 剪贴板 | 注册 OSC 52 handler，验证 base64 解码、CJK 支持、焦点门控、>1MB 拒绝 |
| `../useCommandDetection` | OSC 133 命令边界 | 注册 OSC 133 handler，验证 C/D 序列解析 + onTabStateChange 调用 |
| `../useFontSizeBridge` | 字体大小订阅 | 模拟 Zustand store 字体大小变更 + Ctrl+Wheel 事件 |
| `../webgl` | WebGL 检测 | `detectWebgl()` + `setupWebglWithRetry()` stub，测试 `onContextLoss` 回退 |

**关键测试模式**：

- **vi.hoisted() 共享状态**：所有 mock 函数在 `vi.hoisted()` 中创建（如 `mockSpawn`、`capturedTerminal`），确保在模块级 `vi.mock()` 执行前就绪
- **PTY 输出注入**：从 `pty.spawn` mock 提取 `onOutput` 回调，`sendPtyOutput(bytes)` 注入模拟数据，验证合帧/直写分流
- **Idle+Max 双定时器**：`setTimeout(fn, 2)` 等待验证 idle timer 触发 flush；连续高频发送验证 max timer（16ms）强制 flush
- **DEC 2026 包裹**：`flushBuffer` 输出以 `\x1b[?2026h` 开头、`\x1b[?2026l` 结尾——验证原子渲染包裹
- **非焦点降频**：`rerender({ visible: false })` 后 PTY 输出不写终端；切回 `visible: true` 后立即回放缓冲
- **OSC 52 handler**：捕获 `registerOscHandler(52, ...)` 注册的回调，直接调用验证 base64 解码、CJK 支持、焦点门控、>1MB payload 拒绝
- **OSC 133 handler**：同模式捕获 `registerOscHandler(133, ...)` 回调，验证 OSC 133 C/D 序列解析 + onTabStateChange 调用
- **PTY exit + doSpawn catch + setupRetry**：mock `pty.spawn` reject 模拟 spawn 失败 → 验证错误提示 + Enter 重连 disposable 管理
- **attachCustomKeyEventHandler**：捕获注册的 handler，用 `makeKeyEvent()` 构造 `KeyboardEvent`，验证委托进 `getShortcutRegistry().resolve(event,"terminal")`——resolve→true 则返回 false+preventDefault，resolve→false 则返回 true 透传，keyup 直接透传
- **_test 接口**：`useXterm` 返回 `_test` 对象（`{ cancelPendingFlush, flushBuffer, getPendingBuffer }`），供测试直接调用内部函数
- **共享测试工厂**：`src/__tests__/helpers/xterm-test-utils.ts` 提供 `mockRaf()`/`ptyOutputSpy()`/`mockResizeObserver()`/`makeKeyEvent()` 等共享工厂，消除 7 个 describe 块中的重复定义

### 终端生命周期与组件测试

| 文件 | 用例数 | 模式 |
|------|--------|------|
| `terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整链路；mock `pty.spawn` 验证调用参数 |
| `terminal-strictmode.test.ts` | 2 | `<React.StrictMode>` 包裹验证 `smGuardRef` 防双重挂载：Terminal 实例数=1、PTY spawn 仅一次、dispose 仅在最终卸载时调 |
| `terminal.test.tsx` | 4 | TerminalPanel 组件：mock `useXterm` 返回 stub，验证 loading 遮罩/Windows build/spawn |
| `canFit.test.ts` | 15 | 纯函数边界测试：五条件守卫（null/undefined/0/isDisposed/no element） |
| `detectWebgl.test.ts` | 3 | `vi.spyOn(HTMLCanvasElement.prototype, 'getContext')` 模拟三种分支 |

### 页签标题/图标测试

| 文件 | 用例数 | 模式 |
|------|--------|------|
| `TabTitleRegistry.test.ts` | 8 | 直接测试真实 `TabTitleRegistry` 实例（非 mock）：register/match、大小写、覆盖、`_reset()`、单例校验 |
| `tabRules.test.ts` | 5 | side-effect import 验证（2 条）+ `_reset()` 后手动注册行为（3 条）。前 2 条不依赖手动 register，直接验证模块加载副作用 |
| `workspace-defaulttab.test.tsx` | 13 | MockDefaultTab 渲染（10 条）+ `onDidParametersChange` 事件结构回归测试（3 条）。事件结构测试直接验证 Dockview `Event<Parameters>` 回调行为 |

### 键盘与快捷键测试

| 文件 | 用例数 | 模式 |
|------|--------|------|
| `keyboard.test.ts` | 13 | `createTerminalShortcuts()`（无参）handler 经 `getActiveTerminal()` 派发：设 active stub 后调 handler 验证 copy/paste/newline；无 active 返回 false 透传；Ctrl+C 不注册 |
| `editor-keyboard.test.ts` | 5 | `createEditorShortcuts()` save 经 `getActiveEditor()` 派发；后设置的 active 覆盖先前的；无 active 返回 false |
| `activeTerminal.test.ts` / `activeEditor.test.ts` | 各 4 | active 指针 set/get/覆盖、clear 仅匹配时生效（防竞态） |
| `usePanelFocus.test.ts` | 5 | focusin→pushContext+onActivate、focusout(离子树)→popContext+onDeactivate、内部焦点转移不触发、卸载清理 |
| `shortcuts.test.ts` | 41 | `_reset()` 隔离；指纹 O(1) 匹配；上下文栈竞态；setOverrides 重绑/解绑/降级/冲突、resolve/forceContext、export/list |
| `globalCommands.test.ts` | 12 | `createGlobalShortcuts(getApi)` 延迟求值 DockviewApi |

### 编辑器测试

| 文件 | 用例数 | 模式 |
|------|--------|------|
| `useCodeMirror.test.ts` | 28 | `EditorState.create` 验证字体扩展；Compartment reconfigure 不重复 dispatch；handleSave 8 条（有/无 filePath、另存为、gitDiff 刷新、失败 alert、slterm:file-saved/ file-saved-as 事件） |
| `editor.test.tsx` | 9 | EditorPanel 组件：mock `useCodeMirror` 返回 stub，验证 panelId/filePath 传递 + 容器 `overflow: clip` 样式（4 条） |
| `editor-confirm.test.ts` | 11 | `renderHook(useCodeMirror)` 真实驱动；mock `onFsEvent` 保留回调引用手动触发 fs-event；覆盖订阅/取消、kind 过滤、路径匹配、脏/净状态分支 |
| `editor-font.test.ts` | 8 | 字体 CSS 选择器断言（`.cm-scroller` vs `.cm-editor`） |
| `gitGutter.test.ts` | 20 | StateEffect → RangeSet 映射验证；GutterMarker DOM 颜色断言；SpacerMarker 宽度一致性 |
| `language-mapping.test.ts` | 23 | 扩展名→语言扩展全表验证（`.js`/`.ts`/`.py`/`.rs`/`.json` 等） |

### HTML 面板测试

`html-panel.test.tsx`（18 用例）：
- 三态渲染：loading（转圈）→ loaded（iframe srcDoc）→ error（错误信息）
- 竞态取消：快速切换 filePath 时旧请求 pending resolve 不覆盖新内容
- sandbox 属性验证 + renderer="always" 生命周期

### L3 终端 headless 测试

L3 测试位于 `test/terminal/`，使用 `@xterm/headless`（零 DOM 依赖）+ `@xterm/addon-serialize`：

- 运行环境：`vitest.l3.config.ts`（`environment: 'node'`，非 jsdom），仅包含 `test/terminal/**/*.test.ts`
- `terminal-serialize.test.ts`（41 用例）：基本文本序列化、多行输出、ANSI 颜色保留、大块数据、光标定位、scrollback、resize reflow、SGR 属性叠加、多语言字符、交替屏幕、所有 ED/EL/IL/DL 擦除操作、DECSC/DECRC
- `ansi-correctness.test.ts`（29 用例）：ANSI 颜色正确性——16 色前景/背景、256 色（标准/216 色立方/灰度）、TrueColor 24-bit 前景/背景/混合、SGR 属性（粗体/斜体/下划线/双下划线/慢闪/反显/隐藏/删除线/弱化/上划线）、SGR 组合叠加、SGR 重置/子参数重置、DEC 私有模式（DECTCEM/DECOM/DECAWM）、DECSC/DECRC、RIS、DECSTBM
- `osc.test.ts`（9 用例）：OSC 序列——标题（OSC 0/2 BEL/ST）、调色板（OSC 4 单索引/多索引）、嵌入完整性（OSC 在正常输出中/穿插文本/后紧跟文本不丢失）
- `keyboard.test.ts`（37 用例）：Ctrl 组合键（A-Z 全表 + Backslash/Slash）、Alt+字母/Enter、功能键 F1-F12、Home/End/PgUp/PgDn/Insert/Delete、方向键 + Ctrl+方向键、CSI u 协议（基本键/Shift/Ctrl/Ctrl+Shift/Alt 修饰）、退格/回车/制表符渲染
- `writeSync(term, data)`：`term.write(data, callback)` 的 Promise 化辅助，必须等 callback 完成后才能 serialize

## 添加新面板类型的步骤

1. 在 `src/panels/` 下创建 `newtype/` 目录，含 `index.ts`、`NewTypePanel.tsx` 和必要的 hooks
2. 在 `src/panels/index.ts` 添加 `export { NewTypePanel } from "./newtype";`
3. 在 `src/panelRegistry.ts` 的 `panelRegistry` 对象中注册组件映射
4. 在 `PANEL_TYPES` 数组中追加 `"newtype"`
5. 如涉及新 IPC 命令，在 `src-tauri/capabilities/` 显式放行
