# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels` 是 Dockview 面板的实现层，承担所有可托管到工作区布局的渲染容器（terminal/editor/html/gitshow/diff/settings）。Dockview 只负责布局骨架，面板内部状态、渲染生命周期与平台 API 调用全收敛在本层，使前端其他区域无需关心终端实例、CodeMirror 编辑器、iframe 沙箱等复杂生命周期。

## 关键约束与决策

### 终端：每次挂载新建 Terminal 实例

xterm.js 不支持 `term.open()` 二次调用（GitHub Issue #4978）。因此每次面板 mount 都创建新 Terminal 实例，卸载时 dispose。跨页面终端存活由 `workspace/` 层通过共享宿主 + 页组模型解决（页组容器显隐，面板不随切页卸载重建——见 `workspace/CLAUDE.md`「共享宿主 + 页组模型」节），本层不处理实例复用。

### WebGL 优先 + DOM 兜底

`detectWebgl()` 预检 WebGL2 可用性：可用则加载 WebglAddon，不可用则 DOM renderer 兜底。焦点终端持有 WebGL context，`onContextLoss` 触发后释放 addon 资源。

- **检测不带 `failIfMajorPerformanceCaveat`（FE-26）**：该标志在 Chromium GPU blocklist 场景会连同软件渲染一并拒绝 → DOM renderer 回退 → 快滚整屏重绘掉帧（Win10 + UHD 630 实机症状）。SwiftShader 软件渲染仍远快于 DOM renderer 全帧重建，故不加回该标志。
- **SwiftShader 识别 + 一次性降级提示（CP-018）**：`tryLoad` 成功路径经 `isSwiftShaderRenderer()` 判软件渲染（经 `WEBGL_debug_renderer_info` 扩展读 `UNMASKED_RENDERER_WEBGL`，模块级缓存一次检测全生命周期复用）——落入 SwiftShader 时 `toast.show("warning")` 降级提示一次（`swiftShaderNotified` 旗标全生命周期防重；扩展缺失/读取出错保守返回 false 不提示）。`detectWebgl` 检测契约不变（FE-26 注释保留）。
- **加载时序约束（FE-34）**：`setupWebglWithRetry` 必须在 `term.open(container)` 之后调用。WebGL 渲染器需要挂载后的 canvas；先加载会绑定空 canvas → 静默黑渲染且不触发 context loss 兜底（win10 终端纯黑屏根因）。

### PTY spawn 等待布局就绪（CP-019 事件驱动）

`useXterm` 挂载后不立即 spawn PTY：ResizeObserver 首帧回调确认容器尺寸就绪（`offsetWidth/offsetHeight > 0`，CP-019 事件驱动）→ fit + proposeDimensions 取真实字符尺寸 → `pty.spawn(真实 cols×rows)`。500ms 超时仅作防御底线（回退 80×24）——正常路径不再有时序猜测轮询；`spawned` 守卫保证 RO 回调与超时两者只 spawn 一次，卸载清理断开 observer 并清除超时。

### windowsPty buildNumber 钳制（ADR-0004）

`term.options.windowsPty = { backend: "conpty", buildNumber: clampWindowsBuildForXterm(真实值) }`——钳制至 xterm.js ConPTY 兼容阈值下界 `XTERM_CONPTY_MIN_BUILD = 21376`。低于该值 xterm 启用 wrapping 启发式，claude 全屏高频重绘下误判致 buffer 错乱（Win10 四症状）。钳制使 Win10 与 Win11 行为对齐，连带启用 resize reflow。**真实 build 号获取链不动**，钳制收口 useXterm 两处 windowsPty 写入点；spawn 请求不带 buildNumber，后端不受影响。xterm.js 升级时须重评估此钳制点。

### 预览面板家族（docViewer 共享层）

