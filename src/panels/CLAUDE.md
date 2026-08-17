# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

Dockview 面板系统——所有可托管到 Dockview 布局的面板组件及其生命周期管理。

当前面板类型：
- **terminal** — xterm.js + WebGL addon + fit addon，通过 PTY 连接后端 shell
- **editor** — CodeMirror 6 + git diff gutter，文件查看/编辑
- **html** — iframe + srcDoc HTML 浏览器式预览，sandbox 沙箱隔离
- **gitshow** — CM6 只读模式（`EditorState.readOnly + EditorView.editable`），查看 HEAD 中文件内容
- **diff** — 双栏 CM6 diff 面板：左侧 HEAD 只读 + HEAD gutter + 占位行，右侧工作区可编辑 + workdir gutter + 占位行
- **hooksConfig** — hooks 配置面板（F6，hub 容器）：顶部 CLI 选择行 + 编辑器槽（经选中 CLI 的 `capabilities.hooks.configEditor` 分派渲染——claude = 专属编辑器，双模式编辑 JSON/GUI settings.json 的 hooks 子树 user/project/local 三层，F2 注入入口并入；KZ-1 分派后 hub 零直接引用具体 CLI 编辑器）

## 架构决策

### 终端：每次挂载新建 Terminal 实例

xterm.js 不支持 `term.open()` 二次调用（GitHub Issue #4978）。因此每次面板 mount 都创建新 Terminal 实例，卸载时 dispose。多 Dockview 实例 + CSS 显隐方案在 `workspace/` 层解决跨页面终端存活问题，本层不处理页面切换复用。

### WebGL 优先 + DOM 兜底

`useXterm.ts` 的 `detectWebgl()` 预检 WebGL2 可用性。可用则加载 WebglAddon（监听 `onContextLoss` 自动回退 DOM 渲染器），不可用则 DOM 渲染器兜底。仅焦点终端持有 WebGL context——`onContextLoss` 触发后 dispose addon 释放资源。

**检测不带 `failIfMajorPerformanceCaveat`（FE-26）**：该标志在 Chromium GPU blocklist 场景（老 Intel 核显驱动常见）下连同软件渲染一并拒绝 → DOM renderer 回退 → 快滚整屏重绘掉帧（慢滚增量更新正常，Win10 + UHD 630 实机症状）。xterm WebGL 负载低，SwiftShader 软件渲染亦远快于 DOM renderer 全帧重建——勿加回该标志；DOM renderer 保留为 context loss 重试耗尽后的兜底（`detect-webgl.test.ts` 用例 4 守卫）。

### PTY spawn 等待布局就绪

`useXterm` 挂载后不立即 spawn PTY，而是 rAF 轮询容器 `offsetWidth > 0`（最多 30 帧 / 500ms），然后 fit + proposeDimensions 获取实际字符尺寸，以真实 `cols × rows` 调用 `pty.spawn()`。超时回退 80×24。

### windowsPty buildNumber 钳制（ADR-0004）

`term.options.windowsPty = { backend: "conpty", buildNumber: clampWindowsBuildForXterm(真实值) }`——钳制至 xterm.js ConPTY 兼容阈值下界 `XTERM_CONPTY_MIN_BUILD = 21376`（`@xterm/xterm/src/common/CoreTerminal.ts:283`）。低于该值 xterm 启用 wrapping 启发式（每次 LF + CSI H 重算 `isWrapped`），claude 全屏高频重绘下误判致 buffer 错乱（Win10 四症状）。钳制使 Win10 与 Win11 行为对齐，连带启用 resize reflow（实机验证专项）。**真实 build 号获取链不动**（`TerminalPanel` → `pty.getWindowsBuildNumber()` 传真实值），钳制收口 useXterm 两处 windowsPty 写入点；spawn 请求不带 buildNumber，后端不受影响。xterm.js 升级时须重评估此钳制点（ADR-0004）。

### 编辑器：Compartment 模式切换语言

文件扩展名 → `getLanguageExtension()` 返回对应 CodeMirror 语言扩展。语言扩展通过 `Compartment.reconfigure()` 热切换，不丢失文档状态。

### 编辑器：Compartment 模式切换自动换行（Alt+Z）

`Alt+Z` 触发 `editor.toggleWordWrap` 命令，通过 `wordWrapRef`（`useRef<boolean>`）跟踪当前状态、`wrapCompartment`（`Compartment`）热切换 `EditorView.lineWrapping` 扩展，不丢失文档状态。默认关闭，每编辑器实例独立，不持久化（同 VS Code 行为）。handler 经 `EditorActions.toggleWordWrap()` → `getActiveEditor()` 派发到聚焦编辑器。

### 编辑器：滚动委托 CM .cm-scroller

旧方案外层 div `overflow: auto` 是实际滚动容器，`.cm-scroller` 无溢出（`.cm-editor` `height: auto`=内容高 → `.cm-scroller` `height: 100%`=内容高 → 无溢出 → 无滚动条）。横向滚动条在外层 div 底部，长内容时需垂直滚到底才能看到。

修复分两层：
- **容器** `overflow: clip`（非 `hidden`）：`hidden` 是 CSS 滚动容器→吸收鼠标滚轮事件不传递。`clip` 裁剪但不创建滚动容器→滚轮穿透到 `.cm-scroller`
- **`.cm-editor` 高度**：`EditorView.theme({ "&": { height: "100%" } })` 给予明确高度 → `.cm-scroller` `height: 100%` 约束为视口高度 → 内容溢出 → 滚动条出现。CM6 base theme 已设 `.cm-scroller { overflowX: auto }`，CSS 规范强制 `overflowY: auto`

### 编辑器：CM6 主题扩展与层叠

CM6 编辑器主题来源 = **`editorTheme`**（active 方案 `editor.theme`，linear 为 oneDark 透出）+ **`editorColorOverrides()`**（active 方案 `editor.overrides`，lint/searchMatch/background/正文行号覆盖）+ **`editorSyntaxHighlight()`**（active 方案 `editor.overrides.syntax`，9 组 tag 语法高亮——**消费点须置于 `editorTheme` 之前**，数组顺序决胜，ACC-05），经 `../../theme` barrel 引用，四处消费点：useCodeMirror / GitShowPanel / DiffPanel ×2 / JsonMode。**层叠规则与特异性守卫（ACC-05 实证）见 @../theme/CLAUDE.md「editorColorOverrides 的 CM6 层叠」**——改动覆盖规则前必读：`@codemirror/view` `mountStyles()` reverse 注入使先声明主题恒胜，竞争选择器必须保持 `.cm-editor` 前缀形态。

### 编辑器 Ctrl+S 迁入 ShortcutRegistry

`editor.save`（Ctrl+S）不再走 CodeMirror keymap。命令在 `App.tsx` 一次性注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发到聚焦编辑器；`useCodeMirror` 经 `usePanelFocus("editor", container, activate, deactivate)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CM 内部 keymap（capture/bubble 分阶段共存）。`save` 动作用 `handleSaveRef` 保持最新引用（`handleSave` 依赖 panelId 会变）。

### HTML 面板全局键转发（iframe 键盘桥，postMessage 路径 + 片段拦截）

HTML 内容通过 `<iframe sandbox="allow-scripts" srcDoc={...}>` 渲染（**不含 `allow-same-origin`**）。`HtmlPanel.tsx` 通过 `injectScript()` 注入脚本（`INJECTED_SCRIPT` 常量），实现三段功能：

**1. CSS 注入**：创建 `<style>` 标签，定义 `.slterm-target { display:block !important }` 作为 `:target` CSS 伪类的 JS 替代。

**2. 键盘转发（postMessage）**：`keydown` capture → `window.parent.postMessage({type:"slterm_key", fingerprint, ...}, "null")`。父窗口 `handleMessage` 校验 `e.origin === "null"`（srcdoc opaque origin）+ `e.source === iframeRef.current.contentWindow` → `exportContextBindings("global")` 比对 → 命中则 `window.dispatchEvent(合成KeyboardEvent)`（附带 `__slterm_postMessage` 信任标记）→ `ShortcutRegistry` 分发。

**3. 片段链接拦截**：`click` capture → 检测 `<a href="#...">` → `preventDefault` + `scrollIntoView` + `classList.toggle("slterm-target")` 模拟 `:target`。WebView2 sandboxed iframe 不支持 `#fragment` 导航（srcdoc→跳父 URL、blob→"Not allowed to load local resource"），故必须 JS 拦截。

**限制**：`:target` CSS 伪类在此环境不触发。需用户 HTML 用 JS class 切换替代 `:target`。

**为何去掉 `allow-same-origin`**：`allow-scripts` + `allow-same-origin` 是已知危险组合（Chrome 警告 "can escape its sandboxing"、Tauri CVE-2024-35222），Tauri 会向同源 iframe 注入 App JS bundle 导致片段导航被 React Router 劫持。去掉后 iframe 使用 opaque origin，Tauri 不再注入。

### postMessage origin 校验与威胁模型（SEC-03）

`HtmlPanel.tsx` 的 `handleMessage` 对来自 iframe 的 `postMessage` 实施三层校验，防止恶意页面伪装键盘事件注入：

