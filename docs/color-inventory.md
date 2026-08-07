# 前端颜色清单（全量，含默认色）

> 本文档回答：**项目中所有前端颜色分别是什么**。按颜色来源分组，每条标注消费区域。色值从源码原文直抄，改色前以此为准。
> 配套文档：[`color-implementation.md`](./color-implementation.md)（实现机制与改色影响面）。
> 状态：长期参考文档。2026-08 配色系统重构后，token 值定义于 `src/theme/schemes/darcula.ts`（`ColorScheme.ui` 段），`colors.ts` 为 facade 代理导出（31 个，名称不变）；终端/编辑器/dockview/allotment 四通道映射见 implementation.md §4。

## 1. `src/theme/colors.ts` — facade token 全表（31 个导出，值定义于 `schemes/darcula.ts` ui 段）

### 1.1 文件名 git 状态色 `GIT_FILE_COLORS`（7 键）

消费区域（3 处，非逐行标注）：
- **Commit 视图文件名**：`CommitFileList.tsx` 查表（未命中回退 `EXPLORER_COLORS.fg`）
- **文件浏览器 FileIcon**：模块内 `statusColorMap` 全量 7 键引用
- **FileTree 行内 git 状态色**：直接查表 `GIT_FILE_COLORS[gitStatus] ?? EXPLORER_COLORS.fg`

| token | 色值 | 用途 |
|-------|------|------|
| modified | `#6897BB` | 已修改文件 |
| added | `#629755` | 新增文件 |
| untracked | `#D1675A` | 未跟踪文件 |
| deleted | `#6C6C6C` | 已删除文件 |
| renamed | `#3A8484` | 重命名文件 |
| conflict | `#D5756C` | 冲突文件 |
| ignored | `#848504` | 忽略文件 |

### 1.2 行内 diff 边栏色 `GIT_GUTTER_COLORS`（3 键）

消费区域：编辑器/双栏 diff 的 gutter 标记（`gitGutter.ts`）。

| token | 色值 | 用途 |
|-------|------|------|
| modified | `#374752` | ModifiedMarker 背景 |
| added | `#384C38` | AddedMarker 背景 |
| deleted | `#656E76` | DeletedMarker 三角 border-bottom |

> 原 `whitespaceOnly` 死键已删除（spec §9.2 死配置清理）。

### 1.3 文件浏览器通用色 `EXPLORER_COLORS`（5 键）

> 口径声明：本节消费位置为**代表性摘要（非全量）**；精确全量以 grep 为准。

消费区域：文件树 + 被 agentStatus/claudeHistory/commit 区借用（fg/arrowClosed）。

| token | 色值 | 消费位置 |
|-------|------|---------|
| bg | `#1E1E1E` | `ExplorerPanel.tsx`（文件树背景） |
| fg | `#D4D4D4` | 10 处：`FileTree.tsx`（含 fallback 链）、`FileIcon.tsx`、`CommitFileList.tsx`、`AgentStatusView.tsx`、`ClaudeHistorySections.tsx`、`HistorySessionList.tsx` |
| hover | `#2A2D2E` | `FileTree.tsx`（行 hover） |
| arrowClosed | `#6C6C6C` | 5 处：`FileTree.tsx`、`AgentStatusView.tsx`、`ClaudeHistorySections.tsx`、`HistorySessionList.tsx`、`CommitFileList.tsx` |
| arrowOpen | `#D4D4D4` | `FileTree.tsx`（展开箭头） |

> 原 `selected` 死键已删除（spec §9.2 死配置清理）——文件树选中用 `EXPLORER_SELECTION_BG`，侧栏选中用 `SIDEBAR_COLORS.selected`。

### 1.4 通用 UI 色（21 个标量导出）

> 口径声明：本节各表「消费区域」**全部为全量清单**（2026-08-06 grep 复核，生产 `src/` 不含 import 行）——`INPUT_BORDER`/`FOCUS_BORDER`/`DIM_FG`/`HTML_PANEL_LOADING_FG` 四者亦为全量清单（34/10/10/15 处逐行核对全部命中）。精确计数：`PANEL_BG` **21 处**、`SIDEBAR_FG` **27 处**、`INPUT_BORDER` **34 处**、`FOCUS_BORDER` **10 处**、`DIM_FG` **10 处**、`HTML_PANEL_LOADING_FG` **15 处**。行号变化时以 grep 为准。