htmlviewer / markdownviewer 等「文档型预览面板」共享 `src/panels/docViewer`——预览窗口编排（PreviewFrame）、注入脚本组装、消息桥、缩放/滚动运行时、形态切换条（ModeSwitcher）全部单点收容于此。**S10-② 起（ADR-0019）预览渲染于独立 Tauri webview（label `preview-<panelId>`，自定义协议宿主页域）**：面板根 = 工具条带（切换条/HUD 悬浮带，40px）+ 内容区（预览窗口锚定其下矩形）；消息桥 = Tauri event/IPC（旧 iframe postMessage 通道退役，CP-044）；键盘焦点恒在主窗口（预览窗口 focusable=false——全局快捷键在预览态仍可用，CP-013）。安全红线（SEC-03/04 校验链、sandbox 无 allow-same-origin、注入拼接纪律）与消息协议已随迁 docViewer/CLAUDE.md、previewMessages.ts 与 src/ipc/preview.ts 单点登记，改本家族行为前必读。信任模型（md/html 同态、global 命令集不扩）见 ADR-0017；本地资源通道（fs_read_resource + data: URL 内联 + 预览 webview CSP data: 放行——主窗口已回收 data:，CP-035）见 ADR-0018。

- **htmlviewer**（`panels/html/HtmlPanel`）：二态 render（默认）/ edit 源码（CM6 lang-html）；草稿快照往返 docRef。edit 形态字号 = 共享 editorFontSize store（Ctrl+滚轮，EditorPanel 同款接线——2026-09-06，原恒 14 语义变更，见 markdown/CLAUDE.md「编辑字号语义」）。
- **markdownviewer**（`panels/markdown/MarkdownPanel`）：三态 edit（默认）/ split（allotment 拖拽，比例持久化）/ preview；渲染管线/资源/链接分派见 markdown/CLAUDE.md。
- 文档真值源 = 面板 docRef（草稿优先磁盘）；**markdownviewer CM 恒挂载（CP-037）**：preview-only 改隐藏保活（display:none 照 edit↔split 先例，allotment CM pane 恒 index 0）——undo/光标跨 edit/split/preview 保留，代价 preview 常驻一个 CM 实例内存，已接受（htmlviewer 仍 edit 态挂载、render 卸载，不在此例）。
- **大文件引导口径（CP-022）**：本家族 edit 形态经 `initialDoc` 快照建缓冲——快照路径**跳过**磁盘大文件检查与 largeFile 信号（useCodeMirror 语义：快照已过检/回填源），不消费 >10MB 只读分片浏览（该引导仅适用 EditorPanel 磁盘直读流；gitshow/diff 引导形态见对应小节）。
- **宿主内联 `<script>` 真实执行（S10-② 起，CP-031 消亡判定达成）**：注入机制（injectScript + buildInjectedScript）原样迁入预览 CSP 域（自定义协议宿主页 iframe——域级 CSP meta：`default-src 'none'` + 内联 script/style + data: img/font，SEC-02）；`injectScript` 不再做字符串级转义（存量缺陷转义函数已删）——宿主脚本段原样进入渲染文档并真实可执行，内联事件属性（onload/onerror）与 `<script>` 段同为执行通道（html.e2e 缩放 fixture 与宿主 script 用例双通道实证）。渲染与主窗口 CSP 隔离——收紧主窗口 CSP 不再影响预览。
- markdownviewer 纳入 `renderer="always"` 白名单（iframe 与 CM 编辑实例切走切回不重建——决策 #17）。

### gitshow：只读但可聚焦

`GitShowPanel` 用 `EditorState.readOnly.of(true)` 阻止编辑，**不使用** `EditorView.editable.of(false)`。后者设 `contentEditable=false` 会导致编辑器不可聚焦，CM6 内部键绑定和 ShortcutRegistry 全部失效。

**>10MB 超限引导（CP-022）**：HEAD 内容超过 `MAX_FILE_SIZE_BYTES` 时改渲染 `LargeFileViewer`（`sourceLabel="git show"`，只读分片浏览替代 CM 拒绝文案）。**内容源近似口径**：查看器经磁盘 `filePath` 分片读取——与 HEAD blob 一致场景内容等价（未修改/普通场景）；工作区文件缺失（deleted 状态——本面板主要来源）或与 HEAD 差异大时，读块失败由查看器「部分内容读取失败」兜底提示或内容近似。1MB-10MB 警告 header 语义不变。