| 校验层 | 机制 | 防御目标 |
|--------|------|---------|
| **origin 校验** | `e.origin === "null"` — srcdoc iframe 为 opaque origin，按 WHATWG HTML 规范序列化为字符串 `"null"` | 阻止任意 origin 页面向父窗口发送伪造的 `slterm_key` 消息 |
| **source 校验** | `e.source === iframeRef.current.contentWindow` — 仅接受本面板 iframe 发出的消息 | 阻止同进程内其他 iframe/窗口伪装（即使 origin 同为 "null"） |
| **信任标记** | 合成 `KeyboardEvent` 上 `Object.defineProperty(event, "__slterm_postMessage", { value: true })` | 允许 `ShortcutRegistry` 识别事件来源为 postMessage 重放，与原生 keydown 区分（预留——当前未在匹配逻辑中使用） |

**威胁模型**：假设攻击者构造恶意 HTML 文件诱使用户在 slTerminal 中预览。攻击者可通过内联脚本向父窗口发送伪造的 `slterm_key` 消息，模拟 `Ctrl+W`（关闭页签）等全局快捷键。

- **无 origin 校验时**：任意窗口（包括浏览器中打开的恶意页面）均可发送消息命中 `handleMessage`，但 `source` 校验仍可拦截——因为 `iframe.contentWindow` 仅本面板可匹配
- **无 source 校验时**：同页面其他 `sandbox="allow-scripts"` iframe 可伪装键盘事件（均有 opaque origin `"null"`）
- **信任标记作用**：若未来 `ShortcutRegistry.findWinner` 需区分物理按键与 postMessage 重放（如限制重放仅作用 `global` context），标记提供判定依据。当前两路径 handler 行为一致，标记为预留机制

> **`e.origin === "null"` 为规范推断**：WHATWG HTML 规定 opaque origin 序列化为 `"null"`。此行为未经真实 WebView2 环境实测验证（单元测试用 jsdom 无法模拟 origin 校验），正确性由 E2E（L4）真实 WebView2 中 postMessage 往返验收。

### HTML 面板内联脚本/事件执行（CSP 放行）

预览 HTML 的内联 `<script>` 与内联事件属性（`onclick`/`onload` 等）能执行，依赖**全局** Tauri CSP（`src-tauri/tauri.conf.json`）含 `script-src 'self' 'unsafe-inline'` **且** `dangerousDisableAssetCspModification: ["script-src"]`。

- **为何靠全局 CSP**：`<iframe srcDoc>` 加载的 `about:srcdoc` 是 local scheme，按 WHATWG/CSP3 规范**继承父窗口 CSP**，无法通过子文档自注入宽松 meta 或改 blob:/data: 绕过——子策略只能收紧不能放宽。故必须放宽主窗口 CSP。
- **为何必须关 nonce 注入**：只加 `'unsafe-inline'` 而不关 nonce 注入时，Tauri 注入的 nonce 会按 CSP3 规范使 `'unsafe-inline'` 被浏览器忽略，srcdoc 内联脚本仍被拦。`dangerousDisableAssetCspModification: ["script-src"]` 仅关 script-src 的 nonce（不动 style-src）。
- **安全权衡**：面板仅用于预览**可信本地 HTML**。代价是主应用全局失去 script 的 nonce 加固（`default-src 'self'` 仍拦远程脚本加载，`'unsafe-inline'` 只放行内联；当前无已知注入路径：xterm 只解释 ANSI、CodeMirror 纯文本、React 全程转义）。**勿收紧回严格 script-src——会静默破坏预览**（有 L2 `csp-config.test.ts` 守卫）。
- **sandbox 不含 `allow-same-origin`**：去掉后 iframe 为 opaque origin，脚本仍通过 `'unsafe-inline'` 可执行（CSP 继承自父窗口，`script-src 'unsafe-inline'` 生效）。CSS `:target` 片段导航不再被 Tauri 注入的 React Router 劫持。

### gitshow：CM6 只读 + HEAD 内容查看 + 编辑器快捷键

`GitShowPanel` 通过 `gitFileAtHead(repoPath, oldPath ?? filePath)` 获取文件在 HEAD commit 中的内容，用 CodeMirror 6 只读模式展示。三态：loading → content / error（"该文件在 HEAD 中不存在"）。

- **只读但可聚焦**：仅用 `EditorState.readOnly.of(true)` 阻止编辑；不使用 `EditorView.editable.of(false)`（后者设 `contentEditable=false` 会导致编辑器不可聚焦，CM6 内部键绑定和 ShortcutRegistry 全部失效）
- **Ctrl+Wheel 字体缩放**：调用 `useFontSizeWheel`（`src/lib/useFontSizeWheel.ts`）共享 hook
- **Ctrl+F/Ctrl+G 搜索**：扩展列表含 `search({top:true})` + `searchKeymap` + `highlightSelectionMatches`（`@codemirror/search`，`basicSetup` 不含此功能）
- **Alt+Z 自动换行**：`wrapCompartment` + `toggleWordWrap`，通过 `usePanelFocus("editor")` → `setActiveEditor` → `ShortcutRegistry` 派发
- **字号 Compartment 热切换**：`fontCompartment.reconfigure()` 替代硬编码 `createEditorFontExtension`，字号变化不销毁重建 EditorView
- **callback ref 容器桥接**：`<div>` 在 content 态才挂载，`ref.current` 在 render 期间为 null。`useCallback` ref + `setRenderKey` 触发额外渲染，确保 `useFontSizeWheel`/`usePanelFocus` 在容器就绪后收到非 null DOM 元素
- **大文件阈值复用**：从 `useCodeMirror` 导出 `MAX_FILE_SIZE_BYTES` / `LARGE_FILE_WARN_BYTES`，超限拒绝/警告。禁止新造数值
- **大文件警告图标（FE-18）**：警告 header 行首图标经 CM6 `Decoration.widget` 注入（`LargeFileWarnWidget` + `largeFileWarnField` StateField，仅警告分支挂载；doc 只读 update 仅 map）——⚠ 字符已移除（IC-08），lucide `IconAlertTriangle` 13px、色经 `GIT_FILE_COLORS.modified` warning 语义 token（硬约束 #6）。widget 渲染经 react-dom `createRoot`（`destroy` 时 unmount）
- **错误契约**：catch 任意错误 → 占位文案"该文件在 HEAD 中不存在"，不解析错误内容
- **语言扩展复用**：`getLanguageExtension(filePath)` 从 `useCodeMirror` 导出复用
- **oldPath 优先**：renamed 场景传 oldPath 查询 HEAD 中旧路径内容

### diff：双栏占位对齐 + 滚动同步 + 双侧 gutter

`DiffPanel` 横向均分两栏（flex 50/50）：左 = HEAD 只读 CM + HEAD gutter + 占位行，右 = 工作区可编辑 CM + workdir gutter + 占位行。

**占位对齐（`alignment.ts`）**：`computeAlignment(hunks)` 纯函数根据 DiffHunk[] 计算左右两侧需插入占位行的位置与数量——纯新增行左侧插占位，纯删除行右侧插占位，行数不等侧插差值。结果通过 CM6 `Decoration.widget` 渲染块级占位行（不可选中、不响应指针事件）。规则：oldLines=0→左侧插入 newLines 个；newLines=0→右侧插入 oldLines 个；modified 不等→少的一侧插差值；等行→无需占位。纯函数零 DOM 访问（照 dropTarget 模式）。

**垂直滚动同步**：一侧 `.cm-scroller` scroll 事件 → 另一侧 `scrollTop` 跟随（`syncingRef` 防循环）。水平滚动独立，不强制同步。

**双侧 gutter**：左侧 HEAD 侧使用 `headDiffGutter`（old 行号映射——纯新增无标记、修改标 ModifiedMarker、删除标 DeletedMarker）；右侧工作区侧复用 `diffGutter`（new 行号映射——纯新增标 AddedMarker、修改标 ModifiedMarker、删除标 DeletedMarker）。

**右侧 Ctrl+S**：通过 `usePanelFocus("editor")` + `setActiveEditor` 注册为聚焦编辑器。save = `fs.writeFile` 写回 → 重新 `gitDiff` → 刷新双侧 gutter + 占位对齐。

**左侧 .git 变更刷新**：`onFsEvent` 检测 `.git` 路径变更 → 重取 HEAD 内容更新左侧 CM。

**右侧外部修改**：`onFsEvent` 检测文件路径匹配 → 净自动重载 / 脏弹窗确认（同 editor 语义）。

**编辑器快捷键**（同 gitshow 模式）：
- **只读但可聚焦**：左栏仅用 `EditorState.readOnly.of(true)`，不使用 `EditorView.editable.of(false)`
- **Ctrl+Wheel 字体缩放**：左栏/右栏容器各调用一次 `useFontSizeWheel`
- **Ctrl+F 搜索**：左栏和右栏均注册 `search({top:true})` + `searchKeymap` + `highlightSelectionMatches`
- **Alt+Z 自动换行**：左右栏独立 `Compartment`（`leftWrapCompartment`/`rightWrapCompartment`），`toggleWordWrap` 同步切换两侧，通过 `usePanelFocus("editor")` 在左右栏均注册
- **字号 Compartment 热切换**：左右栏各自独立 `fontCompartment`（CM6 Compartment 绑定到特定 EditorState，不可跨 view 共享），`useEffect` 监听 `editorFontSize` 变化做 `reconfigure`；CM6 创建 effect deps 不含 `editorFontSize`

