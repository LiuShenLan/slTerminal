# 前端颜色清单（全量，含默认色）

> 本文档回答：**当前项目中所有前端颜色分别是什么**。按颜色来源分组，每条标注消费区域。色值从源码原文直抄，改色前以此为准。
> 配套文档：[`color-implementation.md`](./color-implementation.md)（实现机制与改色影响面）。
> 状态：临时摸底文档，改色完成后可能更新或归档。

## 1. `src/theme/colors.ts` — 全局 token 全表（9 组注释块、32 个顶层命名导出）

### 1.1 文件名 git 状态色 `GIT_FILE_COLORS`

消费区域（3 处，非逐行标注）：
- **Commit 视图文件名**：`CommitFileList.tsx:263-266` 查表（`:266` 未命中回退 `EXPLORER_COLORS.fg`）
- **文件浏览器 FileIcon**：模块内 `statusColorMap` 全量 7 键引用（`FileIcon.tsx:9-17`，`:74` 查表消费）
- **FileTree 行内 git 状态色**：直接查表 `GIT_FILE_COLORS[gitStatus] ?? EXPLORER_COLORS.fg`（`FileTree.tsx:233`）

| token | 色值 | 用途 |
|-------|------|------|
| modified | `#6897BB` | 已修改文件 |
| added | `#629755` | 新增文件 |
| untracked | `#D1675A` | 未跟踪文件 |
| deleted | `#6C6C6C` | 已删除文件 |
| renamed | `#3A8484` | 重命名文件 |
| conflict | `#D5756C` | 冲突文件 |
| ignored | `#848504` | 忽略文件 |

### 1.2 行内 diff 边栏色 `GIT_GUTTER_COLORS`

消费区域：编辑器/双栏 diff 的 gutter 标记（`gitGutter.ts`）。

| token | 色值 | 用途 |
|-------|------|------|
| modified | `#374752` | ModifiedMarker 背景 |
| added | `#384C38` | AddedMarker 背景 |
| deleted | `#656E76` | DeletedMarker 三角 border-bottom |
| whitespaceOnly | `#4C4638` | ⚠️ **死键**（零消费方——`gitGutter.ts` 仅消费 added/modified/deleted，同 `DROPDOWN_BG` 待遇） |

### 1.3 文件浏览器通用色 `EXPLORER_COLORS`

> 口径声明：本节消费位置为**代表性摘要（非全量）**；精确全量以 grep 为准。

消费区域：文件树 + 被 agentStatus/claudeHistory/commit 区借用（fg/arrowClosed）。

| token | 色值 | 消费位置 |
|-------|------|---------|
| bg | `#1E1E1E` | `ExplorerPanel.tsx:374`（文件树背景） |
| fg | `#D4D4D4` | 10 处：`FileTree.tsx:182/233/234`（含 fallback 链）、`FileIcon.tsx:74/78`、`CommitFileList.tsx:44/266`、`AgentStatusView.tsx:67`、`ClaudeHistorySections.tsx:81`、`HistorySessionList.tsx:73` |
| hover | `#2A2D2E` | `FileTree.tsx:190`（行 hover） |
| selected | `#37373D` | ⚠️ **死键**（无消费方）——文件树选中实际用 `EXPLORER_SELECTION_BG`（`FileTree.tsx:186`），侧栏选中用 `SIDEBAR_COLORS.selected`（`ActivityBar.tsx:139`） |
| arrowClosed | `#6C6C6C` | 5 处：`FileTree.tsx:208`、`AgentStatusView.tsx:75`、`ClaudeHistorySections.tsx:64`、`HistorySessionList.tsx:56`、`CommitFileList.tsx:28` |
| arrowOpen | `#D4D4D4` | `FileTree.tsx:207`（展开箭头） |

### 1.4 通用 UI 色（22 个标量导出）