**背景色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `PANEL_BG` | `#1E1E1E` | 21 处：App/ErrorBoundary/SideBarArea/SidebarTree（`--sb-bg` 映射）/ActivityBar/AgentStatusView/CommitView/HtmlPanel/TerminalPanel/DiffPanel/GitShowPanel/EventTree/GuiMode/HooksConfigPanel/JsonMode/MatcherTester |
| `SIDEBAR_BG` | `#252526` | 4 处：FileTree（上下文菜单底色）、HistorySessionList、SessionActionDialog、CommitFileList |
| `SECONDARY_BG` | `#2D2D2D` | 3 处：PageDockviewHost（按钮）、ErrorBoundary、GuiMode |
| `APP_BG` | `#1e1e2e` | App.tsx 根容器 |
| `APP_BG_PRIMARY` | `#1e1e2e` | → `ROOT_CSS_VARS` → `--sl-bg-primary`（全局背景） |
| `EDITOR_BG` | `#282C34` | 5 处：EditorPanel、DiffPanel、GitShowPanel、JsonMode；**与 editor.overrides.background 同源**（CM6 编辑器背景经 editorColorOverrides 对齐此值） |

> 原 `DROPDOWN_BG`（零消费）、`APP_BG_SECONDARY`（零消费）已删除（spec §9.2 死配置清理）。

**前景/文字色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `SIDEBAR_FG` | `#D4D4D4` | 27 处：GuiMode/FileTree/MatcherTester/ClaudeHistorySections/HooksConfigPanel/EventTree/JsonMode/SessionActionDialog/HistorySessionList/ActivityBar/CommitFileList |
| `ERROR_FG` | `#F44747` | 17 处：ErrorBoundary/HtmlPanel/DiffPanel/GitShowPanel/EventTree/GuiMode/HooksConfigPanel（outdated 状态）/MatcherTester/HandlerForm |
| `PLACEHOLDER_FG` | `#808080` | 5 处：SidebarTree（占位符）、PageDockviewHost（页签关闭按钮）、ErrorBoundary、HistorySessionList（禁用项灰显）、HandlerForm |
| `BUTTON_FG` | `#CCCCCC` | 7 处：PageDockviewHost、SessionActionDialog、HandlerForm |
| `DIM_FG` | `#999999` | AgentStatusRow/HistorySessionRow/ErrorBoundary/HandlerForm/SessionActionDialog |

**交互控件色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `INPUT_BG` | `#3C3C3C` | 11 处：GuiMode/ClaudeHistorySections/SidebarTree/FileTree/HandlerForm |
| `INPUT_BORDER` | `#6C6C6C` | **全应用最高频 token（34 处）**。代表位置：GuiMode 边框、ClaudeHistorySections、HistorySessionList、CommitView、CommitFileList、ExplorerPanel、AgentStatusView、HandlerForm、JsonMode、TerminalPanel 加载文案、App、PageDockviewHost Watermark、HooksConfigPanel、SidebarTree、FileTree、EventTree、MatcherTester |
| `FOCUS_BORDER` | `#007ACC` | 10 处：FileTree 重命名框、ActivityBar 选中指示条、SidebarTree、JsonMode hover、GuiMode 组框三目 |
| `ACTIVE_SELECTION_BG` | `#094771` | 6 处：GuiMode、EventTree、HistorySessionList、SidebarTree、CommitFileList、FileTree |
| `SEPARATOR_BG` | `#444` | 8 处：DiffPanel（分栏线）、PageDockviewHost、ErrorBoundary、AgentStatusView、CommitView、ExplorerPanel、HandlerForm |
| `CONTEXT_MENU_BORDER` | `#454545` | 5 处：FileTree、HistorySessionList、SessionActionDialog、CommitFileList |