**CSS flexbox `min-width: auto` 与 `minWidth: 0` 修复**：

DiffPanel 的 DOM 层级为双层 flex 嵌套——外层 `diff-panel`（flex row）→ wrapper（flex: 50%）→ diff-left/diff-right（flex: 1, overflow: clip）。CSS flexbox 默认 `min-width: auto` 使 flex 子项拒绝收缩到内容宽度以下——CM6 `.cm-content`（`flex-shrink: 0`，`white-space: pre`）随长行横向扩展，逐层撑开 flex 子项，导致分界线偏离 50% 中线。`overflow: clip` 不创建 scroll container，因此不触发 flexbox 的 min-width 归零规则（仅 `overflow: hidden|scroll|auto` 有此效果）。

**修复**：四个 flex 子项（左右 wrapper + diff-left/diff-right）均加 `minWidth: 0`，显式覆盖 `min-width: auto`。`overflow: clip` 保留——裁剪溢出但不吸收滚轮事件，确保滚轮穿透到 `.cm-scroller`。

**容器 ref 桥接（renderKey + bridgedRef）**：

DiffPanel 的加载/错误/就绪三态中，容器 div 仅在 `"ready"` 态挂载。`useRef` 在 React commit 后才绑定 DOM 元素——首次 `"ready"` 渲染期间 `ref.current` 仍为 null。旧代码的 `useEffect([], [])` bridge（右容器）和直接 `ref.current` 读取（左容器）均无法在条件渲染下获取非 null DOM 元素。

**修复**：`renderKey` state + `bridgedRef` guard + `useEffect([state.kind])`——effect 在 commit 后运行（ref 已绑定），设 `bridgedRef = true` 防重入，`setRenderKey` 触发额外渲染。`state.kind !== "ready"` 时重置 `bridgedRef`，支持 filePath 切换后重新桥接。效果：`useFontSizeWheel` / `usePanelFocus` 在 bridge 重渲染时以非 null 容器执行。

### hooksConfig：hub 容器 + claude 专属编辑器（MC-502~508）

`HooksConfigPanel`（F6）为 **hub 容器**（Stage 06，MC-502~507）：顶部 CLI 选择行（遍历 `cliProfileRegistry.getAll()` 过滤 `capabilities.hooks?.hasConfigEditor === true`，按钮 = iconSrc 16×16 logo + displayName，选中态高亮走 theme token）+ 编辑器槽（**KZ-1 分派**：`const Editor = selectedProfile?.capabilities?.hooks?.configEditor`——Editor 存在则 `<Editor key={selectedProfile.id} profile={...} onDirtyChange={...} askGuardRef={...} />`，缺失则空态占位「该 CLI 未提供配置编辑器」；hub 不直接引用任何具体 CLI 编辑器，新增 CLI 自带编辑器组件挂入 profile 即接入）。选中态 `params.selectedCli` 随布局 JSON 持久化（照 F8 customTitle 先例——`api.updateParameters` + **显式 `onLayoutChange(saveLayout(api))`**，updateParameters 不触发 onDidLayoutChange 须显式保存；挂载读 params 恢复，缺省/失效回退首个有能力 CLI）。切换 CLI = 卸载当前编辑器并重挂载目标编辑器（ADR-0001 先例），dirty 时 `confirmDialog` 确认丢弃（`src/lib/ConfirmDialog`，UI-801/803——原 `dialog.ask` 已退役 OV-02；askGuard 防循环）。无任何 hasConfigEditor profile → 「无可配置 CLI」占位（不渲染编辑器）；单 CLI 也渲染选择行（防布局跳动）。

**claude 专属编辑器**（MC-223/508，决策 2）：`ClaudeHooksConfigEditor`（原 HooksConfigPanel 全部内容整体下移一层，P3-FE-02/11/12/16/17/19/21/22）编辑 settings.json 的 **hooks 子树**（C13-1 编辑范围），三层配置：`user`（`~/.claude/settings.json`）/ `project`（`<projectPath>/.claude/settings.json`）/ `local`（`<projectPath>/.claude/settings.local.json`），优先级 Local > Project > User。**层集合声明于 claude profile 的 `capabilities.hooks.configLayers`（KZ-4）**——编辑器层切换器数据源 = `profile.configLayers`（模块级 LAYERS 常量已退役，三层值 + label/hint 文案迁入 `features/cliProfiles/profiles/claude/`），初始层 = `configLayers[0].id`；「project/local 需 rootPath」禁用判定与优先级标注（PRIORITY_HINT）是 claude 知识，保留编辑器内部硬编码。claude hooks 协议知识（eventsCatalog 30 事件/matcherEngine/5 种 handler 字段矩阵/schema 内嵌/Draft07 校验）**不抽象**、文件物理位置保留现状——**KZ-1 分派后 hub 零直接引用本组件**：整文件仅经 claude profile 的 `capabilities.hooks.configEditor` 引用（挂载于 `features/cliProfiles/profiles/claude/`，features→panels 依赖方向合法化见 @../features/cliProfiles/CLAUDE.md），hub 内 IPC 实参一律来自选中态 cliId（Stage 03 临时代理常量已回收，MC-220）。

- **双模式编辑**（P3-FE-16）：默认 JSON 模式（CM6 + codemirror-json-schema 悬停/波浪线 + 事件导航侧栏 + MatcherTester 内联试测），GUI 模式为 Master-Detail（EventTree 事件树 + HandlerForm 表单）。`configJson` / `guiModel` / `dirty` 共享于 `useHooksConfig`——JSON 编辑经 `jsonToGui` 重算 GUI，GUI 编辑经 `guiToJson` 回写 JSON。非法 JSON（`onValidationChange` 上报）→ 禁切 GUI + 禁用保存。schema 内嵌于 `src/features/hooksConfig/schema/claude-code-settings.json`（SchemaStore 官方 schema + hooks 子 schema 提取，本地 `$ref` 自包含已核实）。
- **hooks 子树三层配置**：rootPath 为空（无活跃项目）时 project/local 层禁用（仅 user 可用）；切层 / 页面重新可见（document.visibilitychange，visibilityState === "visible"，面板可见时）轻量重读做外部修改检测，dirty 时 `confirmDialog` 确认丢弃（confirm 弹窗打开/关闭的回归触发由 askGuard 抑制——验收 2.1 弹窗循环根因；select 下拉等页面内焦点转移不触发 visibilitychange；窗口移动/缩放全程可见不触发——拖动窗口标题框不误弹）；后端 `agent_hooks_config_write` read-modify-write merge 原样保留 permissions/env 等其他字段（P3-BE-03），前端不做 .bak。
- **保存安全**（P3-FE-17）：JSON.parse + `validateHooksJson`（json-schema-library Draft07，非 ajv）双校验，任一失败弹窗拒绝写盘 → 写盘；成功后提示条文案由 `profile.hooks.restartHint` 驱动（claude 值 =「hooks 改动需重启 claude 会话生效」，MC-506；`data-e2e="hooks-restart-hint"` 保留）。
- **注入段保护**（C13-8）：`command` 含 `slterm-hook-reporter` 子串的条目（`isSltermManaged`，识别规则照 C9）GUI 标记「slTerminal 托管」+ 禁删/表单只读；**JSON 模式不限制**（用户对自己文件有最终权利）。
- **F2 并入**（P3-FE-21/22）：工具栏「注入 Hooks」/「卸载 Hooks」按钮调用 `src/ipc/agentHooks` 的 `inject()`/`uninstall()`（cliId 实参 = hub 选中态，MC-221），状态条显示 `getInjectionStatus()` 三态（已注入/未注入/版本过旧）；注入/卸载完成后自动重读 user 层配置（操作改写 `~/.claude/settings.json`，C13-8）——当前层非 user 则切到 user 层。
- **同页单例**（C13-7）：面板 id = `hooksConfig-{activePageId}`，入口为活动栏底部「配置」钮（NAV-05 决策 4 入口唯一化——原 SidebarTree 右键菜单项随其退役；`hooksConfig/openHooksConfig.ts` 的 `openHooksConfigFromActivityBar`：先 `switchToPageShared` 切页 → `openHooksConfigPanel(pageId)`，见 workspace/pageApis）命中 `getPanel(id)` 聚焦、未命中 addPanel；面板 props 兼容 Dockview（无需依赖 panelId 的单例语义）。

### Ctrl+C 保留为中断

`keyboard.ts` 的 `createTerminalShortcuts` 不注册 Ctrl+C 命令——`ShortcutRegistry` 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY，claude 用它取消操作。

### 输出合帧策略（终端平台能力：针对 Ink 系 TUI 流式输出优化）

Ink 系 TUI（设计动机：Claude Code 基于 Ink (React-in-terminal)）以约 60fps 全帧刷写 ANSI 序列，单次输出通常 64-200 字节（逐 token 级 ANSI 控制序列）。合帧管道是终端平台能力，对全部子进程生效。

**合帧管道**：`PTY 输出 → handlePtyOutput → 阈值分流 → 合帧缓冲 → 双定时器 → flushBuffer → xterm.js`