### diff：双栏占位对齐 + 滚动同步

`DiffPanel` 横向均分两栏：左 = HEAD 只读 + HEAD gutter + 占位行，右 = 工作区可编辑 + workdir gutter + 占位行。

**>10MB 超限引导（CP-022）**：任一侧内容超过 `MAX_FILE_SIZE_BYTES` → 该侧改渲染 `LargeFileViewer`（左 `sourceLabel="HEAD"`、右 `sourceLabel="工作区"`）；对齐/滚动同步/占位对齐装饰对该侧降级（view 缺失天然 no-op），另一侧 CM 行为不变；双侧超限分栏各自只读浏览。查看器内容源 = 磁盘 `filePath` 分片（HEAD 侧与工作区内容差异场景为近似口径，同 gitshow 登记）。1MB-10MB 警告 header 语义不变。

- **占位对齐**：`computeAlignment(hunks)` 纯函数根据 DiffHunk[] 计算左右两侧需插入占位行的位置与数量——纯新增行左侧插占位，纯删除行右侧插占位，modified 行数不等时少的一侧插差值。通过 CM6 `Decoration.widget` 渲染块级占位行。
- **垂直滚动同步**：一侧 `.cm-scroller` scroll → 另一侧 `scrollTop` 跟随（`syncingRef` 防循环）。水平滚动不同步。
- **CSS flexbox `min-width: auto` 修复**：DOM 层级为双层 flex 嵌套，CM6 `.cm-content`（`flex-shrink: 0`，`white-space: pre`）会随长行横向扩展撑开 flex 子项，导致分界线偏离 50%。四个 flex 子项均加 `minWidth: 0` 显式覆盖。`overflow: clip` 保留——裁剪溢出但不吸收滚轮事件。
- **容器 ref 桥接**：DiffPanel 三态中容器 div 仅在 `"ready"` 态挂载。`renderKey` state + `bridgedRef` guard + `useEffect([state.kind])` 在 commit 后触发额外渲染，确保 `useFontSizeWheel` / `usePanelFocus` 在容器就绪后收到非 null DOM 元素。

### settings：设置中心壳 + 配置页注册表分派（F11）

`SettingsPanel`（F11）为**壳容器**：左导航（组序 global→project，固定 180px）+ 右配置页槽位，槽位经 `SettingsPageRegistry`（features/settingsCenter）分派渲染 `page.component`（`key={selectedPage}` 强制重挂载——ADR-0001 先例，页内状态随卸载丢弃）。配置页注册集中在 `features/settingsCenter/pages.ts`（side-effect import 触发点：SettingsPanel 顶部 import 即注册全部配置页），壳零直接引用任何具体配置页组件，新增配置页 = pages.ts 追加一条 register。

