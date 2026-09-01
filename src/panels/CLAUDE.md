# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels` 是 Dockview 面板的实现层，承担所有可托管到工作区布局的渲染容器（terminal/editor/html/gitshow/diff/settings）。Dockview 只负责布局骨架，面板内部状态、渲染生命周期与平台 API 调用全收敛在本层，使前端其他区域无需关心终端实例、CodeMirror 编辑器、iframe 沙箱等复杂生命周期。

## 关键约束与决策

### 终端：每次挂载新建 Terminal 实例

xterm.js 不支持 `term.open()` 二次调用（GitHub Issue #4978）。因此每次面板 mount 都创建新 Terminal 实例，卸载时 dispose。跨页面终端存活由 `workspace/` 层通过多 Dockview 实例 + CSS 显隐解决，本层不处理实例复用。

### WebGL 优先 + DOM 兜底

`detectWebgl()` 预检 WebGL2 可用性：可用则加载 WebglAddon，不可用则 DOM renderer 兜底。焦点终端持有 WebGL context，`onContextLoss` 触发后释放 addon 资源。

- **检测不带 `failIfMajorPerformanceCaveat`（FE-26）**：该标志在 Chromium GPU blocklist 场景会连同软件渲染一并拒绝 → DOM renderer 回退 → 快滚整屏重绘掉帧（Win10 + UHD 630 实机症状）。SwiftShader 软件渲染仍远快于 DOM renderer 全帧重建，故不加回该标志。
- **加载时序约束（FE-34）**：`setupWebglWithRetry` 必须在 `term.open(container)` 之后调用。WebGL 渲染器需要挂载后的 canvas；先加载会绑定空 canvas → 静默黑渲染且不触发 context loss 兜底（win10 终端纯黑屏根因）。

### PTY spawn 等待布局就绪

`useXterm` 挂载后不立即 spawn PTY，而是 rAF 轮询容器 `offsetWidth > 0`（最多 30 帧 / 500ms），然后 fit + proposeDimensions 获取真实字符尺寸，以真实 `cols × rows` 调用 `pty.spawn()`。超时回退 80×24。

### windowsPty buildNumber 钳制（ADR-0004）

`term.options.windowsPty = { backend: "conpty", buildNumber: clampWindowsBuildForXterm(真实值) }`——钳制至 xterm.js ConPTY 兼容阈值下界 `XTERM_CONPTY_MIN_BUILD = 21376`。低于该值 xterm 启用 wrapping 启发式，claude 全屏高频重绘下误判致 buffer 错乱（Win10 四症状）。钳制使 Win10 与 Win11 行为对齐，连带启用 resize reflow。**真实 build 号获取链不动**，钳制收口 useXterm 两处 windowsPty 写入点；spawn 请求不带 buildNumber，后端不受影响。xterm.js 升级时须重评估此钳制点。

### HTML 面板：iframe 键盘桥与片段拦截

HTML 内容通过 `<iframe sandbox="allow-scripts" srcDoc={...}>` 渲染（**不含 `allow-same-origin`**）。注入脚本实现三段功能：

1. **CSS 注入**：`.slterm-target` 作为 `:target` 伪类的 JS 替代。
2. **键盘转发**：`keydown` capture → `window.parent.postMessage({type:"slterm_key", ...}, "null")`。
3. **片段链接拦截**：`click` capture → 检测 `<a href="#...">` → `preventDefault` + `scrollIntoView` + class 切换模拟 `:target`。

WebView2 sandboxed iframe 中 `#fragment`/`:target` 彻底失效，且 `allow-scripts` + `allow-same-origin` 是已知危险组合（Tauri CVE-2024-35222），Tauri 会向同源 iframe 注入 App JS bundle 导致片段导航被 React Router 劫持。去掉 `allow-same-origin` 后 iframe 为 opaque origin，Tauri 不再注入。

### postMessage 校验（SEC-03/SEC-04）

`HtmlPanel.handleMessage` 对来自 iframe 的 `postMessage` 实施四层校验：

| 校验层 | 机制 | 防御目标 |
|--------|------|---------|
| origin | `e.origin === "null"`（srcdoc iframe 为 opaque origin） | 阻止任意 origin 页面伪造 |
| source | `e.source === iframeRef.current.contentWindow` | 阻止同进程内其他 iframe/窗口伪装 |
| nonce（SEC-04） | 面板挂载期 `crypto.getRandomValues` 生成 128 位 nonce，拼入注入脚本；父窗口校验 `e.data.nonce` | 阻止不知密钥的外部伪造 |
| 信任标记 | 合成 `KeyboardEvent` 上定义 `__slterm_postMessage = true` | 预留：未来可区分物理按键与 postMessage 重放 |