- **直写阈值 64 字节**：<64 字节（打字回显）直写终端，≥64 字节走合帧路径
- **Idle+Max 双定时器**：空闲 2ms 无新数据则 flush；最多 16ms 强制 flush 一次（防饥饿）。替代原纯 rAF 方案（对 Ink 高频小块输出合帧效果差）
- **Uint8Array 缓冲**：`pendingBufferRef: Uint8Array[]` 跳过 TextDecoder 中间步骤，合并后单次 `term.write(merged)`，减少 GC 压力 60-80%
- **DEC 2026 同步更新**：flushBuffer 中 `\x1b[?2026h` / `\x1b[?2026l` 包裹，xterm.js 6.0+ 原生支持，所有 grid 变更在单帧内原子渲染——消除撕裂
- **非焦点终端降频**：`visible=false` 时仅累积不 flush（上限 64KB），切回时立即回放。`visibleRef` 避免 `handlePtyOutput` 依赖 `visible` 导致 PTY 回调重建
- **交替缓冲 resize**：`pty.resize()` 只发 SIGWINCH 给 ConPTY/子进程，不改变 xterm.js `term.rows`/`term.cols`。网格尺寸必须由客户端 `fitAddon.fit()` → `term.resize()` 更新。因此交替缓冲中也**必须调 `fit()`** 同步 xterm.js 网格——若跳过，Ink SIGWINCH 后新尺寸输出会渲染到旧网格造成永久撕裂。交替缓冲 reflow 的短暂错位（≤1 帧）由 TUI 下一帧全量重绘覆盖

### Resize X/Y 分离 debounce + NaN 防御

针对 Ink 系 TUI 在 resize 后画面错位问题（设计动机：Claude Code；调查 #3），ResizeObserver 回调采用分层策略：

- **NaN guard**：`proposeDimensions()` 在 WebGL 渲染器未就绪时可能返回 `cols/rows=NaN`（xtermjs#4338），`Number.isFinite()` 守卫防止 NaN 传入 `pty.resize()`
- **X/Y 分离**：仅行数变化（高度拖拽，廉价）→ 立即 `fit()` + `pty.resize()`；列数变化（宽度拖拽，需 re-wrap）→ 100ms debounce。`prevDimsRef` 跟踪上次尺寸区分变化类型
- **resize 前丢弃缓冲**：`cancelPendingFlush()` 在 resize 前清除 `idleTimer`/`maxTimer` + 丢弃 `pendingBufferRef`（不渲染），防止旧尺寸 PTY 数据在新视口中错位。区别于 `flushBuffer()`——后者用 DEC 2026 包裹渲染缓冲数据，与紧随其后的 `fit()` 几何变更会在同一帧产生撕裂。debounce 窗口内再次 discard 处理期间新积压数据
- **无变化跳过**：尺寸与前次完全一致时直接返回，避免无意义 fit

### OSC 52 剪贴板拦截（调查 #4）

OSC 52 剪贴板写入是终端平台能力，对全部子进程生效（设计动机：Claude Code `/copy` 命令通过 OSC 52 序列（`ESC ] 52 ; c ; <base64> BEL`）写入系统剪贴板）。
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

`theme.ts` 的 `terminalOptions` 设置 `vtExtensions: { kittyKeyboard: true }`（终端平台能力，设计动机 Ink 系 TUI，对全部子进程生效），允许子进程（如 Claude Code）通过 `CSI>1u`（Disambiguate 模式）激活差异化编码。协议为被动模式：终端声明能力后，应用需主动 push flags。若应用未激活，`KeyboardService.useKitty` 返回 `false`，回退传统 handler。

### OSC 8 超链接

xterm.js 6.0.0 原生支持 OSC 8 解析渲染。`useXterm.ts` 在 `term.open()` 后设置 `term.options.linkHandler.activate`，通过 `src/ipc/shell` 的 `openUrl()` 打开系统默认浏览器。已前置安装且 capabilities 放行，零新依赖。`hover`/`leave` 回调一期不做。

### OSC 133 命令边界检测 + 页签标题/状态圆点动态切换

`shell-integration.ps1` 的 Enter hook 在命令执行前发射 OSC 133 C（`ESC ] 133;C;<命令行> ST`），`prompt()` 在命令退出后发射 OSC 133;D（退出码）。`useXterm.ts` 注册 `term.parser.registerOscHandler(133, ...)` 解析 C/D 序列：

- **OSC 133 C**：提取命令行文本 → 调用 `onTabStateChange({ active: true, title, status: "attention" })` 设置 attention 态；同时 `cliProfileRegistry.matchByCommand(command)` 查 profile → 命中时覆盖 `title` = profile.tabTitle，并 `setAgentSession({ cliId: profile.id })` 写入会话（F9 行为修订：logo 不经 C 路径直传——页签 logo 由 TerminalPanel 订阅 sessionChange 按 agentSession.cliId 查 profile.iconSrc 驱动，会话绑定）。**B12：先写会话再发回调**——TerminalPanel 的 originalTitleRef 捕获守卫检查 agentSession 非空即跳过，回调触发 onDidTitleChange 时会话必须已置位
- **OSC 133 D**：命令退出 → `onTabStateChange({ active: false })`（restoreTitle 缺省 true）→ `TerminalPanel` 恢复原标题并单清状态；`setAgentSession(null)` → sessionChange 驱动清 logo（F8 后原标题 = `customTitle` 优先，用户重命名过的终端恢复自定义名）

**`TabState.restoreTitle` 信号（B13）**：`active=false` 时是否恢复原标题（缺省 true；false = 仅清状态圆点）。调用点语义分工——**真退出信号**（OSC 133 D / PTY EXIT）缺省恢复；**SessionEnd/Exit hook 事件与 spawn 初始化重置**传 `restoreTitle:false`（/resume 的 SessionEnd→SessionStart 序列中 claude 进程未退出，恢复会把标题误回退为 terminal-N；spawn 初始化恢复会抹掉 B12 重算结果）。

**`CliProfileRegistry`**（Registry Pattern 单例，见 @../features/cliProfiles/CLAUDE.md）：管理 `command → profile` 映射（`matchByCommand` 首 token 精确查表——覆盖 `claude --resume` / `claude -p` 等带参变体）。新增 CLI 只需在 `features/cliProfiles/profiles/` 追加 profile 注册，不修改核心逻辑；标题与品牌 logo 均取自 profile（tabTitle / iconSrc，F9——F9 行为修订后 logo 经 sessionChange 按 cliId 查询，非 C 路径直传）。

**初始化重置**：PTY spawn 成功后调 `onTabStateChange({ active: false, restoreTitle: false })`（B13：仅清状态不恢复标题——防抹掉 B12 重算结果），覆盖持久化残留。`PtyEvent::Exit` 时若 `isCommandRunningRef` 为 true 同样重置（**真退出信号，缺省恢复标题**）。

**仅限于 pwsh/powershell**——shell integration 脚本仅在 PowerShell 注入，cmd.exe 无此能力。

### F3 页签四态指示（agent-event + StatusDot 圆点）

终端页签通过双源事件合成四态状态（渲染层 = `StatusDot` 圆点，IC-03），优先级自上而下：

| 状态 | 圆点色 | 触发源 | 说明 |
|------|--------|--------|------|
| `working` | 绿 | agent-event `PreToolUse`/`PostToolUse` | Claude Code 正在执行工具调用 |
| `attention` | 黄 | OSC 133 C（命令开始）或 agent-event Notification | 用户命令运行中或需要关注（notificationType 为 `permission_prompt`/`idle_prompt`/`agent_needs_input`） |
| `done` | 灰 | agent-event `Stop` | 主代理完成响应输出（`SessionEnd`/OSC 133 D 由 hook/命令层清状态，不产生 ✅） |
| `error` | 红 | agent-event `PostToolUseFailure`/`StopFailure` | 工具调用失败或轮次因 API 错误结束 |

实现（**`params.tabIcon` emoji/img 分支已退役，IC-03 改 `params.tabStatus` → `StatusDot`**）：