**阴影 / HTML 面板色 / 强调底色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `SHADOW_MENU` | `rgba(0,0,0,0.5)` | SessionActionDialog 遮罩 |
| `HTML_PANEL_LOADING_FG` | `#6C6C6C` | 15 处："加载中…"文案——HtmlPanel/DiffPanel/GitShowPanel/MatcherTester/JsonMode/HooksConfigPanel/GuiMode/EventTree/CommitView |
| `HTML_PANEL_IFRAME_BG` | `#FFFFFF` | HtmlPanel iframe 白底 |
| `ON_ACCENT_FG` | `#FFFFFF` | JsonMode 事件导航 hover 文字（`:216`）——**新增**（spec §9.2 收编原 `style.color = "#FFFFFF"` 硬编码） |

### 1.5 侧栏配色组 `SIDEBAR_COLORS`（8 键）

消费区域：**27 处（10 文件）**，全量清单：
- **SidebarTree（12 处）**：映射 `--sb-fg/--sb-hover/--sb-selected`（注意 `--sb-bg` = `PANEL_BG`，非本对象 bg——两条独立路径，改侧栏背景需分别改动）+ bg / contextMenuBorder / contextMenuShadow / fg / border / fg / fg / border
- **ActivityBar（3 处）**：border / selected / hover
- **AgentStatusRow（4 处）**：hover / fg 标题 / border 用量条轨道 / fg 条件分支
- **其余 8 处**：AgentStatusView treeGuide、ClaudeHistorySections treeGuide、HistorySessionRow fg、HistorySessionList contextMenuShadow / treeGuide、SessionActionDialog contextMenuShadow、CommitFileList contextMenuShadow、FileTree contextMenuShadow

| token | 色值 | 备注 |
|-------|------|------|
| bg | `#252526` | = SIDEBAR_BG |
| fg | `#D4D4D4` | = SIDEBAR_FG |
| hover | `#2A2D2E` | = EXPLORER_COLORS.hover |
| selected | `#37373D` | 侧栏选中态（ActivityBar `:139`） |
| border | `#444` | = SEPARATOR_BG |
| contextMenuBorder | `#454545` | = CONTEXT_MENU_BORDER |
| contextMenuShadow | `0 4px 12px rgba(0,0,0,0.5)` | 阴影字符串（非色值） |
| treeGuide | `#3C3C3C` | 树形引导线（agent 侧栏层级竖线） |

### 1.6 其余分组

| token | 色值 | 消费区域 |
|-------|------|---------|
| `ERROR_BANNER_BG` | `#5A1D1D` | ExplorerPanel 沙箱错误横幅 |
| `ERROR_BANNER_BORDER` | `#8B0000` | 同上 |
| `ERROR_BANNER_FG` | `#F48771` | 同上 |
| `EXPLORER_SELECTION_BG` | `#094771` | FileTree 选中行、HistorySessionRow；**与 ACTIVE_SELECTION_BG 同值** |
| `AGENT_STATUS_USAGE_COLORS.low` | `#629755` | AgentStatusRow `usageBarColor`：<50% |
| `AGENT_STATUS_USAGE_COLORS.medium` | `#BBB529` | 50-80% |
| `AGENT_STATUS_USAGE_COLORS.high` | `#F44747` | >80%；**与 ERROR_FG 同值** |
| `ROOT_CSS_VARS["--sl-bg-primary"]` | `#1e1e2e` | main.tsx 注入 → App.css `var()`（值 = ui.appBgPrimary） |
| `ROOT_CSS_VARS["--sl-fg-primary"]` | `#cdd6f4` | main.tsx 注入 → App.css `var()`（值 = ui.appFg，收编自原 App.css hex） |

> 原 `ROOT_CSS_VARS["--sl-bg-secondary"]` 已随 `APP_BG_SECONDARY` 删除（零消费，spec §9.2）。

## 2. `src/panels/terminal/theme.ts` — xterm 终端主题 adapter（25 键）

消费区域：所有终端面板（`useXterm` → `useTerminalInstance` → `new Terminal`）。

> theme.ts 现为 **adapter**：`theme: { ...schemeRegistry.getActive().terminal }`——25 键全部来自 active 方案 terminal 段（darcula.ts 值），非独立主题定义。下表为 darcula 方案值。

**基础 6 键**：