> 口径声明：本节各表「消费区域」**全部为全量清单**（2026-08-06 grep 复核，生产 `src/` 不含 import 行）——`INPUT_BORDER`/`FOCUS_BORDER`/`DIM_FG`/`HTML_PANEL_LOADING_FG` 四者亦为全量清单（34/10/10/15 处逐行核对全部命中）。精确计数：`PANEL_BG` **21 处**、`SIDEBAR_FG` **27 处**（GuiMode 6 + FileTree 6 + MatcherTester 3 + ClaudeHistorySections 2 + HooksConfigPanel 2 + EventTree 2 + JsonMode 2 + SessionActionDialog/HistorySessionList/ActivityBar/CommitFileList 各 1）、`INPUT_BORDER` **34 处**、`FOCUS_BORDER` **10 处**、`DIM_FG` **10 处**、`HTML_PANEL_LOADING_FG` **15 处**。行号变化时以 grep 为准。

**背景色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `PANEL_BG` | `#1E1E1E` | 21 处：App:198；ErrorBoundary:61/122；SideBarArea:69；SidebarTree:25（`--sb-bg` 映射）；ActivityBar:33；AgentStatusView:150；CommitView:76；HtmlPanel:73；TerminalPanel:106/121；DiffPanel:608/616/654；GitShowPanel:54；EventTree:60；GuiMode:45；HooksConfigPanel:56/63；JsonMode:68；MatcherTester:26 |
| `SIDEBAR_BG` | `#252526` | 4 处：FileTree:82（上下文菜单底色）、HistorySessionList:133、SessionActionDialog:75、CommitFileList:106 |
| `SECONDARY_BG` | `#2D2D2D` | 3 处：PageDockviewHost:82（按钮）、ErrorBoundary:98、GuiMode:105 |
| `DROPDOWN_BG` | `#2A2D2E` | ⚠️ **死 token**（无生产消费方） |
| `APP_BG` | `#1e1e2e` | App.tsx 根容器（:215） |
| `APP_BG_PRIMARY` | `#1e1e2e` | → `ROOT_CSS_VARS` → `--sl-bg-primary`（全局背景） |
| `APP_BG_SECONDARY` | `#2b2b3c` | → `--sl-bg-secondary`（全局次级背景） |
| `EDITOR_BG` | `#282C34` | 5 处：EditorPanel:44、DiffPanel:636/643、GitShowPanel:61、JsonMode:104；**与 oneDark background 同值** |

**前景/文字色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `SIDEBAR_FG` | `#D4D4D4` | 27 处：GuiMode:62/80/91/119/349/501、FileTree:101/456/494/538/593/633、MatcherTester:33/41/48、ClaudeHistorySections:104/115、HooksConfigPanel:84/120、EventTree:84/104、JsonMode:91/217、SessionActionDialog:86、HistorySessionList:166、ActivityBar:50、CommitFileList:125 |
| `ERROR_FG` | `#F44747` | 17 处：ErrorBoundary:62/121；HtmlPanel:165；DiffPanel:617；GitShowPanel:220；EventTree:111/112；GuiMode:127/128；HooksConfigPanel:119（outdated 状态）/265/318/340；MatcherTester:48；HandlerForm:168/172/222 |
| `PLACEHOLDER_FG` | `#808080` | 5 处：SidebarTree:171（占位符）、PageDockviewHost:229（页签关闭按钮）、ErrorBoundary:76、HistorySessionList:149（禁用项灰显）、HandlerForm:162 |
| `BUTTON_FG` | `#CCCCCC` | 7 处：PageDockviewHost:82/111、SessionActionDialog:117/138、HandlerForm:183/209/229 |
| `DIM_FG` | `#999999` | AgentStatusRow（:122/132/141）、HistorySessionRow（:94/98/108）、ErrorBoundary（:89/133）、HandlerForm（:248）、SessionActionDialog（:97） |