**威胁模型**：被预览 HTML 的自身内联脚本可读取注入脚本中的 nonce 并伪造消息，故 nonce 不防内部伪造。防线分层：nonce = 外部防线；global 命令集最小化兜底——当前 global context 仅 `global.closeTab`（低风险）。扩充 global 命令前须重评估本威胁模型。

`e.origin === "null"` 由 WHATWG HTML 规范推断，未在真实 WebView2 环境单独实测，正确性由 L4 真实 WebView2 中 postMessage 往返验收。

### HTML 内联脚本/事件执行（CSP 放行）

预览 HTML 的内联 `<script>` 与内联事件属性能执行，依赖全局 Tauri CSP 含 `script-src 'self' 'unsafe-inline'` **且** `dangerousDisableAssetCspModification: ["script-src"]`。

- `<iframe srcDoc>` 加载的 `about:srcdoc` 继承父窗口 CSP，子策略只能收紧不能放宽，故必须放宽主窗口 CSP。
- 只加 `'unsafe-inline'` 而不关 nonce 注入时，Tauri 注入的 nonce 会使 `'unsafe-inline'` 被浏览器忽略，srcdoc 内联脚本仍被拦。
- 代价是主应用全局失去 script 的 nonce 加固（`default-src 'self'` 仍拦远程脚本加载）。面板仅用于预览可信本地 HTML。**勿收紧回严格 script-src——会静默破坏预览**。

### gitshow：只读但可聚焦

`GitShowPanel` 用 `EditorState.readOnly.of(true)` 阻止编辑，**不使用** `EditorView.editable.of(false)`。后者设 `contentEditable=false` 会导致编辑器不可聚焦，CM6 内部键绑定和 ShortcutRegistry 全部失效。

### diff：双栏占位对齐 + 滚动同步

`DiffPanel` 横向均分两栏：左 = HEAD 只读 + HEAD gutter + 占位行，右 = 工作区可编辑 + workdir gutter + 占位行。

- **占位对齐**：`computeAlignment(hunks)` 纯函数根据 DiffHunk[] 计算左右两侧需插入占位行的位置与数量——纯新增行左侧插占位，纯删除行右侧插占位，modified 行数不等时少的一侧插差值。通过 CM6 `Decoration.widget` 渲染块级占位行。
- **垂直滚动同步**：一侧 `.cm-scroller` scroll → 另一侧 `scrollTop` 跟随（`syncingRef` 防循环）。水平滚动不同步。
- **CSS flexbox `min-width: auto` 修复**：DOM 层级为双层 flex 嵌套，CM6 `.cm-content`（`flex-shrink: 0`，`white-space: pre`）会随长行横向扩展撑开 flex 子项，导致分界线偏离 50%。四个 flex 子项均加 `minWidth: 0` 显式覆盖。`overflow: clip` 保留——裁剪溢出但不吸收滚轮事件。
- **容器 ref 桥接**：DiffPanel 三态中容器 div 仅在 `"ready"` 态挂载。`renderKey` state + `bridgedRef` guard + `useEffect([state.kind])` 在 commit 后触发额外渲染，确保 `useFontSizeWheel` / `usePanelFocus` 在容器就绪后收到非 null DOM 元素。

### settings：设置中心壳 + 配置页注册表分派（F11）

`SettingsPanel`（F11）为**壳容器**：左导航（组序 global→project，固定 180px）+ 右配置页槽位，槽位经 `SettingsPageRegistry`（features/settingsCenter）分派渲染 `page.component`（`key={selectedPage}` 强制重挂载——ADR-0001 先例，页内状态随卸载丢弃）。配置页注册集中在 `features/settingsCenter/pages.ts`（side-effect import 触发点：SettingsPanel 顶部 import 即注册全部配置页），壳零直接引用任何具体配置页组件，新增配置页 = pages.ts 追加一条 register。