| 项 | 色值 | 项 | 色值 |
|----|------|----|------|
| foreground | `#D4D4D4` | selectionBackground | `#264F78` |
| background | `#1E1E1E` | selectionForeground | `#D4D4D4` |
| cursor | `#D4D4D4` | cursorAccent | `#1E1E1E` |

**滚动条 3 键**（spec §9.2 勘误 2 显式化——原为 xterm 运行期算法派生值 foreground × 20%/40%/50% 透明度，现显式配置）：

| 项 | 色值 |
|----|------|
| scrollbarSliderBackground | `rgba(212,212,212,0.2)` |
| scrollbarSliderHoverBackground | `rgba(212,212,212,0.4)` |
| scrollbarSliderActiveBackground | `rgba(212,212,212,0.5)` |

**ANSI 16 色**：

| 项 | 色值 | 项 | 色值 |
|----|------|----|------|
| black | `#000000` | brightBlack | `#666666` |
| red | `#CD3131` | brightRed | `#F14C4C` |
| green | `#0DBC79` | brightGreen | `#23D18B` |
| yellow | `#E5E510` | brightYellow | `#F5F543` |
| blue | `#2472C8` | brightBlue | `#3B8EEA` |
| magenta | `#BC3FBC` | brightMagenta | `#D670D6` |
| cyan | `#11A8CD` | brightCyan | `#29B8DB` |
| white | `#E5E5E5` | brightWhite | `#FFFFFF` |

ANSI 16 色全部覆盖 → xterm 默认调色板运行时失效（默认值仅作缺省兜底）。foreground/background 与 ui 段的 SIDEBAR_FG/PANEL_BG 同值（值单点分别在 ui/terminal 两段，改一侧需手动同步另一侧——terminal 段是 xterm 专用色板，未引用 ui 槽位）。

## 3. `App.css` + `index.html`

| 位置 | 内容 | 说明 |
|------|------|------|
| `App.css:5` | 注释：`--sl-bg-primary / --sl-fg-primary 由 ROOT_CSS_VARS（colors.ts）经 main.tsx 注入` | **App.css 已无 hex 定义**——原 `--sl-fg-primary: #cdd6f4`（:6）收编为 ui.appFg，`--sl-fg-secondary`（:7）死配置删除（spec §9.2） |
| `App.css:9` | `color-scheme: dark` | 非色值，驱动原生控件/滚动条暗色渲染 |
| `App.css` :root / html,body,#root | `color: var(--sl-fg-primary)` / `background: var(--sl-bg-primary)` | 全局默认前景/背景（值 `#cdd6f4`/`#1e1e2e` 由 main.tsx 从 active 方案注入；文字色被组件 token 覆盖） |
| `index.html:10` | body `background: #1e1e2e` | 防白闪首帧，与 APP_BG_PRIMARY 同值但独立硬编码（启动链 fail-safe，darcula.ts 头注释交叉引用） |
| `tauri.conf.json:20` | `backgroundColor: "#1e1e2e"` | 窗口原生底色（theme: "Dark"），同值独立硬编码 |

## 4. 组件硬编码（不经过 token）

| 位置 | 色值 | 性质 |
|------|------|------|
| `src/main.tsx` 超时页 | `background:#1e1e1e;color:#f44747` | 启动链合理例外（IPC 超时 fail-safe，React 未挂载） |

> 原 `JsonMode.tsx:213` `style.color = "#FFFFFF"` 真违规已收敛——现 `JsonMode.tsx:216` 引用 `ON_ACCENT_FG` token（ui.onAccentFg，spec §9.2）。**真违规清零**。

**`transparent` 全量分类**（合规非硬编码，`选中 ? token : "transparent"` 模式为主）：