**交互控件色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `INPUT_BG` | `#3C3C3C` | 11 处：GuiMode:90/349/501、ClaudeHistorySections:103、SidebarTree:267、FileTree:454/492/536/591/631、HandlerForm:180 |
| `INPUT_BORDER` | `#6C6C6C` | **全应用最高频 token（34 处）**。代表位置：GuiMode 边框（:56/81/92/101/349/501）、ClaudeHistorySections（:105/116/127）、HistorySessionList（:81/90）、CommitView（:30/44）、CommitFileList（:64/280）、ExplorerPanel（:389/479）、AgentStatusView（:50/92）、HandlerForm（:181/230/250）、JsonMode（:67/118）、TerminalPanel 加载文案（:119）、App（:202）、PageDockviewHost Watermark（:66）、HooksConfigPanel（:74/83/85）、SidebarTree（:446）、FileTree（:690）、EventTree（:59）、MatcherTester（:42） |
| `FOCUS_BORDER` | `#007ACC` | 10 处：FileTree 重命名框（:455/493/537/592/632）、ActivityBar 选中指示条（:63/161）、SidebarTree（:268）、JsonMode hover（:212）、GuiMode 组框三目（:101） |
| `ACTIVE_SELECTION_BG` | `#094771` | 6 处：GuiMode:120、EventTree:85、HistorySessionList:172、SidebarTree:96、CommitFileList:130、FileTree:106 |
| `SEPARATOR_BG` | `#444` | 8 处：DiffPanel:632（分栏线）、PageDockviewHost:82/111、ErrorBoundary:99、AgentStatusView:47、CommitView:27、ExplorerPanel:386、HandlerForm:216 |
| `CONTEXT_MENU_BORDER` | `#454545` | 5 处：FileTree:83、HistorySessionList:134、SessionActionDialog:76/118、CommitFileList:107 |

**阴影 / HTML 面板色**：

| token | 色值 | 消费区域 |
|-------|------|---------|
| `SHADOW_MENU` | `rgba(0,0,0,0.5)` | SessionActionDialog 遮罩（:61） |
| `HTML_PANEL_LOADING_FG` | `#6C6C6C` | 15 处："加载中…"文案——HtmlPanel:157、DiffPanel:609、GitShowPanel:212、MatcherTester:98、JsonMode:76、HooksConfigPanel:93/118/257、GuiMode:69/422/433、EventTree:68/93/184、CommitView:94 |
| `HTML_PANEL_IFRAME_BG` | `#FFFFFF` | HtmlPanel iframe 白底（:81） |

### 1.5 侧栏配色组 `SIDEBAR_COLORS`（8 键）

消费区域：**27 处（10 文件）**，全量清单：
- **SidebarTree（12 处）**：:26/27/28 映射 `--sb-fg/--sb-hover/--sb-selected`（注意 **:25 `--sb-bg` = `PANEL_BG`**，非本对象 bg——两条独立路径，改侧栏背景需分别改动）+ :72 bg / :73 contextMenuBorder / :78 contextMenuShadow / :91 fg / :117 border / :126 border / :127 fg / :269 fg / :432 border
- **ActivityBar（3 处）**：:34 border / :139 selected / :141 hover
- **AgentStatusRow（4 处）**：:50 hover / :88 fg 标题 / :110 border 用量条轨道 / :132 fg 条件分支
- **其余 8 处**：AgentStatusView:85 treeGuide、ClaudeHistorySections:87 treeGuide、HistorySessionRow:77 fg、HistorySessionList:139 contextMenuShadow / :422 treeGuide、SessionActionDialog:78 contextMenuShadow、CommitFileList:112 contextMenuShadow、FileTree:88 contextMenuShadow

| token | 色值 | 备注 |
|-------|------|------|
| bg | `#252526` | = SIDEBAR_BG |
| fg | `#D4D4D4` | = SIDEBAR_FG |
| hover | `#2A2D2E` | = EXPLORER_COLORS.hover |
| selected | `#37373D` | = EXPLORER_COLORS.selected |
| border | `#444` | = SEPARATOR_BG |
| contextMenuBorder | `#454545` | = CONTEXT_MENU_BORDER |
| contextMenuShadow | `0 4px 12px rgba(0,0,0,0.5)` | 阴影字符串（非色值） |
| treeGuide | `#3C3C3C` | 树形引导线（agent 侧栏层级竖线） |

### 1.6 其余分组

| token | 色值 | 消费区域 |
|-------|------|---------|
| `ERROR_BANNER_BG` | `#5A1D1D` | ExplorerPanel 沙箱错误横幅（:408） |
| `ERROR_BANNER_BORDER` | `#8B0000` | 同上（:409） |
| `ERROR_BANNER_FG` | `#F48771` | 同上（:410/424） |
| `EXPLORER_SELECTION_BG` | `#094771` | FileTree 选中行（:186）、HistorySessionRow（:61）；**与 ACTIVE_SELECTION_BG 同值** |
| `AGENT_STATUS_USAGE_COLORS.low` | `#629755` | AgentStatusRow `usageBarColor`（:22-25）：<50% |
| `AGENT_STATUS_USAGE_COLORS.medium` | `#BBB529` | 50-80% |
| `AGENT_STATUS_USAGE_COLORS.high` | `#F44747` | >80%；**与 ERROR_FG 同值** |
| `ROOT_CSS_VARS["--sl-bg-primary"]` | `#1e1e2e` | main.tsx 注入 → App.css `var()` |
| `ROOT_CSS_VARS["--sl-bg-secondary"]` | `#2b2b3c` | main.tsx 注入 → **App.css 零消费**（仅 :5 注释提及，见 §3） |