- **壳是 params 持久化单点**：选中切换与 `onPageParamsChange`（pageParams[selectedPage] 槽 merge patch）统一经 `persistParams`（`api.updateParameters` + 显式 `onLayoutChange(saveLayout)` + 按 `settings-` 前缀解析 pageId → `updatePageLayout`）——updateParameters 不触发 onDidLayoutChange，必须显式保存（F8 先例）。
- **dirty 汇聚（SC-FE-07）**：页组件经 `SettingsPageProps.onDirtyChange` 上报 → 壳维护 dirtyMap（导航项 7px 中性色圆点，不用 F3 四态色防语义混淆）+ 同步 `dirtyRegistry`（与 DefaultTab × 关闭拦截共享同一真值源，防两处状态漂移）。切配置页时当前页 dirty → `confirmDialog` 确认丢弃（askGuard 500ms 防循环，照 hub 先例）；× 关闭拦截在 workspace 层（见 workspace/CLAUDE.md）。
- **切项目自动关闭（SC-FE-08）**：订阅 activePageId 所属项目 ≠ 面板所属项目 → 关闭。初始评估（布局恢复挂载即不一致，新挂载不可能 dirty）静默关；变化触发 dirty 守卫 confirmDialog，取消则不关（面板暂留非活跃项目，尊重用户选择）；`activePageId === null` 不动（删除末页/启动瞬态，防连锁误关）。
- **settings 已纳入 renderer="always"（SC-FE-06 翻案，CP-017）**：dirty 真值源（dirtyRegistry）脱离壳生命周期——壳不随页签切换卸载，dirtyMap/dirtyRegistry 条目跨切签存活（壳卸载不再 clear，条目收口到「确认丢弃关闭」动作点）。
- **corrupted 警示条**：挂载 `loadSettings()` → corrupted → 顶部警示条（× 可关，`data-e2e="settings-corrupted-banner"`，不阻塞）。L2 覆盖（loadSettings mock），L4 豁免登记——写坏文件需沙箱外写，无命令通道。
- claude 专属 hooks 编辑器归域 `features/cliProfiles/profiles/claude/configEditor/`（KZ-1，见 cliProfiles/CLAUDE.md），经 profile 的 `configEditor` 字段挂入；本面板经 HooksSettingsPage 页组件接入，不再跨 features 引用。

### Ctrl+C 保留为中断（CP-020）

`keyboard.ts` 的 `createTerminalShortcuts` 注册 `terminal.interrupt` 命令——handler 经 active 指针派发本地中断提示（页签 working→attention）后**必须返回 false 透传**，`\x03` 仍由 xterm.js 自然发送到 PTY，claude 用它取消操作（SIGINT 语义不变）。任何新增 terminal context 命令不得拦截 Ctrl+C 透传语义；`isReserved` 仍拦用户覆盖（保留键语义不变，代码默认键绑保留键为 CP-020 显式豁免）。

### 输出合帧策略（终端平台能力）

针对 Ink 系 TUI（如 Claude Code）约 60fps 全帧刷写的 ANSI 序列优化：

- **直写阈值 256 字节（FE-18）**：≤256 字节直写终端，>256 字节走合帧路径。
- **Idle+Max 双定时器**：空闲 2ms 无新数据则 flush；最多 16ms 强制 flush 一次（防饥饿）。
- **DEC 2026 同步更新**：flushBuffer 用 `\x1b[?2026h` / `\x1b[?2026l` 包裹，xterm.js 6.0+ 原生支持，所有 grid 变更在单帧内原子渲染。
- **非焦点终端降频**：`visible=false` 时仅累积不 flush（上限 64KB），切回时立即回放。
- **交替缓冲 resize**：`pty.resize()` 只发 SIGWINCH，不改变 xterm.js 网格尺寸。网格尺寸必须由客户端 `fitAddon.fit()` → `term.resize()` 更新；交替缓冲中也必须调 `fit()`，否则 Ink SIGWINCH 后新尺寸输出会渲染到旧网格造成永久撕裂。

### Resize X/Y 分离 debounce + NaN 防御

- **NaN guard**：`proposeDimensions()` 在 WebGL 渲染器未就绪时可能返回 `cols/rows=NaN`（xtermjs#4338），`Number.isFinite()` 守卫防止传入 `pty.resize()`。
- **X/Y 分离**：仅行数变化（高度拖拽）→ 立即 `fit()` + `pty.resize()`；列数变化（宽度拖拽，需 re-wrap）→ 100ms debounce。
- **resize 前丢弃缓冲**：`cancelPendingFlush()` 在 resize 前清除 timer 并丢弃缓冲，防止旧尺寸 PTY 数据在新视口中错位。

### OSC 52 剪贴板拦截

xterm.js 6.0+ 核心解析器内建 OSC 52 handler，但无 addon 时静默丢弃。`useXterm.ts` 在 `term.open()` 后注册自定义 handler：