- **`useCommandDetection`**：OSC 133 C 触发时经 `cliProfileRegistry.matchByCommand(command)` 匹配（MC-105：命中 → **B12 先 `setAgentSession` 后** `onTabStateChange({ active: true, title: profile.tabTitle, status: "attention" })`，未命中零副作用）；OSC 133 D 重置（restoreTitle 缺省 true）；resetCommandState 传 `restoreTitle:false`（B13）。**F9 行为修订：`TabState.logo` 字段已退役**——页签 logo 会话绑定（见 TerminalPanel 行）；`TabState.icon` 字段同退役——状态经 `status` 字段传递
- **`useXterm`**：新增 `onAgentEvent` 订阅 → 按 `panelId` 过滤 → 来源 CLI 经 `resolvePayloadCliId` 单点解析（MC-205 三级解析，`src/panels/terminal/resolvePayloadCliId.ts`，ZQ-2 契约 4——空串/空白 cliId 同等回退）→ `eventToStatus(event, notificationType?)`（经 `profile.hooks` 委托，claude 实现在 `src/features/cliProfiles/profiles/claude/strategies.ts`）→ `onTabStateChange({ active: true, status })`；清状态判定 `SessionEnd ∨ Exit` 双事件（ZQ-6，与删 agentSession 判定对齐）调 `{ active: false, restoreTitle: false }`（**B13：仅清状态不恢复标题**）；**SessionStart 分支补 title + 异步标题覆盖（人工验证问题 3）**：先同步 `onTabStateChange({ active: true, status, title: profile?.tabTitle })`（/resume 无 OSC 133 C，标题经 hook 事件保持 claude），再 `readHistoryTitle(cliId, sessionId)` 异步读历史同源标题覆盖（null 兜底 tabTitle）；非 SessionStart 事件 5s 节流重查；陈旧守卫 + 标题去重 + 失败静默
- **`TerminalPanel.handleTabStateChange`**：`active=true` 时只有 `title` 存在才 `setTitle`，只有 `status !== undefined` 才 `updateParameters({ ...latestParamsRef.current, tabStatus: status })`；`active=false` **`restoreTitle !== false` 才恢复原标题**（B13）+ **单清** status。**页签 logo 会话绑定（F9 行为修订）**：`logoRef` 退役——TerminalPanel 订阅 `TerminalRegistry.subscribe`（register/sessionChange 事件过滤 panelId）→ 读 `get(panelId)?.agentSession` → session 非 null 时 `tabLogo = cliProfileRegistry.get(session.cliId ?? CLAUDE_CLI_ID)?.iconSrc ?? null`（cliId 缺省兜底口径与 useAgentStatus 行建行一致），null/undefined → `tabLogo = null` → `updateParameters({ ...latestParamsRef.current, tabLogo })`；挂载同步一次覆盖布局 JSON 残留、页面切回（H6）register 幂等保留旧 session 即恢复；deps 仅 `[api, panelId]`（params 经 ref 读——tabStatus 高频更新不重建订阅）。**`latestParamsRef` 参数合并单点（参数覆盖回归修复）**：tabStatus/tabLogo 分头经 `updateParameters` 写入互不可见——合并基准 = `latestParamsRef`（props 同步 + `onDidParametersChange` 合并），禁止用 props 快照覆盖（快照抹掉另一路径刚写入的键——mockcli E2E 冒烟 tabStatus 丢失根因）。**visible 判定（B14）**：`activePageId != null && panelId.startsWith(`terminal-${activePageId}-`)` 前缀匹配（旧恢复格式含 Date.now 数字段，贪婪正则/切分会误解析——历史恢复黑屏根因）。**originalTitleRef 同步（B12）**：`onDidTitleChange` 订阅——customTitle 存在或 agentSession 非空不捕获，其余捕获（布局恢复重算标题后同步，真退出恢复重算名而非持久化瞬态值）
- **`DefaultTab`**（workspace 层，见 @../workspace/CLAUDE.md）：`tabStatus` 非 null → `StatusDot` 圆点（IC-03）；`tabLogo`（`params.tabLogo`）在圆点后渲染 16×16 CLI logo（F9 行为修订：**跟随页签名显示**——`tabLogo` 单条件，不依赖 tabStatus；圆点缺席时 logo 顶到标题前）；文件型页签（`params.filePath`）渲染 FileIcon 彩色图标
- **profile 注册表**（Stage 01）：claude profile 身份域只含 `tabTitle`/`iconSrc` 等数据，不含硬编码事件图标——圆点色由 F3 四态系统 + StatusDot 色映射接管（原 `tabRules.ts` 已退役）

### 中断场景已知行为（Ctrl+C）

Claude Code 在用户主动 Ctrl+C 中断时不发射任何 hook 事件（`Stop` = 主代理完成响应输出，`StopFailure` = 轮次因 API 错误结束；中断既非正常完成也非 API 错误）。此设计导致四态状态机 `working`（⚡）无中断出边——中断后页签滞留 ⚡ 直至下一事件覆盖。