## 2. `src/panels/terminal/theme.ts` — xterm 终端主题（22 项）

消费区域：所有终端面板（`useXterm` → `useTerminalInstance` → `new Terminal`）。

| 项 | 色值 | 项 | 色值 |
|----|------|----|------|
| foreground | `#D4D4D4` | brightBlack | `#666666` |
| background | `#1E1E1E` | brightRed | `#F14C4C` |
| cursor | `#D4D4D4` | brightGreen | `#23D18B` |
| cursorAccent | `#1E1E1E` | brightYellow | `#F5F543` |
| selectionBackground | `#264F78` | brightBlue | `#3B8EEA` |
| selectionForeground | `#D4D4D4` | brightMagenta | `#D670D6` |
| black | `#000000` | brightCyan | `#29B8DB` |
| red | `#CD3131` | brightWhite | `#FFFFFF` |
| green | `#0DBC79` | white | `#E5E5E5` |
| yellow | `#E5E510` | cyan | `#11A8CD` |
| blue | `#2472C8` | magenta | `#BC3FBC` |

ANSI 16 色全部覆盖 → xterm 默认调色板运行时失效（默认值仅作缺省兜底）。foreground/background 与 colors.ts 的 SIDEBAR_FG/PANEL_BG 同值但**不联动**。

## 3. `App.css` + `index.html`

| 位置 | 内容 | 说明 |
|------|------|------|
| `App.css:6` | `--sl-fg-primary: #cdd6f4` | 全局默认前景（Catppuccin 系），**不在 colors.ts** |
| `App.css:7` | `--sl-fg-secondary: #a6adc8` | 次要前景，当前无消费方 |
| `App.css:9` | `color-scheme: dark` | 非色值，驱动原生控件/滚动条暗色渲染 |
| `App.css:15/37` | `var(--sl-fg-primary)` | 全局默认文字色（:root 与 html,body,#root 的 color，被组件 token 覆盖） |
| `App.css:16/36` | `var(--sl-bg-primary)` | 全局背景（值 `#1e1e2e` 由 main.tsx 注入）——`--sl-bg-secondary` 同样注入但 App.css 零 `var()` 引用（仅 :5 注释提及），与 `--sl-fg-secondary` 同性质 |
| `index.html:10` | body `background: #1e1e2e` | 防白闪首帧，与 APP_BG_PRIMARY 同值但独立硬编码 |
| `tauri.conf.json:20` | `backgroundColor: "#1e1e2e"` | 窗口原生底色（theme: "Dark"），与 APP_BG_PRIMARY 同值但独立硬编码，防启动白闪 |

## 4. 组件硬编码（不经过 token）

| 位置 | 色值 | 性质 |
|------|------|------|
| `src/main.tsx:31` | `background:#1e1e1e;color:#f44747` | 合理例外（IPC 超时 fail-safe，React 未挂载） |
| `src/panels/hooksConfig/JsonMode.tsx:213` | `style.color = "#FFFFFF"` | **真违规**（事件导航 hover 文字变白），改色时应收敛为 token |

**`transparent` 全量分类**（合规非硬编码，`选中 ? token : "transparent"` 模式为主）：