| 类别 | 位置 | 说明 |
|------|------|------|
| 真 toggle（条件三目，8 处） | AgentStatusRow、HistorySessionRow、SidebarTree、ActivityBar、GuiMode、EventTree、HooksConfigPanel、FileTree | 选中/激活态置 token，否则 transparent |
| 事件对（1 处） | JsonMode | hover 置 `FOCUS_BORDER`（`ON_ACCENT_FG` 管文字色）/ leave 置 transparent |
| mouseout/leave 重置回调（7 处） | SidebarTree、FileTree、HistorySessionList、CommitFileList | `style.background = "transparent"` |
| 静态常量（8 处） | ActivityBar（iconButtonStyle）、GuiMode、JsonMode、MatcherTester、HandlerForm、ClaudeHistorySections（refreshButtonStyle）、SessionActionDialog | style 定义里固定 transparent |
| 变量式开关（1 处） | ActivityBar | `let bg = "transparent"`，分支置 token |
| 非独立 transparent 字面量（2 处） | ActivityBar（`borderLeft: "2px solid transparent"` 占位，**带引号**字符串值）、gitGutter（CSS 三角 border 透明，位于模板字符串内） | 嵌入边框值/模板字符串，非独立字面量 |

## 5. 三方库通道（经 overrides.ts / adapter 映射 active 方案）

### 5.1 CodeMirror 主题（`editor` 段 → CM6）

消费区域：**4 个生产导入文件**——`useCodeMirror.ts` 等 4 处经 `editorTheme` + `editorColorOverrides()`（`overrides.ts`）替换原 oneDark 直 import（`useCodeMirror.ts:289-290`、`GitShowPanel.tsx:142-143`、`DiffPanel.tsx:520-521/566-567`、`JsonMode.tsx:162-163`）；+ 2 处测试 mock（gitshow-panel.test.tsx、hooks-config-jsonmode.test.tsx，mock 包继续生效——darcula.ts 直 import 透出）。「五个 CM6 面板」= 5 个面板实例 = editor 1 + gitshow 1 + diff 左右 2 + JsonMode 1。

**语法高亮色板**（来自 `@codemirror/theme-one-dark` 包，经 editor.theme 透出，决策 D6——色值仍由包定义）：

| 名 | 色值 | 语法角色 |
|----|------|---------|
| chalky | `#e5c07b` | 类型/类名/数字/注解 |
| coral | `#e06c75` | 函数名/删除线/标题 |
| cyan | `#56b6c2` | 运算符/URL/正则 |
| invalid | `#ffffff` | 非法 |
| ivory | `#abb2bf` | 正文/定义名 |
| stone | `#7d8799` | 注释/meta/链接 |
| malibu | `#61afef` | 函数调用 |
| sage | `#98c379` | 字符串/新增 |
| whiskey | `#d19a66` | 常量/bool/atom |
| violet | `#c678dd` | 关键字 |

**背景族**（oneDark 包内）：darkBackground `#21252b`（面板）、highlightBackground `#2c313a`（活动行 gutter/补全选中）、background `#282c34`（编辑器底，被 editor.overrides.background 显式覆盖对齐 ui.editorBg）、tooltipBackground `#353a42`、selection `#3E4451`、cursor `#528bff`。

**editor.overrides 覆盖值**（darcula.ts，经 editorColorOverrides 生效——spec §4.4 现行有效值）：

| 键 | 色值 | 生效目标 |
|----|------|---------|
| background | `#282C34` | 编辑器背景（对齐 ui.editorBg） |
| lint.error / warning / info / hint | `#f11` / `orange` / `#999` / `#66d` | JsonMode 诊断波浪线（原 @codemirror/lint 默认） |
| lint.activeBackground | `#ffdd9980` | lint 消息激活背景 |
| lint.tooltipBackground / tooltipBorder | `#2e343e` / `#444` | lint tooltip 底色/边框 |
| searchMatch.match / matchOutline | `#72a1ff59` / `#457dff` | 搜索匹配背景/描边（原 oneDark 字面量规则） |
| searchMatch.selected | `#6199ff2f` | 选中匹配背景 |
| searchMatch.selectionMatch | `#aafe661a` | 多匹配整体背景 |

**oneDark 包内其余字面量色**（未覆盖，包内规则）：`.cm-panels` 边框 `2px solid black`、`.cm-activeLine` `#6699ff0b`、`.cm-matchingBracket` `#bad0f847`、`.cm-foldPlaceholder` 文字 `#ddd`。

### 5.2 Dockview（`libraries.dockview` 20 条 → 内联 CSS 变量）