- **壳是 params 持久化单点**：选中切换与 `onPageParamsChange`（pageParams[selectedPage] 槽 merge patch）统一经 `persistParams`（`api.updateParameters` + 显式 `onLayoutChange(saveLayout)` + 按 `settings-` 前缀解析 pageId → `updatePageLayout`）——updateParameters 不触发 onDidLayoutChange，必须显式保存（F8 先例）。
- **dirty 汇聚（SC-FE-07）**：页组件经 `SettingsPageProps.onDirtyChange` 上报 → 壳维护 dirtyMap（导航项 7px 中性色圆点，不用 F3 四态色防语义混淆）+ 同步 `dirtyRegistry`（与 DefaultTab × 关闭拦截共享同一真值源，防两处状态漂移）。切配置页时当前页 dirty → `confirmDialog` 确认丢弃（askGuard 500ms 防循环，照 hub 先例）；× 关闭拦截在 workspace 层（见 workspace/CLAUDE.md）。
- **切项目自动关闭（SC-FE-08）**：订阅 activePageId 所属项目 ≠ 面板所属项目 → 关闭。初始评估（布局恢复挂载即不一致，新挂载不可能 dirty）静默关；变化触发 dirty 守卫 confirmDialog，取消则不关（面板暂留非活跃项目，尊重用户选择）；`activePageId === null` 不动（删除末页/启动瞬态，防连锁误关）。
- **isAlwaysRenderPanel 不加入 settings（决策写死，SC-FE-06）**：同 editor/gitshow/diff——重建无视觉闪屏，状态在 params/store；未保存 dirty 随卸载丢失与旧 hooksConfig 面板行为一致继承，不新增 always 内存开销。
- **corrupted 警示条**：挂载 `loadSettings()` → corrupted → 顶部警示条（× 可关，`data-e2e="settings-corrupted-banner"`，不阻塞）。L2 覆盖（loadSettings mock），L4 豁免登记——写坏文件需沙箱外写，无命令通道。
- claude 专属 hooks 编辑器归域 `features/cliProfiles/profiles/claude/configEditor/`（KZ-1，见 cliProfiles/CLAUDE.md），经 profile 的 `configEditor` 字段挂入；本面板经 HooksSettingsPage 页组件接入，不再跨 features 引用。

### Ctrl+C 保留为中断

`keyboard.ts` 的 `createTerminalShortcuts` 不注册 Ctrl+C 命令——ShortcutRegistry 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY，claude 用它取消操作。

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

`terminal.copy`/`terminal.paste`/`terminal.newline` 均为可重绑的注册命令，handler 经 `getActiveTerminal()` 派发到聚焦终端。window capture 命中即 `stopPropagation`，事件到不了 xterm；仅 capture 失效时委托层兜底。

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

Claude Code 在用户主动 Ctrl+C 中断时不发射任何 hook 事件。四态状态机 `working` 无中断出边——中断后页签滞留 `working` 直至下一事件覆盖。下一事件（UserPromptSubmit/Stop 等）会自动覆盖；中断回提示符约 60s 无操作 → `idle_prompt` Notification → 自动转 `attention`。

## 外部坑/红线

- **xterm.js `open()` 不可复用**：同实例二次 `open()` 抛异常，必须每次挂载新建实例。
- **WebGL 检测不带 `failIfMajorPerformanceCaveat`**（FE-26）：GPU blocklist 场景会误杀软件渲染，导致 DOM renderer 快滚掉帧。
- **WebGL 加载时序（FE-34）**：`setupWebglWithRetry` 必须在 `term.open(container)` 之后，否则 win10 纯黑屏。
- **proposeDimensions NaN**（xtermjs#4338）：WebGL 未就绪时可能返回 NaN，必须 `Number.isFinite()` 守卫后再传 `pty.resize()`。
- **iframe `allow-same-origin` 禁用**：与 `allow-scripts` 组合是已知危险组合（Tauri CVE-2024-35222），且会导致 Tauri 注入 App JS 劫持片段导航。
- **CSP 全局放宽**：srcdoc iframe 继承父窗口 CSP，必须主窗口 `script-src 'self' 'unsafe-inline'` + `dangerousDisableAssetCspModification: ["script-src"]`。收紧会静默破坏 HTML 预览。
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
- **编辑器测试模式**见 `@editor/CLAUDE.md`。

## 添加新面板类型的步骤

1. 在 `src/panels/` 下创建 `newtype/` 目录，含 `index.ts`、`NewTypePanel.tsx` 和必要的 hooks。
2. 在 `src/panelRegistry.ts` 注册组件映射（**无 `src/panels/index.ts` barrel**——各面板经 `panelRegistry.ts` 顶部逐文件 import 直连）。
3. 在 `PANEL_TYPES` 数组中追加 `"newtype"`。
4. 如涉及新 IPC 命令，在 `src-tauri/capabilities/` 显式放行。