| 类别 | 位置 | 说明 |
|------|------|------|
| 真 toggle（条件三目，8 处） | AgentStatusRow:50、HistorySessionRow:61、SidebarTree:243、ActivityBar:161、GuiMode:120、EventTree:85、HooksConfigPanel:83、FileTree:186 | 选中/激活态置 token，否则 transparent |
| 事件对（1 处） | JsonMode:212/216 | hover 置 `FOCUS_BORDER` / leave 置 transparent |
| mouseout/leave 重置回调（7 处） | SidebarTree:99/168/252、FileTree:109/195、HistorySessionList:175、CommitFileList:133 | `style.background = "transparent"` |
| 静态常量（8 处） | ActivityBar:47（iconButtonStyle）、GuiMode:79、JsonMode:90、MatcherTester:40、HandlerForm:249、ClaudeHistorySections:114（refreshButtonStyle）、SessionActionDialog:116/137 | style 定义里固定 transparent |
| 变量式开关（1 处） | ActivityBar:137 | `let bg = "transparent"`，:138-141 分支置 token |
| 非独立 transparent 字面量（2 处） | ActivityBar:49（`borderLeft: "2px solid transparent"` 占位，**带引号**字符串值）、gitGutter:55/56（CSS 三角 border 透明，位于模板字符串内） | 嵌入边框值/模板字符串，非独立字面量 |

## 5. 外部主题包

### 5.1 CodeMirror oneDark（`@codemirror/theme-one-dark` ^6.1.3）

消费区域：**4 个生产导入文件**——`useCodeMirror.ts:20`（编辑器）、`GitShowPanel.tsx:12`（gitshow）、`DiffPanel.tsx:25`（双栏 diff 共用一 import、左右各一实例）、`JsonMode.tsx:25`（JSON 模式）；+ 2 处测试 mock（gitshow-panel.test.tsx、hooks-config-jsonmode.test.tsx）。「五个 CM6 面板」= 5 个面板实例 = editor 1 + gitshow 1 + diff 左右 2 + JsonMode 1，来自 4 个生产导入文件。

**命名调色板**：

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

**背景族**：darkBackground `#21252b`（面板）、highlightBackground `#2c313a`（活动行 gutter/补全选中）、background `#282c34`（编辑器底，**= EDITOR_BG 同值不联动**）、tooltipBackground `#353a42`、selection `#3E4451`、cursor `#528bff`。

**字面量色**：`.cm-panels` 边框 `2px solid black`（拆为 `.cm-panels.cm-panels-top` borderBottom + `.cm-panels.cm-panels-bottom` borderTop 两条变体规则）、`.cm-searchMatch` 背景 `#72a1ff59`+描边 `#457dff`、`.cm-searchMatch-selected` `#6199ff2f`、`.cm-activeLine` `#6699ff0b`、`.cm-selectionMatch` `#aafe661a`、`.cm-matchingBracket` `#bad0f847`、`.cm-foldPlaceholder` 文字 `#ddd`。

### 5.2 Dockview 暗色主题（`dockview.css` `.dockview-theme-dark`）

> 变量与行号基于 `node_modules/dockview-react/dist/styles/dockview.css`（应用实际加载文件，`src/App.tsx:23`）。

消费区域：全部 Dockview 页签/布局（PageDockviewHost `className="dockview-theme-dark"`）。