消费区域：全部 Dockview 页签/布局——`PageDockviewHost.tsx:368` 根 div style 展开 `dockviewVarStyle()`（20 条 `--dv-*` 变量内联注入），`className="dockview-theme-dark"` 保留（布局样式仍由 dockview.css 提供）。

> 值定义于 `schemes/darcula.ts` libraries.dockview 段；**与 ui 同值的条目引用 ui 槽位构造（值单点，重构后已联动）**。下表为 darcula 值。

| 变量 | 色值 | 说明 |
|------|------|------|
| `--dv-group-view-background-color` | ui.panelBg（`#1E1E1E`） | 主背景 |
| `--dv-tabs-and-actions-container-background-color` | ui.sidebarBg（`#252526`） | 页签容器背景 |
| `--dv-activegroup-visiblepanel-tab-background-color` | `#1E1E1E` | 活跃页签背景 |
| `--dv-activegroup-hiddenpanel-tab-background-color` | ui.secondaryBg（`#2D2D2D`） | 隐藏页签背景 |
| `--dv-inactivegroup-visiblepanel-tab-background-color` | `#1E1E1E` | 非活跃组同值 |
| `--dv-inactivegroup-hiddenpanel-tab-background-color` | ui.secondaryBg（`#2D2D2D`） | 非活跃隐藏 |
| `--dv-tab-divider-color` | `#1E1E1E` | 页签分隔 |
| `--dv-separator-border` | ui.separatorBg（`#444`） | 分隔条 |
| `--dv-paneview-header-border-color` | `rgba(204,204,204,0.2)` | 页眉边框 |
| `--dv-activegroup-visiblepanel-tab-color` | `#FFFFFF` | 活跃页签文字 |
| `--dv-activegroup-hiddenpanel-tab-color` | `#969696` | 隐藏页签文字 |
| `--dv-inactivegroup-visiblepanel-tab-color` | `#8F8F8F` | 非活跃页签文字 |
| `--dv-inactivegroup-hiddenpanel-tab-color` | `#626262` | 非活跃隐藏 |
| `--dv-drag-over-background-color` | `rgba(83,89,93,0.5)` | 拖拽覆盖 |
| `--dv-icon-hover-background-color` | `rgba(90,93,94,0.31)` | 图标 hover |
| `--dv-floating-box-shadow` | `0 8px 32px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.25)` | 浮动组阴影 |
| `--dv-floating-border` | `1px solid rgba(255,255,255,0.1)` | 浮动组边框 |
| `--dv-tabs-container-scrollbar-color` | `#888888` | 页签滚动条 |
| `--dv-scrollbar-background-color` | `rgba(255,255,255,0.25)` | 通用滚动条（**新增显式覆盖**——原走 dockview.css:29 全局兜底） |
| `--dv-paneview-active-outline-color` | `dodgerblue` | 拖拽 outline |

> 非色值/透明类变量（`--dv-sash-color`、`--dv-active-sash-color`、`--dv-drag-over-border-color`、`--dv-floating-group-border`、`--dv-floating-group-dragging-opacity`、`--dv-tab-group-line-opacity`）不入方案，保持 dockview.css 默认。页签分组色（grey `#5f6368`、blue `#1a73e8` 等 9 色，仅 tabGroup API 用，本项目未用）不入方案。`.dv-debug` 前缀硬编码色仅调试模式生效，正常 UI 不渲染。

### 5.3 Allotment（`libraries.allotment` 2 键 → 内联 CSS 变量）

消费区域：三栏/上下分栏——`Workspace.tsx:228` 根容器 style 合并 `allotmentVarStyle()`，CSS 变量继承覆盖内层 SideBarArea 的 Allotment。

| 变量 | 色值 | 用途 |
|------|------|------|
| `--separator-border` | `rgba(128,128,128,0.35)` | 分栏分割线 |
| `--focus-border` | `#007fd4` | 拖拽 sash hover 高亮（VS Code 系） |

## 6. 依赖默认值（未显式配置）

### 6.1 组件库层