- 仅写入（不响应读请求 `Pd=?`），仅系统剪贴板选择器 `c`。
- 焦点门控：`visibleRef.current === false` 时忽略，防止后台 Tab 静默改剪贴板。
- Payload 上限 1MB；CJK 正确解码（`atob` → `Uint8Array` → `TextDecoder.decode("utf-8")`）。
- 直接 import `src/ipc/clipboard` 的 `writeText`，与 `Ctrl+Shift+C` 共用同一写入路径。

### attachCustomKeyEventHandler 委托式 fallback

xterm.js 6.1.0-beta 后，ShortcutRegistry 窗口级 capture 路径在真实 WebView2 中可能因 `focusin` 未正确冒泡而使 terminal context 未激活。为双重保障，`term.open()` 后 `term.attachCustomKeyEventHandler()` 委托进 ShortcutRegistry：

```typescript
term.attachCustomKeyEventHandler((event) => {
  if (event.type !== "keydown") return true;
  const consumed = getShortcutRegistry().resolve(event, "terminal");
  if (consumed) { event.preventDefault(); return false; }
  return true;
});
```

`terminal.copy`/`terminal.paste`/`terminal.newline`/`terminal.interrupt`（CP-020）均为可重绑的注册命令，handler 经 `getActiveTerminal()` 派发到聚焦终端。window capture 命中即 `stopPropagation`，事件到不了 xterm；仅 capture 失效时委托层兜底。interrupt 是唯一返回 false 的注册命令——透传语义使 \x03 双路径各触发一次 handler，由 working 守卫幂等去重。

### Kitty 键盘协议被动启用

`theme.ts` 的 `terminalOptions` 设置 `vtExtensions: { kittyKeyboard: true }`，允许子进程通过 `CSI>1u` 激活差异化编码。协议为被动模式：终端声明能力后，应用需主动 push flags。若应用未激活，`KeyboardService.useKitty` 返回 `false`，回退传统 handler。

### OSC 8 超链接

xterm.js 6.0.0 原生支持 OSC 8 解析渲染。`useXterm.ts` 在 `term.open()` 后设置 `term.options.linkHandler.activate`，通过 `src/ipc/shell` 的 `openUrl()` 打开系统默认浏览器。`hover`/`leave` 回调一期不做。

### OSC 133 命令边界 + 页签标题/状态

`shell-integration.ps1` 的 Enter hook 在命令执行前发射 OSC 133 C，prompt 在命令退出后发射 OSC 133 D。`useXterm.ts` 注册 `term.parser.registerOscHandler(133, ...)`：

- **OSC 133 C**：提取命令行文本 → `onTabStateChange({ active: true, title, status: "attention" })`；同时 `cliProfileRegistry.matchByCommand(command)` 查 profile → 命中时覆盖 `title = profile.tabTitle`，并 `setAgentSession({ cliId: profile.id })`。
- **OSC 133 D**：命令退出 → `onTabStateChange({ active: false })`（restoreTitle 缺省 true）→ TerminalPanel 恢复原标题并单清状态；`setAgentSession(null)` → sessionChange 驱动清 logo。

**B12**：先写会话再发回调——TerminalPanel 的 originalTitleRef 捕获守卫检查 agentSession 非空即跳过，回调触发 onDidTitleChange 时会话必须已置位。

**B13 `restoreTitle` 信号**：`active=false` 时是否恢复原标题（缺省 true；false = 仅清状态圆点）。**真退出信号**（OSC 133 D / PTY EXIT）缺省恢复；SessionEnd/Exit hook 事件与 spawn 初始化重置传 `restoreTitle:false`（/resume 的 SessionEnd→SessionStart 序列中 claude 进程未退出，恢复会把标题误回退为 terminal-N；spawn 初始化恢复会抹掉 B12 重算结果）。

**F9 页签 logo 会话绑定**：页签 logo 不经 C 路径直传。TerminalPanel 订阅 `TerminalRegistry.subscribe`（register/sessionChange 事件过滤 panelId）→ 读 `get(panelId)?.agentSession` → session 非 null 时按 `cliId ?? CLAUDE_CLI_ID` 查 `profile.iconSrc` 写 `tabLogo`，null/undefined 清 `tabLogo`。