| 变量 | 色值 | 说明 |
|------|------|------|
| `--dv-group-view-background-color` | `#1e1e1e` | = PANEL_BG，**不联动** |
| `--dv-tabs-and-actions-container-background-color` | `#252526` | = SIDEBAR_BG，不联动 |
| `--dv-activegroup-visiblepanel-tab-background-color` | `#1e1e1e` | 活跃页签背景 |
| `--dv-activegroup-hiddenpanel-tab-background-color` | `#2d2d2d` | = SECONDARY_BG，不联动 |
| `--dv-inactivegroup-*`（visible/hidden） | `#1e1e1e` / `#2d2d2d` | 非活跃组同值 |
| `--dv-tab-divider-color` | `#1e1e1e` | 页签分隔 |
| `--dv-separator-border` | `rgb(68, 68, 68)` | = SEPARATOR_BG，不联动 |
| `--dv-paneview-header-border-color` | `rgba(204,204,204,0.2)` | 页眉边框 |
| `--dv-activegroup-visiblepanel-tab-color` | `white` | 活跃页签文字 |
| `--dv-activegroup-hiddenpanel-tab-color` | `#969696` | 隐藏页签文字 |
| `--dv-inactivegroup-visiblepanel-tab-color` | `#8f8f8f` | 非活跃页签文字 |
| `--dv-inactivegroup-hiddenpanel-tab-color` | `#626262` | 非活跃隐藏 |
| `--dv-drag-over-background-color` | `rgba(83,89,93,0.5)` | 拖拽覆盖 |
| `--dv-icon-hover-background-color` | `rgba(90,93,94,0.31)` | 图标 hover |
| `--dv-floating-box-shadow` | `0 8px 32px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.25)` | 浮动组阴影 |
| `--dv-floating-border` | `1px solid rgba(255,255,255,0.1)` | 浮动组边框 |
| `--dv-floating-group-border` | `none` | 浮动组边框（非色值，:62） |
| `--dv-floating-group-dragging-opacity` | `0.5` | 浮动组拖拽透明度（:64） |
| `--dv-tabs-container-scrollbar-color` | `#888` | 页签滚动条 |
| `--dv-paneview-active-outline-color` | `dodgerblue` | 拖拽 outline |
| `--dv-sash-color` | `transparent` | 分隔条 |
| `--dv-active-sash-color` | `transparent` | 激活分隔条（:54） |
| `--dv-drag-over-border-color` | `transparent` | 拖拽覆盖边框（:43，被 `.dv-drop-target-selection.dv-drop-target-*` 边框规则（:2213-2222）使用）；无 `-color` 后缀的 `--dv-drag-over-border`（:63）才被 `.dv-drop-target-anchor`（:2178）与 `.dv-drop-target-selection`（:2206）消费 |
| `--dv-tab-group-line-opacity` | `0.6` | tab 组指示线透明度（tabGroup API 用，:78） |

页签分组色（仅 tabGroup API 用，本项目未用）：grey `#5f6368`、blue `#1a73e8`、red `#d93025`、yellow `#f9ab00`、green `#188038`、pink `#d01884`、purple `#a142f4`、cyan `#007b83`、orange `#e8710a`。

> `.dv-debug` 前缀硬编码色（dockview.css :2301-2316 resize handles / :2495 render overlay / :2593-2602 sash，red/green/yellow/blue/cyan/black/orange）仅调试模式生效，正常 UI 不渲染。

### 5.3 Allotment（`allotment/dist/style.css`）

| 变量 | 色值 | 用途 |
|------|------|------|
| `--separator-border` | `rgba(128,128,128,0.35)` | 三栏/上下分栏分割线 |
| `--focus-border` | `#007fd4` | 拖拽 sash hover 高亮（VS Code 系） |

## 6. 依赖默认值（未显式配置）

### 6.1 组件库层