| 项 | 依赖方 | 默认色 |
|----|--------|--------|
| 终端滚动条 | xterm | 滑块三键**已显式配置**（terminal 段 `scrollbarSliderBackground/Hover/Active` = `rgba(212,212,212,0.2/0.4/0.5)`——spec §9.2 勘误 2：原「运行期算法计算」派生值显式化）；CSS 兜底 `rgba(100,100,100,0.4)`（xterm.css:229）仍作用于滚动条**箭头按钮**（`.xterm-scra`）——滑块与箭头按钮两处色需分别处理 |
| 终端合成窗口（IME） | xterm `.composition-view` | 背景 `#000`、文字 `#FFF`（库内部默认，不入方案） |
| 终端 viewport 衬底 | xterm `.xterm-viewport` | `#000`（被渲染器主题背景覆盖，仅滚动条衬底可见） |
| 终端 OSC 8 链接色 | xterm | **不可配置**——xterm 6.x `ITheme` 无 `link` 键（spec §9.2 勘误 1，`xterm.d.ts` 全键列表一手确认），链接渲染色为库默认；`useXterm.ts` 仅设 `linkHandler.activate`（打开 URL 行为） |
| CM 剩余默认 | oneDark 未覆盖项 | placeholder 硬编码灰 `#888`（@codemirror/view baseTheme，editor/gitshow/diff/JsonMode 四面板真实渲染色）、搜索面板输入框基础态（继承 oneDark 面板底色） |
| CM lint 诊断色系 | @codemirror/lint | **已收编**进 editor.overrides（§5.1）——JsonMode 语法错误/schema 违规波浪线颜色随方案 |
| CM 搜索匹配高亮 | @codemirror/search | **已收编**进 editor.overrides.searchMatch（§5.1）——editor/diff/gitshow 4 个生产位置 |
| Dockview 滚动条 | dockview | `--dv-scrollbar-background-color` **已显式覆盖**（libraries.dockview，§5.2）；页签滚动条由 `--dv-tabs-container-scrollbar-color: #888888` 决定；浮动组拖拽透明度 0.5、tab 溢出下拉（容器 `#1e1e1e`，active 行 `#1e1e1e`，inactive 行 `#2d2d2d`）为 dockview.css 内置 |

### 6.2 浏览器/WebView2 层（跟随 `color-scheme: dark` + 窗口 `theme: "Dark"`，无显式定制）

| 项 | 涉及区域 |
|----|---------|
| 原生滚动条 | 所有 `overflowY: auto` 容器（SidebarTree/FileTree/AgentStatusView/HistorySessionList/hooksConfig/HtmlPanel） |
| 文本选区 `::selection` | 普通 DOM 文本（侧栏/explorer/表单；CM6 与 xterm 已各自覆盖） |
| input 聚焦环（focus ring） | GuiMode/MatcherTester/HooksConfigPanel 输入框、FileTree 重命名框、SessionActionDialog（`FOCUS_BORDER` token 仅覆盖部分自绘边框，不统一） |
| placeholder 色 | 全部 `<input>`/`<select>`（`PLACEHOLDER_FG` 仅用于树形占位符，未用于 input） |
| 原生右键菜单 | 输入框/文本选区（Explorer/Sidebar 自定义菜单已用 `CONTEXT_MENU_BORDER` + 阴影） |
| 原生表单控件 | GUI 模式 select/checkbox/按钮 |

> **链接色/原生控件澄清**：应用自身 UI **无 `<a>` 元素**（生产代码零命中）——链接色仅存在于 HTML 面板 iframe 内的用户内容（内容层，系统默认渲染）；WebView2 原生右键菜单/对话框/select 下拉为系统渲染色，由系统决定**不可配置**。

其余系统默认（字体等）遵循 WebView2 暗色默认，不逐项列。

## 7. 内容层（不可配置，改色无需处理）