**B14 visible 前缀匹配**：`activePageId != null && panelId.startsWith(`terminal-${activePageId}-`)`。旧恢复格式含 Date.now 数字段，正则/切分解析会吞掉多余数字段得到错误 pageId → visible 恒 false → 非焦点降频永不 flush（历史恢复黑屏根因）。

**会话元数据单点（硬约束 #8）**：PTY 进程映射仅在 `panels/terminal/TerminalRegistry`（模块级 Map）管理，前端会话元数据已合并入 registry；面板只订阅，不自存。

**仅限于 pwsh/powershell**——shell integration 脚本仅在 PowerShell 注入，cmd.exe 无此能力。

### F3 页签四态指示

终端页签通过双源事件合成四态（渲染层 = `StatusDot` 圆点）：

| 状态 | 圆点色 | 触发源 | 说明 |
|------|--------|--------|------|
| `working` | 绿 | agent-event `PreToolUse`/`PostToolUse` | 工具调用中 |
| `attention` | 黄 | OSC 133 C 或 agent-event Notification | 命令运行中或需要关注 |
| `done` | 灰 | agent-event `Stop` | 主代理完成响应输出 |
| `error` | 红 | agent-event `PostToolUseFailure`/`StopFailure` | 工具调用失败或轮次因 API 错误结束 |

实现要点：

- `useCommandDetection`：OSC 133 C 触发时 `matchByCommand` 命中 → 先 `setAgentSession` 后 `onTabStateChange({ active: true, title: profile.tabTitle, status: "attention" })`（B12）。
- `useXterm`：新增 `onAgentEvent` 订阅 → 按 `panelId` 过滤 → 来源 CLI 经 `resolvePayloadCliId` 三级解析（ZQ-2，空串/空白 cliId 同等回退）→ `eventToStatus(event, notificationType?)`（经 `profile.hooks` 委托）→ `onTabStateChange({ active: true, status })`；`SessionEnd ∨ Exit` 双事件清状态（ZQ-6）调 `{ active: false, restoreTitle: false }`（B13）。
- `TerminalPanel.handleTabStateChange`：`active=true` 时只有 `title` 存在才 `setTitle`，只有 `status !== undefined` 才 `updateParameters({ tabStatus: status })`；`active=false` 时 `restoreTitle !== false` 才恢复原标题 + 单清 status。
- `params.tabIcon` emoji/img 分支已退役（IC-03），`TabState.logo`/`icon` 字段已退役。

### 中断场景已知行为（Ctrl+C）

Claude Code 在用户主动 Ctrl+C 中断时不发射任何 hook 事件。CP-020 起由前端本地中断命令（`terminal.interrupt`）在 `\x03` 透传前显式将 `working` 页签置 `attention`——幂等设计：window capture 与 xterm 委托双路径各触发一次 handler，第二次为 no-op（见 TerminalPanel.handleInterrupt）。60s 兜底语义保留：中断回提示符后长时间无操作 → `idle_prompt` Notification → 自动转 `attention`。

## 外部坑/红线