| 项 | 依赖方 | 默认色 |
|----|--------|--------|
| 终端滚动条 | xterm | `foreground × 20% 透明度` ≈ `rgba(212,212,212,0.2)`（hover 0.4/active 0.5）；色值由**运行期算法计算**（主题未显式配置；lib/xterm.js 含 `.2/.4/.5` 透明度参数），rgba 为 foreground=#D4D4D4 下的推导，以实际渲染为准；CSS 兜底 `rgba(100,100,100,0.4)`（xterm.css:229）**作用于滚动条箭头按钮（`.xterm-scra`）**——滑块本身由 JS 内联样式 `scrollbarSliderBackground` 系列控制，改终端滚动条色两处需分别处理 |
| 终端合成窗口（IME） | xterm `.composition-view` | 背景 `#000`、文字 `#FFF` |
| 终端 viewport 衬底 | xterm `.xterm-viewport` | `#000`（被渲染器主题背景覆盖，仅滚动条衬底可见） |
| 终端 OSC 8 链接色 | xterm | `theme.link` 未配置（terminal/theme.ts 无 link 键），链接渲染色为库默认；`useXterm.ts:240` 仅设 `linkHandler.activate`（打开 URL 行为）——改色需在 `terminal/theme.ts` 显式配置 `theme.link` |
| CM 剩余默认 | oneDark 未覆盖项 | placeholder 硬编码灰 `#888`（@codemirror/view baseTheme，editor/gitshow/diff/JsonMode 四面板真实渲染色）、搜索面板输入框基础态（继承 oneDark 面板底色） |
| CM lint 诊断色系 | @codemirror/lint（不经 colors.ts） | `.cm-lintRange-error` 下划线 `#f11`、warning `orange`、info `#999`、hint `#66d`、active 背景 `#ffdd9980`、tooltip 底色 `#2e343e`/`#444`——JsonMode（:155-156 挂载 `jsonParseLinter`+`jsonSchemaLinter`）语法错误/schema 违规时真实渲染 |
| CM 搜索匹配高亮 | @codemirror/search | 默认 5 色（8 位 hex 带 alpha，light/dark 各取）：`#ffff0054`（light 匹配）/`#00ffff8a`（dark 匹配）/`#ff6a0054`（light 选中）/`#ff00ff8a`（dark 选中）/`#99ff7780`（selectionMatch）——editor/diff/gitshow 4 个生产位置（useCodeMirror.ts:296、DiffPanel.tsx:526/571、GitShowPanel.tsx:151） |
| Dockview 滚动条 | dockview | `.dockview-theme-dark` **未显式定义** `--dv-scrollbar-background-color`——通用滚动条（`.dv-scrollbar`）走全局规则 `var(..., rgba(255,255,255,0.25))` 兜底（dockview.css:29，dark 下实际生效）；`.dockview-theme-light` 显式定义 `rgba(0,0,0,0.25)`（:160）；页签滚动条由 `--dv-tabs-container-scrollbar-color: #888`（:44）决定；另浮动组拖拽透明度 0.5、tab 溢出下拉（容器 `#1e1e1e` = `--dv-group-view-background-color`（dockview.css:3060），active 行 `#1e1e1e` = `--dv-activegroup-visiblepanel-tab-background-color`（:3080-3083），inactive 行 `#2d2d2d` = `--dv-activegroup-hiddenpanel-tab-background-color`（:3084-3087）） |

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
| `src/__tests__/colors.test.ts` | **54 项精确 toBe 断言**（改动即红，STS-01）——**treeGuide 除外**（仅键数断言，改 `#3C3C3C` 测试不会红，需人工同步） |
| `test/terminal/theme-options.test.ts`（L3） | 16 色与主题色板一致（E2E-02；**:46-61 为 16 色全量副本镜像，改终端配色必须双边同步，有漂移风险**） |
| `explorer-git-status.test.tsx` | git 状态文件名着色（:324-428） |
| `git-gutter.test.ts` | GutterMarker 颜色（:115-117） |
| `commit-view-list.test.tsx` | 状态色 rgb 转换（:129-133） |
| `explorer-selection.test.tsx` | 选中高亮 `rgb(9,71,113)`（:85/:93/:144，EXPLORER_SELECTION_BG）+ hover 色 `rgb(42,45,46)`（:154/:162，HOVER_BG_RGB 常量 :13）+ hover token hex 断言（:168 `#2A2D2E`） |
| `html-panel.test.tsx` | 组件色值断言副本（:133/:145） |
| `sideBarArea.test.tsx` | 组件色值断言副本（:462） |
| `commit-context-menu-ui.test.tsx` | 组件色值断言副本（:248） |
| `explorer-crud-success.test.tsx` | 组件色值断言副本（:147/:163） |
| `main-bootstrap.test.tsx` | fail-safe `#f44747`（:35） |
| `use-xterm-integration.test.ts` | canvas mock `#000`（:22-23） |

### 8.2 token 联动校验（组件-token 脱钩才红）

| 测试文件 | 断言内容 |
|----------|---------|
| `agent-status-view.test.tsx` | 用量条分段色（:498-543，`hexToRgbStr(AGENT_STATUS_USAGE_COLORS.*)` 派生比较） |
| `file-icon.test.tsx` | GIT_FILE_COLORS 全键（:150-198）+ EXPLORER_COLORS.fg（`hexToStyleRgb` 派生比较） |
| `activityBar.test.tsx` | SIDEBAR_COLORS.selected（:179/190/311/313）、hover（:303）、FOCUS_BORDER 指示条（:180）断言（`hexToRgb` 派生比较） |
| `hooks-config-gui.test.tsx` | FOCUS_BORDER（:183/355）、INPUT_BORDER（:181/363）边框断言 + ACTIVE_SELECTION_BG（:173/186/413）选中高亮断言（`hexToRgb` 派生比较） |
| `claude-history-row.test.tsx` | EXPLORER_SELECTION_BG 选中标记（:206，`hexToRgb` 派生比较） |

> `src/__tests__/theme.test.ts`（L2）为 terminalOptions 主题结构断言，非色值守卫。