- **emoji 图标**：文件图标映射（FileIcon）、F3 四态（⚡🟡✅❌）——颜色由系统 emoji 字体决定
- **终端输出的 ANSI 内容色**：claude/pwsh 输出经 xterm 16 色映射渲染，色值属内容非 UI 配置
- **shell 集成脚本**：`src-tauri/assets/shell-integration.ps1` **不含 ANSI 色序列**（仅 OSC 7/9;9/133 控制序列），无需处理
- **应用图标**：`public/tauri.svg`（`#FFC131`/`#24C8DB`）为**孤儿文件**（从未被任何代码引用，改色可忽略）——但 Vite 会将 `public/` 原样拷贝进 `dist/`，文件实际随打包产物发布（彻底清理孤儿资产时可删除）；应用图标实际来自 `src-tauri/icons/`（PNG/ICO 二进制）。另 `index.html:5` favicon 指向不存在的 `/vite.svg`（404，与颜色无关）

## 8. 附录：测试文件色值断言索引（改色须同步——分两类）

> 两类守卫语义：「改色即红（字面量守卫）」= 断言字面量色值，改 token 值测试必红，改色时必须同步；「token 联动校验（组件-token 脱钩才红）」= 渲染色 `===` `hexToRgb(token)` 两侧同源，改 token 值两侧同变**测试不会红**，只守卫组件与 token 脱钩，改色时无需为它们做无效同步。
>
> 行号为代表性位置，非全量——改色时以 grep 为准。

### 8.1 改色即红（字面量守卫）

| 测试文件 | 断言内容 |
|----------|---------|
| `src/__tests__/colors.test.ts` | **全 token 精确 toBe 断言**（it.each 表驱动，断言值 = darcula 方案值——改 darcula 值即红，STS-01）——**treeGuide/contextMenuShadow 除外**（treeGuide 仅键数断言，改 `#3C3C3C` 测试不会红，需人工同步）；已随死配置清理（删 DROPDOWN_BG/APP_BG_SECONDARY/whitespaceOnly/selected）与 ON_ACCENT_FG 新增同步（「通用 UI 色 24 个 token」断言） |
| `test/terminal/theme-options.test.ts`（L3） | 16 色与主题色板一致（E2E-02；色板源 = darcula.ts terminal 段，**:46-61 为 16 色全量副本镜像，改终端配色必须双边同步，有漂移风险**） |
| `explorer-git-status.test.tsx` | git 状态文件名着色 |
| `git-gutter.test.ts` | GutterMarker 颜色（modified/added/deleted 三键） |
| `commit-view-list.test.tsx` | 状态色 rgb 转换 |
| `explorer-selection.test.tsx` | 选中高亮 `rgb(9,71,113)`（EXPLORER_SELECTION_BG）+ hover 色 `rgb(42,45,46)` + hover token hex 断言 `#2A2D2E` |
| `html-panel.test.tsx` | 组件色值断言副本 |
| `sideBarArea.test.tsx` | 组件色值断言副本 |
| `commit-context-menu-ui.test.tsx` | 组件色值断言副本 |
| `explorer-crud-success.test.tsx` | 组件色值断言副本 |
| `main-bootstrap.test.tsx` | fail-safe `#f44747` |
| `use-xterm-integration.test.ts` | canvas mock `#000` |

### 8.2 token 联动校验（组件-token 脱钩才红）

| 测试文件 | 断言内容 |
|----------|---------|
| `agent-status-view.test.tsx` | 用量条分段色（`hexToRgbStr(AGENT_STATUS_USAGE_COLORS.*)` 派生比较） |
| `file-icon.test.tsx` | GIT_FILE_COLORS 全键 + EXPLORER_COLORS.fg（`hexToStyleRgb` 派生比较） |
| `activityBar.test.tsx` | SIDEBAR_COLORS.selected、hover、FOCUS_BORDER 指示条断言（`hexToRgb` 派生比较） |
| `hooks-config-gui.test.tsx` | FOCUS_BORDER、INPUT_BORDER 边框断言 + ACTIVE_SELECTION_BG 选中高亮断言（`hexToRgb` 派生比较） |
| `claude-history-row.test.tsx` | EXPLORER_SELECTION_BG 选中标记（`hexToRgb` 派生比较） |

> `src/__tests__/theme.test.ts`（L2）为 terminalOptions 主题结构断言（adapter 形态下断言 active 方案展开），非色值守卫；`scheme-registry.test.ts` / `overrides.test.ts` 守方案系统结构（四段键数/导出键集合）。