- **xterm.js `open()` 不可复用**：同实例二次 `open()` 抛异常，必须每次挂载新建实例。
- **WebGL 检测不带 `failIfMajorPerformanceCaveat`**（FE-26）：GPU blocklist 场景会误杀软件渲染，导致 DOM renderer 快滚掉帧。
- **WebGL 加载时序（FE-34）**：`setupWebglWithRetry` 必须在 `term.open(container)` 之后，否则 win10 纯黑屏。
- **proposeDimensions NaN**（xtermjs#4338）：WebGL 未就绪时可能返回 NaN，必须 `Number.isFinite()` 守卫后再传 `pty.resize()`。
- **iframe `allow-same-origin` 禁用**：与 `allow-scripts` 组合是已知危险组合（Tauri CVE-2024-35222），且会导致 Tauri 注入 App JS 劫持片段导航。
- **主窗口 CSP 禁 script-src 'unsafe-inline'、禁 dangerousDisableAssetCspModification（S10-② 起，CP-012）**：预览渲染于独立 webview（自有 CSP 域——自定义协议宿主页，内联脚本仅该域放行）；srcdoc iframe 通道已退役。收紧主窗口 CSP 不再影响预览；回潮放宽即破 CP-012 终态（csp-config.test.ts 锁死）。**img-src/font-src 的 data: 亦已回收（CP-035）**——主窗口内任何 data: 图像/字体技法（如 CSS background url(data:)）都会被 CSP 拦截（CM6 lint 波浪线曾为此改为 text-decoration 技法，见 theme/overrides.ts 注释）；data: 数据通道仅存预览域（域级 CSP meta 放行 img/font data:），主窗口零放行由 csp-config.test.ts「data: 不在主窗口任何指令」守卫锁死。
- **CM6 `readOnly` vs `editable`**：gitshow/diff 左栏只能用 `EditorState.readOnly`，不能用 `EditorView.editable`，否则编辑器不可聚焦、快捷键失效。
- **DiffPanel flexbox 撑开**：CM6 `.cm-content` 的 `flex-shrink: 0` + `white-space: pre` 会撑开双层 flex，必须所有 flex 子项设 `minWidth: 0`。
- **ConPTY 并发 spawn 死锁**：PTY spawn 由后端 `SPAWN_LOCK` 串行化（详见 ../src-tauri/src/pty/CLAUDE.md），前端不直接处理，但 L1 测试必须 `--test-threads=1`。
- **中文 IME 合成要尽早实测**：键盘/IME 改动后须尽早用真实 WebView2 环境验证中文输入合成，避免合成路径破坏积累。
- **PowerShell 是 OSC 133 唯一注入目标**：cmd.exe 无 shell integration，标题/状态/命令边界检测对 cmd 会话不可用。

## 测试模式

- **L3（node + `@xterm/headless`）**：用 `@xterm/headless` 验证网格状态，Kitty 编码/亮色渲染依赖 DOM/渲染器层由 L4 验收。
- **useXterm 是编排层**：mock 6 个子 hook 才能隔离测试（`useTerminalInstance` / `usePtyOutput` / `usePtyResize` / `useClipboardHandler` / `useCommandDetection` / `webgl`）。共享测试工厂见 `src/__tests__/helpers/xterm-test-utils.ts`。
- **L3 复用生产实现**：`oscHandlers.ts`（TQ-E-01）与 `keyEventHandler.ts`（TQ-E-02）抽为纯函数后，L3 `production-osc.test.ts` / `shortcut-dispatch.test.ts` 直接复用生产真值源，不再复刻。
- **HTML 面板 postMessage**：jsdom 不强制 CSP，L2 校验四层校验逻辑；真实 WebView2 行为由 L4 验收。
- **注入脚本缩放/滚动运行时（zoomRuntime/scrollRuntime）**：jsdom 不执行 srcdoc 内脚本——运行时生成参数化函数源码（`function(doc,win)`），L2 经 `new Function` 在桩 doc/win 上真实执行（行为级，doc-viewer-zoom-runtime.test.ts / doc-viewer-injection.test.ts）；物理滚轮设备与悬停语义由 L4 fixture 合成事件 + 手工验收（豁免登记见 `.claude/test-exemptions.md`）。
- **编辑器测试模式**见 `@editor/CLAUDE.md`。

## 添加新面板类型的步骤

1. 在 `src/panels/` 下创建 `newtype/` 目录，含 `index.ts`、`NewTypePanel.tsx` 和必要的 hooks。
2. 在 `src/panelRegistry.ts` 注册组件映射（**无 `src/panels/index.ts` barrel**——各面板经 `panelRegistry.ts` 顶部逐文件 import 直连）。
3. 在 `PANEL_TYPES` 数组中追加 `"newtype"`。
4. 如涉及新 IPC 命令，在 `src-tauri/capabilities/` 显式放行。