行为特征（非 bug，属于阶段 1 规划缺口）：
1. **滞留自愈**：下一事件（UserPromptSubmit/Stop 等）覆盖旧状态——继续使用后图标自动恢复正常流转
2. **内置衰减**：中断回提示符约 60s 无操作 → `idle_prompt` Notification（attention 子类型）→ 自动转 🟡，无需超时机制
3. **已知局限**：`eventToStatus`（`src/features/cliProfiles/profiles/claude/strategies.ts`）无中断类事件映射（清图标不受影响——`useXterm` agent-event 处理已按 `SessionEnd ∨ Exit` 双事件清图标，ZQ-6）

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口：导出 TerminalPanel、EditorPanel、HtmlPanel、GitShowPanel、DiffPanel、HooksConfigPanel |
| `terminal/index.ts` | TerminalPanel 及 terminalOptions 导出 |
| `terminal/TerminalPanel.tsx` | 终端面板 React 组件：获取 Windows build 号 → useXterm → 加载遮罩；`originalTitleRef` 挂载时取 `params.customTitle ?? api.title ?? "terminal"`（F8 自定义标题优先）并订阅 `onDidParametersChange`（customTitle 同步）+ `onDidTitleChange`（B12：customTitle 存在或 agentSession 非空不捕获，其余捕获重算标题）——OSC 133 D 恢复标题用自定义名/重算名；**页签状态圆点（IC-03）**：`handleTabStateChange` 写 `updateParameters({ ...latestParamsRef.current, tabStatus })`（null 清状态）；**页签 logo 会话绑定（F9 行为修订）**：`logoRef` 退役，订阅 TerminalRegistry（register/sessionChange）→ agentSession 非 null 按 `cliId ?? CLAUDE_CLI_ID` 查 iconSrc 写 `tabLogo`、null 清 `tabLogo`；inactive 单清 tabStatus（restoreTitle=false 时跳过标题恢复，B13）；**visible 前缀匹配（B14）**：`activePageId` 与 `panelId` 前缀比对（旧恢复格式数字段兼容）；**`latestParamsRef` 参数合并单点（参数覆盖回归修复）**：tabStatus/tabLogo 分头经 `updateParameters` 写入互不可见——合并基准 = `latestParamsRef`（props 同步 + `onDidParametersChange` 合并），禁止用 props 快照覆盖（快照抹掉另一路径刚写入的键——mockcli E2E 冒烟 tabStatus 丢失根因；terminal.test.tsx 两键共存断言防复发） |
| `terminal/useTerminalInstance.ts` | Terminal 实例 + WebGL/FitAddon 生命周期 + StrictMode 守卫 |
| `terminal/usePtyOutput.ts` | PTY 输出合帧（Idle+Max 双定时器 + DEC 2026）+ 非焦点降频 |
| `terminal/usePtyResize.ts` | ResizeObserver X/Y 分离 debounce + NaN 守卫 |
| `terminal/useClipboardHandler.ts` | OSC 52 剪贴板拦截 + CJK 解码 + 焦点门控 |
| `terminal/useCommandDetection.ts` | OSC 133 命令边界检测 + `cliProfileRegistry.matchByCommand` 匹配（MC-105：title 取 profile.tabTitle）+ attention 状态（OSC 133 C 触发 `onTabStateChange({ active: true, title, status: "attention" })`，B12 先 setAgentSession 后回调）；`TabState` 迁入本文件顶部导出（Stage 01 退役 TabTitleRegistry 后；F9 行为修订：`logo`/`icon` 字段退役——logo 会话绑定由 TerminalPanel 订阅驱动、状态经 `status` 字段；**B13：新增 `restoreTitle?: boolean` 字段**——false = 仅清状态不恢复标题，resetCommandState 传 false） |
| `terminal/webgl.ts` | `detectWebgl()` + `setupWebglWithRetry()` 纯函数 |
| `terminal/useXterm.ts` | 编排层（~420 行），组合上述 6 个 hook + `src/lib/useFontSizeWheel`（Ctrl+Wheel 字体缩放）+ `onAgentEvent` 订阅（按 panelId 过滤 → 来源 CLI 经 `resolvePayloadCliId` 单点三级解析（ZQ-2）→ `eventToStatus` → F3 四态 status（圆点渲染在 DefaultTab/StatusDot）；SessionEnd/Exit 双事件清状态（ZQ-6）；非 SessionEnd/Exit 时 `setAgentSession` 携 `sessionId`/`usageSourcePath`/`status`——两区四态同源，**payload 空串归一 `|| undefined`** 防 claude hook 输入缺字段时下游静默失效）+ **运行中会话标题通道（人工验证问题 3）**：SessionStart 同步兜底 `profile.tabTitle` 后经 `readHistoryTitle(cliId, sessionId)` 异步读历史同源标题覆盖（回退链 custom-title > ai-title > summary > firstPrompt；null → 兜底 tabTitle）；非 SessionStart 事件 5s 节流（`TITLE_FETCH_THROTTLE_MS`）重查（/rename custom-title、ai-title 运行中变化）；**陈旧守卫**（应用前校验注册表 `agentSession.sessionId` 与捕获值一致）+ **标题去重**（`lastAppliedTitleRef`——同值不重复回调）+ 失败静默（未知 cliId 无 provider 不炸），对外接口兼容 TerminalPanel |
| `terminal/keyboard.ts` | 终端快捷键命令工厂：`createTerminalShortcuts()`（无参）经 `commandFromMeta` 生成 `terminal.copy/paste/newline`，App 一次性注册；handler 经 `getActiveTerminal()` 派发到聚焦终端。Ctrl+C 不注册（透传 SIGINT） |
| `terminal/activeTerminal.ts` | 模块级"聚焦终端"指针：`setActiveTerminal`/`clearActiveTerminal`（仅匹配时清）/`getActiveTerminal`。终端聚焦时设为 active，命令 handler 据此派发 |
| `terminal/theme.ts` | xterm.js 主题 adapter（既定例外收敛表述）：**不再是独立主题定义**——`theme: { ...schemeRegistry.getActive().terminal }` 将 active 方案 terminal 段 25 键展开进 xterm `ITheme`（linear 方案值，硬约束 #6）；非色选项原位保留：`drawBoldTextInBrightColors` 显式声明为 `true`（消除对 xterm.js 默认值的隐式依赖，仅影响 ANSI 16 色粗体→亮色映射，不影响 True Color）、`vtExtensions: { kittyKeyboard: true }`（Kitty 键盘协议被动支持）、scrollback 等 |
| `terminal/TerminalRegistry.ts` | 模块级 `Map<panelId, RegisteredTerminal>` + `AgentSessionInfo`（含可选 `cliId`，存在即运行中，二态模型）+ `setAgentSession(panelId, patch|null)`（merge 语义：null 清空、undefined 键不覆盖、缺 lastEventAt 自动填 Date.now()）+ `subscribe(listener)` 订阅 register/remove/**sessionChange** 事件（sessionChange 仅携 panelId，listener 经 `get()` 读现值防快照不一致）；register 幂等覆盖时 `agentSession` 缺省保留旧值（StrictMode/重试场景不丢 session）。跨页面切换时供查询/reattach |
| `editor/index.ts` | EditorPanel 导出 |
| `editor/EditorPanel.tsx` | 编辑器面板 React 组件：container `overflow: clip`（裁剪不吸收滚动事件，委托 `.cm-scroller` 管理滚动；`.cm-editor` `height: 100%` 约束 scroller 高度产生溢出）→ useCodeMirror |
| `editor/keyboard.ts` | 编辑器快捷键命令工厂：`createEditorShortcuts()`（无参）经 `commandFromMeta` 生成 `editor.save`、`editor.toggleWordWrap`，App 一次性注册；handler 经 `getActiveEditor()` 派发 |
| `editor/activeEditor.ts` | 模块级"聚焦编辑器"指针：`setActiveEditor`/`clearActiveEditor`（仅匹配时清）/`getActiveEditor` |
| `editor/useCodeMirror.ts` | CodeMirror 6 生命周期 hook：创建 EditorView、`.cm-editor` `height: 100%` theme（约束 scroller 高度产生溢出→滚动条）、语言扩展、字体大小动态调节、自动换行 Compartment 热切换（Alt+Z）、Ctrl+Wheel 监听、Ctrl+S 保存（`usePanelFocus("editor")` + `setActiveEditor`，无路径则另存为）、Tab 缩进/Shift+Tab 反缩进（`keymap.of([indentWithTab])`）、Ctrl+F 搜索/撤销/重做仍归 CM keymap、外部文件改动监听、脏状态跟踪 |
| `editor/gitGutter.ts` | CodeMirror 6 gutter 扩展：DiffHunk → RangeSet<GutterMarker> 映射、setDiffMarkers StateEffect、diffMarkersField StateField、SpacerMarker 固定宽度防光标错位；新增 HEAD 侧 buildHeadRangeSet（old 行号映射）/ headDiffGutter / updateHeadDiffGutter / clearHeadDiffGutter |
| `html/index.ts` | HtmlPanel 导出 |
| `html/HtmlPanel.tsx` | HTML 预览面板：fs.readFile → injectScript 注入脚本（键盘转发 postMessage + 片段链接 click拦截 + scrollIntoView）→ iframe srcDoc 渲染（sandbox="allow-scripts"，不含 allow-same-origin），三态（loading/loaded/error），cancelled 防竞态；postMessage 接收键盘事件 → ShortcutRegistry 分发 |
| `gitshow/index.ts` | GitShowPanel 导出 |
| `gitshow/GitShowPanel.tsx` | HEAD 文件只读查看面板：gitFileAtHead 取内容 → CM6 只读（readOnly+editable）+ editorTheme/editorColorOverrides（经 `../../theme`）+ 语言扩展 + 字体主题；三态（loading/content/error）；大文件阈值复用；任意错误 → "该文件在 HEAD 中不存在" |
| `diff/index.ts` | DiffPanel 导出 + DiffPanelParams 类型 |
| `diff/DiffPanel.tsx` | Git 双栏 diff 面板：横向均分两栏（flex 50/50 + minWidth:0 防内容撑开）、容器 ref 桥接（renderKey + bridgedRef 支持条件渲染）、占位对齐（Decoration.widget）、垂直滚动同步（syncingRef）、双侧 gutter（headDiffGutter + diffGutter）、右侧 Ctrl+S 保存刷新链、左侧 .git 变更刷新 HEAD、右侧外部修改检测（净重载/脏弹窗） |
| `diff/alignment.ts` | `computeAlignment(hunks)` 纯函数：DiffHunk[] → `{ left: Map<afterLine, count>, right: Map<afterLine, count> }`。规则：纯新增左侧插占位、纯删除右侧插占位、modified 行数不等少侧插差值。零 DOM 访问 |
| `hooksConfig/index.ts` | HooksConfigPanel 导出 |
| `hooksConfig/HooksConfigPanel.tsx` | **hub 容器**（Stage 06，MC-502~507）：顶部 CLI 选择行（`hasConfigEditor` 过滤 + iconSrc logo + displayName + 选中态高亮 token）+ 编辑器槽（**KZ-1 分派**：经选中 CLI 的 `capabilities.hooks.configEditor` 渲染，缺失 → 空态占位「该 CLI 未提供配置编辑器」）；选中态 `params.selectedCli` 随布局 JSON 持久化（`updateParameters` + 显式 `onLayoutChange`，挂载恢复/失效回退首个有能力 CLI）；切换 = 卸载重挂载，dirty `confirmDialog` 守卫（askGuard 防循环，OV-02）；空态「无可配置 CLI」；单 CLI 也渲染选择行 |
| `hooksConfig/ClaudeHooksConfigEditor.tsx` | **claude 专属编辑器**（MC-504/508）：原 HooksConfigPanel 全部内容整体下移一层——层级切换器（**数据源 = `profile.capabilities.hooks.configLayers`（KZ-4：claude 三层值声明于 claude profile，模块级 LAYERS 常量已退役）；优先级标注 + rootPath 空时 project/local 禁用是 claude 语义保留内部**）+ 模式切换（GUI/JSON，非法 JSON 禁 GUI）+ F2 注入状态条与注入/卸载按钮（cliId = hub 选中态，MC-221）+ 保存按钮（dirty 且合法才可点）+ 重启提示条（`profile.hooks.restartHint` 驱动）；三态（loading/content/损坏 error）。**KZ-1**：整文件仅经 claude profile 的 `configEditor` 字段引用（hub 零直接引用），被 `features/cliProfiles/profiles/claude/index.ts` 挂载 |
| `hooksConfig/useHooksConfig.ts` | 数据 hook：cliId 实参 = hub 选中态 profile.id（MC-220，ipc 实参唯一来源）、**初始层 = initialLayer 参数（编辑器传 `configLayers[0].id`，KZ-4；缺省回退 "user"）**、rootPath 推导（照 useCommitStatus）、`readHooksConfig(cliId, layer, ...)` 加载（null 视为 {}，Err 置损坏态）、双模式同步（configJson/guiModel/dirty）、保存（双校验 + writeHooksConfig(cliId)）、切层/visibilitychange 轻量重读（dirty `confirmDialog` 守卫 + askGuard 防循环 + generation 取消） |
| `hooksConfig/configModel.ts` | 配置模型双向转换纯函数：`jsonToGui`/`guiToJson`（round-trip 不丢数据，未知字段归 extraFields）、`isSltermManaged`（注入段识别，C9） |
| `hooksConfig/eventsCatalog.ts` | 事件元数据单点（P3-FE-26）：30 事件 × 10 组全表 + handler 支持档（A/B/C）+ 5 种 handler 字段矩阵（C13-3 官方版）+ matcher 窄字符集受限事件（FileChanged/StopFailure）+ 纯查询函数（getEventMeta/isMatcherSupported/getSupportedHandlerTypes 等） |
| `hooksConfig/matcherEngine.ts` | matcher 语义引擎（C13-5）：`matchHook` 纯函数（exact-or / regex / all + 受限窄字符集），供 MatcherTester 试测与保存校验共用 |
| `hooksConfig/JsonMode.tsx` | JSON 模式编辑器（P3-FE-11）：CM6 + `codemirror-json-schema`（jsonSchemaHover/jsonSchemaLinter，hooks 子 schema；**无自动补全**——验收后决策删除）+ jsonParseLinter + `EditorView.theme` height:100% + 容器 `overflow:clip`（竖向滚动条委托 `.cm-scroller`，照编辑器滚动委托决策）+ 事件导航侧栏（点击跳转选区）+ MatcherTester 内联试测；校验经 onValidationChange 上报 |
| `hooksConfig/GuiMode.tsx` | GUI 表单模式（P3-FE-12，Master-Detail）：EventTree + 详情区（HandlerForm + matcher 输入）；选中态派生守卫（事件删除/重载回退空态）；注入段禁删（handler/含托管 handler 的 matcher 组/事件三层删除按钮禁用） |
| `hooksConfig/EventTree.tsx` | 事件树（P3-FE-13）：三级树（分组折叠 → 事件 → matcher 组 → handler 摘要）+ hook 总数 + 仅渲染配置中已存在事件（未知事件归「未知事件」组）+ 「slTerminal 托管」标记 |
| `hooksConfig/HandlerForm.tsx` | handler 编辑表单（P3-FE-14）：5 种 type 专用字段（字段矩阵来自 eventsCatalog 单一真值源）+ 通用字段（if/timeout/statusMessage，once 不展示）+ type 切换保留通用字段清除不适用字段 + 注入段只读/禁删 |
| `hooksConfig/MatcherTester.tsx` | matcher 实时试测工具（P3-FE-11）：输入 matcher + toolName + 事件 → `matchHook` 命中结果与匹配模式（exact-or/regex/all），事件感知窄字符集 |

## 硬约束

- **#5 面板封闭**：新增面板类型流程为 `panels/<newtype>/` 创建目录 → 实现面板组件 → 在 `panelRegistry.ts` 注册 → `PANEL_TYPES` 追加类型名
- **#6 配色单点**：所有颜色从 `theme/colors.ts` token 引用（如 `GIT_GUTTER_COLORS`），禁止硬编码色值。终端配色经 `panels/terminal/theme.ts` adapter 映射 active 方案 terminal 段
- **前端不碰 OS**：面板组件和 hooks 中所有系统调用（PTY、文件读写、剪贴板、git diff）必须经 `src/ipc/` 层调用，禁止直接 `invoke`
- **IPC 边界**：终端子 hook（`useTerminalInstance`/`usePtyOutput`/`usePtyResize`）通过 `ipc/pty` 调 spawn/write/resize/kill——`write`/`resize`/`kill` 签名含 `panelId`（后端 SEC-08 归属校验），调用点在 `useXterm.ts`（`writeToPty`/`onData`/cleanup kill/字号 resize）和 `usePtyResize.ts`（行/列变化），panelId 均为作用域内现成值；`editor/useCodeMirror.ts` 通过 `ipc/fs` 读写文件、`ipc/git` 获取 diff、`ipc/dialog` 弹另存为；`terminal/keyboard.ts` 命令 handler 通过 `ipc/clipboard` 读写剪贴板；`usePanelFocus` hook 管理焦点上下文与聚焦实例跟踪（命令在 App 一次性注册）。`terminal/TerminalPanel.tsx` 通过 `onTabStateChange` 回调桥接 useXterm → Dockview API（`api.setTitle` / `api.updateParameters`），不引入新 IPC 命令

## 测试模式

> 本节用例数为快照，最新计数以 `.claude/test-inventory.md` 为准。

测试文件位于 `src/__tests__/`，命名规则：`terminal*.test.ts(x)`、`editor*.test.ts(x)`、`html-panel.test.tsx`、`keyboard.test.ts`。L3 测试位于 `test/terminal/`。

### 技术栈

- Vitest（jsdom 环境）+ React Testing Library（`renderHook` / `render`）
- `@tauri-apps/api/mocks` 的 `mockIPC` 拦截 Tauri IPC
- 不使用 Playwright / Cypress / 真实浏览器

### useXterm 测试模式

> 用例数见 `.claude/test-inventory.md`（终端面板类目，`use-xterm-output.test.ts`（35 用例）+ `use-xterm-lifecycle.test.ts`（80 用例）+ `use-xterm-integration.test.ts`（12 用例））。

useXterm 是编排层——mock 6 个子 hook 才能隔离测试（`useFontSizeBridge` 已删除，字体缩放委托 `src/lib/useFontSizeWheel`）：

| Mock 模块 | 原因 | 关键验证 |
|-----------|------|---------|
| `../useTerminalInstance` | Terminal 实例生命周期 | 捕获 `Terminal` 构造实例到 `capturedTerminal`，供测试验证 `write`/`open`/`dispose` 调用 |
| `../usePtyOutput` | PTY 输出合帧管道 | 模拟 `handlePtyOutput`/`flushBuffer`/`cancelPendingFlush`，注入 PTY 数据验证合帧分流 |
| `../usePtyResize` | ResizeObserver + debounce | `proposeDimensions` 返回模拟 cols/rows，验证 X/Y 分离 debounce |
| `../useClipboardHandler` | OSC 52 剪贴板 | 注册 OSC 52 handler，验证 base64 解码、CJK 支持、焦点门控、>1MB 拒绝 |
| `../useCommandDetection` | OSC 133 命令边界 | 注册 OSC 133 handler，验证 C/D 序列解析 + onTabStateChange 调用 |
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

| 文件 | 模式 |
|------|------|
| `terminal-lifecycle.test.ts`（4 用例） | 挂载→创建→卸载→dispose 完整链路；mock `pty.spawn` 验证调用参数 |
| `terminal-instance.test.ts`（7 用例） | `useTerminalInstance` 生命周期分支（TRM-07）：fit 抛异常吞掉/`fontSize` undefined 跳过/prevFontSize 相同跳过重复写入/tryLoadWebgl 幂等（含 term 为 null 短路） |
| `terminal-strictmode.test.ts`（2 用例） | `<React.StrictMode>` 包裹验证 `smGuardRef` 防双重挂载：Terminal 实例数=1、PTY spawn 仅一次、dispose 仅在最终卸载时调 |
| `terminal.test.tsx`（19 用例） | TerminalPanel 组件：mock `useXterm` 返回 stub，验证 loading 遮罩/Windows build/spawn/customTitle 挂载恢复 + onDidParametersChange 同步（F8）/**页签 logo 会话绑定（F9 行为修订，真实 TerminalRegistry + registerStub 驱动 sessionChange）**：C 命中写 tabLogo/D 清空/hook 路径按 agentSession.cliId 查 + CLAUDE_CLI_ID 兜底/未注册 cliId null/挂载清残留+恢复/register 事件同步 |
| `can-fit.test.ts`（15 用例） | 纯函数边界测试：五条件守卫（null/undefined/0/isDisposed/no element） |
| `detect-webgl.test.ts`（3 用例） | `vi.spyOn(HTMLCanvasElement.prototype, 'getContext')` 模拟三种分支 |
| `webgl-setup.test.ts`（7 用例） | `setupWebglWithRetry` 指数退避（TRM-06）：不可用即回退/成功加载/context loss 重建（1000/2000ms 序列）/重试耗尽回退/cancel 清理/loadAddon 异常退避 |

### 页签标题/图标测试

| 文件 | 模式 |
|------|------|
| （Stage 01 退役迁移） | 原 `tab-title-registry.test.ts`（13 用例）/ `tab-rules.test.ts`（6 用例）语义已并入 `src/__tests__/cli-profile-registry.test.ts` + `cli-profile-claude.test.ts`（注册表/侧效应注册/logo 资源守卫，见 @../features/cliProfiles/CLAUDE.md） |
| `workspace-defaulttab.test.tsx`（34 用例，WRK-05） | 渲染**生产 `DefaultTab`**（非手写 Mock，经 `PageDockviewHost.tsx` 导出）：**`tabStatus` 状态圆点（IC-03：StatusDot 按状态渲染，null 不渲染）**、**FileIcon 文件型页签（TAB-03：filePath 存在即渲染）**、`onDidParametersChange` 事件结构回归（回调直接接收扁平 `Parameters`，`event.tabStatus` 而非 `event.params.tabStatus`——漂移即失败）、`onDidTitleChange` 标题更新、**关闭按钮 hover 显隐（TAB-02）**、**激活指示条（TAB-01：isActive && isGroupActive 底部 2px FOCUS_BORDER）**、**tabLogo 跟随页签名渲染（F9 行为修订：tabStatus null 仍渲染/仅 tabLogo 动态出现）** |

### 键盘与快捷键测试

| 文件 | 模式 |
|------|------|
| `keyboard.test.ts`（12 用例） | `createTerminalShortcuts()`（无参）handler 经 `getActiveTerminal()` 派发：设 active stub 后调 handler 验证 copy/paste/newline；无 active 返回 false 透传；Ctrl+C 不注册。jsdom 局限标注（真实按键投递由 L4 E2E 验收，E2E-01） |
| `editor-keyboard.test.ts`（7 用例） | `createEditorShortcuts()` save 经 `getActiveEditor()` 派发；后设置的 active 覆盖先前的；无 active 返回 false |
| `active-terminal.test.ts`（4 用例）/ `active-editor.test.ts`（5 用例） | active 指针 set/get/覆盖、clear 仅匹配时生效（防竞态） |
| `use-panel-focus.test.ts`（5 用例） | focusin→pushContext+onActivate、focusout(离子树)→popContext+onDeactivate、内部焦点转移不触发、卸载清理 |
| `shortcuts.test.ts`（54 用例） | `_reset()` 隔离；指纹 O(1) 匹配；上下文栈竞态；setOverrides 重绑/解绑/降级/冲突、resolve/forceContext、export/list |
| `global-commands.test.ts`（13 用例） | `createGlobalShortcuts(getApi)` 延迟求值 DockviewApi |

### 编辑器测试

| 文件 | 模式 |
|------|------|
| `use-code-mirror.test.ts`（39 用例） | `EditorState.create` 验证字体扩展；Compartment reconfigure 不重复 dispatch；handleSave（有/无 filePath、另存为、gitDiff 刷新、失败 alert、slterm:file-saved/file-saved-as 事件） |
| `editor.test.tsx`（9 用例） | EditorPanel 组件：mock `useCodeMirror` 返回 stub，验证 panelId/filePath 传递 + 容器 `overflow: clip` 样式 |
| `editor-confirm.test.ts`（11 用例） | `renderHook(useCodeMirror)` 真实驱动；mock `onFsEvent` 保留回调引用手动触发 fs-event；覆盖订阅/取消、kind 过滤、路径匹配、脏/净状态分支 |
| `editor-font.test.ts`（8 用例） | 字体 CSS 选择器断言（`.cm-scroller` vs `.cm-editor`） |
| `git-gutter.test.ts`（32 用例） | StateEffect → RangeSet 映射验证；GutterMarker DOM 颜色断言；SpacerMarker 宽度一致性 |
| `language-mapping.test.ts`（23 用例） | 扩展名→语言扩展全表验证（`.js`/`.ts`/`.py`/`.rs`/`.json` 等） |

### HTML 面板测试

`html-panel.test.tsx`：
- 三态渲染：loading（转圈）→ loaded（iframe srcDoc）→ error（错误信息）
- 竞态取消：快速切换 filePath 时旧请求 pending resolve 不覆盖新内容
- sandbox 属性验证 + renderer="always" 生命周期
- postMessage origin 校验 + 信任标记注入

> 内联脚本/事件的 CSP 放行（见「HTML 面板内联脚本/事件执行」决策）不由 jsdom 验证（jsdom 不强制 CSP）：配置不变量由 L2 `src/__tests__/csp-config.test.ts` 守卫，真实执行由 L4 E2E（真实 WebView2 强制 CSP）验证。

### gitshow 面板测试

`gitshow-panel.test.tsx`（21 用例）：
- mock `../ipc/git` `gitFileAtHead`、`@codemirror/search`、`useFontSizeWheel`、`usePanelFocus`、`activeEditor`
- 三态渲染：loading → content（CM6 只读编辑器）→ error 占位文案"该文件在 HEAD 中不存在"
- `EditorState.readOnly` 断言只读；`editable.of(false)` 断言不存在（编辑器可聚焦）
- `@codemirror/search` 扩展注册验证（`mockSearchFn` 调用参数）
- `useFontSizeWheel` 参数验证（container/min/max/setter）
- `usePanelFocus("editor")` 注册验证
- `createEditorFontExtension` 调用验证（默认字号 14）
- `oldPath` 优先于 `filePath` 调用 gitFileAtHead

### diff 面板测试

`diff-panel.test.tsx`（40 用例）：
- mock `gitFileAtHead` + `fs.readFile` + `gitDiff` + `onFsEvent` + `useFontSizeWheel` + `usePanelFocus`
- 双栏渲染验证（`data-e2e="diff-left"` / `diff-right`）
- 加载态 + 错误占位文案
- 保存后刷新链（`fs.writeFile` → `gitDiff` 重调 → gutter 更新）
- 左栏 `contentEditable` 不为 `"false"`（编辑器可聚焦）
- 左栏 `Ctrl+F` 触发 `.cm-panel.cm-search` 出现
- `useFontSizeWheel` 左右各调用一次
- `usePanelFocus` 左右栏各注册一次
- **FE-02 浮层回归**：脏弹窗改 mock `confirmDialog`（断言 title/confirmText/message 参数）+ 保存失败 `toast.show("error")`

`diff-alignment.test.ts`（18 用例）：
- `computeAlignment` 纯函数全分支覆盖：纯新增/纯删除/等行修改/多删少/多增少/多 hunk 合并/空 hunks
- 占位位置正确性：afterLine 索引 + 行数累积

### hooks 配置面板测试

测试文件位于 `src/__tests__/`，命名规则 `hooks-config-*.test.ts(x)` + `ipc-hooks-config-contract.test.ts`（11 文件，用例数见 `.claude/test-inventory.md`）：

- **纯函数层**：`hooks-config-catalog`（eventsCatalog 事件元数据全表）、`hooks-config-matcher`（matchHook 全分支 + 受限窄字符集）、`hooks-config-model`（jsonToGui/guiToJson 双向转换 + isSltermManaged）、`hooks-config-schema`（HKC-08 新建——直测 `validateHooksJson` 边界：合法/缺 hooks 键/非法 matcher/未知事件告警）
- **组件层**：`hooks-config-panel`（三态/层级切换器禁用逻辑/保存按钮/visibilitychange 重读）、`hooks-config-jsonmode`（CM6 + schema 扩展注册/校验上报/事件导航/MatcherTester）、`hooks-config-gui`（Master-Detail 渲染/增删事件与 handler/选中守卫）、`hooks-config-handlerform`（5 种 type 字段矩阵断言/type 切换/注入段禁改）、`open-hooks-config-panel`（入口命令同页单例，原 `hooks-config-entry` 更名）、`hooks-config-sync`（双模式同步）
- **IPC 层**：`ipc-hooks-config-contract`（readHooksConfig/writeHooksConfig 四维验证，经共享工厂 `helpers/ipc-contract.ts` 声明式驱动，IHE-06）

### L3 终端 headless 测试

L3 测试位于 `test/terminal/`，使用 `@xterm/headless`（零 DOM 依赖）+ `@xterm/addon-serialize`：

- 运行环境：`vitest.l3.config.ts`（`environment: 'node'`，非 jsdom），仅包含 `test/terminal/**/*.test.ts`
- `terminal-serialize.test.ts`（41 用例）：基本文本序列化、多行输出、ANSI 颜色保留、大块数据、光标定位、scrollback、resize reflow、SGR 属性叠加、多语言字符、交替屏幕、所有 ED/EL/IL/DL 擦除操作、DECSC/DECRC。E2E-07 后断言精确化——CUP/reflow/SGR 用例按 `getLine(y).translateToString()` 行断言 + `getCell(x,y).getFgColorMode()` 单元格属性断言
- `ansi-correctness.test.ts`（30 用例）：ANSI 颜色正确性——16 色前景/背景、256 色（标准/216 色立方/灰度）、TrueColor 24-bit 前景/背景/混合、SGR 属性（粗体/斜体/下划线/双下划线/慢闪/反显/隐藏/删除线/弱化/上划线）、SGR 组合叠加、SGR 重置/子参数重置、DEC 私有模式（DECTCEM/DECOM/DECAWM）、DECSC/DECRC、RIS、DECSTBM。E2E-08 补 256 色 palette 0-15 优化断言（SerializeAddon 将 0-15 优化为基本 SGR：`30+(c&7)`/`90+(c&7)`，断言 `\x1b[30m`/`\x1b[97m` 等优化后序列）
- `osc.test.ts`（9 用例）：OSC 序列——标题（OSC 0/2 BEL/ST）、调色板（OSC 4 单索引/多索引）、嵌入完整性（OSC 在正常输出中/穿插文本/后紧跟文本不丢失）
- `keyboard.test.ts`（36 用例）：Ctrl 组合键（A-Z 全表 + Backslash/Slash）、Alt+字母/Enter、功能键 F1-F12、Home/End/PgUp/PgDn/Insert/Delete、方向键 + Ctrl+方向键、CSI u 协议（基本键/Shift/Ctrl/Ctrl+Shift/Alt 修饰）、退格/回车/制表符渲染
- `theme-options.test.ts`（5 用例，E2E-02）：生产 `terminalOptions`（`src/panels/terminal/theme.ts`）——16 色与主题色板一致、`CSI>1u` 可激活 Kitty、scrollback 容量生效、`drawBoldTextInBrightColors` 亮色映射。**定位声明**：L3 = 网格状态正确性，Kitty 编码/亮色渲染依赖 DOM/渲染器层，由 L4 真实 WebView2 视觉回归验收
- `production-osc.test.ts`（8 用例，E2E-03）：生产 OSC 52/133/8 handler 语义——按生产实现原样复刻注册代码（逐段标注来源文件行号，防生产与测试漂移）：OSC 52 → mock `ipc/clipboard.writeText` + CJK 解码、OSC 133 → `onTabStateChange` 参数、OSC 8 → mock `ipc/shell.openUrl`。hook 包装层（React 侧）由 L2 use-xterm 测试覆盖
- `negative-ansi.test.ts`（9 用例，E2E-14）：反向/异常 ANSI 输入——非法 ANSI、截断多字节序列、嵌套/未终止 OSC、异常 resize、非法 SGR 参数、超长 CSI 参数——headless 不崩溃且网格状态可恢复
- `writeSync(term, data)`：`term.write(data, callback)` 的 Promise 化辅助，必须等 callback 完成后才能 serialize

> L3 各文件用例数见 `.claude/test-inventory.md`。

## 添加新面板类型的步骤

1. 在 `src/panels/` 下创建 `newtype/` 目录，含 `index.ts`、`NewTypePanel.tsx` 和必要的 hooks
2. 在 `src/panels/index.ts` 添加 `export { NewTypePanel } from "./newtype";`
3. 在 `src/panelRegistry.ts` 的 `panelRegistry` 对象中注册组件映射
4. 在 `PANEL_TYPES` 数组中追加 `"newtype"`
5. 如涉及新 IPC 命令，在 `src-tauri/capabilities/` 显式放行
