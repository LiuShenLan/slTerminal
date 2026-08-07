# 前端颜色配置实现方式（摸底）

> 本文档回答：**当前项目中所有前端颜色配置是如何实现的**。为后续"修改整个客户端颜色配置"提供机制全貌与改色影响面分析。
> 配套文档：[`color-inventory.md`](./color-inventory.md)（全部颜色值清单，本文不重复列值）。
> 状态：临时摸底文档，改色完成后可能更新或归档。

## 1. 总览

颜色体系是**单一 TS 真值源 + 两条分发通道**架构：

```
src/theme/colors.ts（唯一 token 真值源，9 组注释块、32 个顶层命名导出）
  │
  ├─ 通道 A：named export 内联消费
  │   组件直接 import token → 内联 style / style.cssText（30 个生产文件，含测试共 39 个导入文件；引用点 **369 处**（2026-08-06 逐 token 行数求和口径实测，可重放：`rg -n '<token>' src -g '*.ts' -g '*.tsx' --glob '!**/__tests__/**' --glob '!src/theme/colors.ts' --glob '!src/theme/index.ts' | rg -v ':import ' | wc -l` 对 32 token 求和，同一行多 token 各计各的；行数口径跨行 import 块仍漏计，仅示数量级）——**非精确值，改色时以 grep 为准**）
  │
  ├─ 通道 B：ROOT_CSS_VARS 注入 CSS 变量
  │   main.tsx 启动时 setProperty 到 document.documentElement
  │   → App.css 经 var(--sl-bg-primary) 引用（--sl-bg-secondary 同样注入但零消费，仅 App.css :5 注释提及）
  │
  └─ 通道 C：SidebarTree 内部桥接（二级变量）
      PANEL_BG + SIDEBAR_COLORS.fg/hover/selected → 内联 CSS 变量 --sb-* → var() 引用
```

主题层（**不经过 colors.ts** 的独立色源，改色漏网点）：

```
xterm 终端主题（src/panels/terminal/theme.ts，唯一"既定例外"）→ 22 色
CodeMirror oneDark（@codemirror/theme-one-dark，4 个生产导入点 + 2 处测试 mock）→ 语法高亮全色板
Dockview 内置暗色主题类（dockview-theme-dark，库 CSS 变量）→ 页签/布局色
Allotment 库 CSS 默认变量 → 分栏分割线
App.css :root 硬编码 hex（--sl-fg-*，不在 colors.ts）
index.html / main.tsx / tauri.conf.json 硬编码
```

## 2. token 层：`src/theme/colors.ts`

文件头注释声明架构约束 #6："组件引用 token，禁止硬编码颜色"；配色以 JetBrains IDEA 暗色主题（Darcula）为基准（`--sl-fg-*` 为 Catppuccin 系例外，见 §6 #4）。

**9 组注释块**（7 个 `// --- 组名 ---` 分节头 + 2 处普通注释；计数口径：**2 处普通注释 = 侧栏配色组（L76）+ 错误提示色（L89）**——通用 UI 色组内另有 5 处子分节注释（背景/前景/交互/阴影/HTML 面板，L44-72）、文件头注释（L1-4）、treeGuide JSDoc（L85）不计入；顶层 `export` 计 1，对象内键不计入，共 **32 个顶层命名导出**）：

| 分组 | 导出形式 | 内容 |
|------|----------|------|
| 文件名 git 状态色 | 对象 `GIT_FILE_COLORS` | 7 键（modified/added/untracked/deleted/renamed/conflict/ignored） |
| 行内 diff 边栏色 | 对象 `GIT_GUTTER_COLORS` | 4 键（modified/added/deleted/whitespaceOnly） |
| 文件浏览器通用色 | 对象 `EXPLORER_COLORS` | 6 键（bg/fg/hover/selected/arrowClosed/arrowOpen） |
| 通用 UI 色 | 22 个标量导出 | 组内按"背景/前景文字/交互控件/阴影/HTML 面板"分节（推导：26 标量 = 22 通用 UI + 3 错误提示 + 1 选中高亮，加 6 对象 = 32） |
| 侧栏配色组（普通注释） | 对象 `SIDEBAR_COLORS` | 8 键（含 JSDoc 注释的 treeGuide） |
| 错误提示色（普通注释） | 3 个独立导出 | ERROR_BANNER_BG/BORDER/FG |
| Explorer 选中高亮 | 独立导出 | EXPLORER_SELECTION_BG（注释：VS Code list activeSelectionBackground） |
| Agent Status 用量条 | 对象 `AGENT_STATUS_USAGE_COLORS` | 3 键（low/medium/high，阈值由组件逻辑决定 <50/50-80/>80） |
| CSS 变量桥接 | 对象 `ROOT_CSS_VARS` | 2 键（--sl-bg-primary/--sl-bg-secondary） |

**导出与命名规则**：
- 全部 `export const` 命名导出；所有对象带 `as const`（字面量类型）
- 经 `src/theme/index.ts` barrel 统一 re-export（32 个名字）
- 命名 = 语义域大写前缀 + 用途（`PANEL_BG`/`SIDEBAR_FG`/`FOCUS_BORDER`）；多值域用对象 `*_COLORS` + 小写键（`EXPLORER_COLORS.fg`）

## 3. 消费机制

**TS token + 内联 style，不用 CSS 变量**（组件层）。两种 import 路径混用：

- barrel：`import { PANEL_BG, APP_BG } from "./theme"`（大多数组件，`src/App.tsx:22` 等）
- 直连（**4 处**）：`SideBarArea.tsx:12`、`ActivityBar.tsx:24`、`AgentStatusRow.tsx:12`、`gitGutter.ts:15`——均 `from "../../theme/colors"`；其余 26 个生产文件走 barrel `theme`

应用方式三种：

| 方式 | 示例 |
|------|------|
| 内联 style 对象 | `style={{ background: PANEL_BG }}`（`SideBarArea.tsx:69`） |
| style.cssText 字符串 | `gitGutter.ts:25` 注入 GutterMarker DOM（gutter 色） |
| 内联 CSS 变量赋值 | `SidebarTree.tsx:25-28` 把 `PANEL_BG`（`--sb-bg`）+ `SIDEBAR_COLORS.fg/hover/selected`（`--sb-fg/--sb-hover/--sb-selected`）映射为内联 CSS 变量，组件内 `var()` 引用 |

`var(--sl-*)` 只在 `App.css` 出现（4 处：L15/16/36/37），组件层不用——`ROOT_CSS_VARS` 桥接由 `main.tsx:38-40` 在 React 挂载前执行（注释："替代 App.css :root 硬编码 hex"）。

无 styled-components/emotion/tailwind（package.json 确认），只有原生 CSS + 内联样式。

## 4. 主题层

### 4.1 xterm 终端主题（唯一"既定例外"）

`src/panels/terminal/theme.ts` 导出 `terminalOptions: ITerminalOptions`，文件头注释："配色单点：所有终端颜色在此定义，组件引用此 token（硬约束 #6）"。22 项颜色（foreground/background/cursor/cursorAccent/selectionBackground/selectionForeground + ANSI 16 色）+ 非色选项（fontSize/fontFamily/cursorBlink/cursorStyle/scrollback/drawBoldTextInBrightColors/vtExtensions/allowProposedApi）。

**"既定例外"的规范出处**（三处，措辞不同）：
- `src/theme/CLAUDE.md:7`："配色源自 JetBrains IDEA 暗色主题（Darcula）。**既定例外**：xterm.js 终端主题在 `src/panels/terminal/theme.ts` 独立定义（历史遗留）"
- `src/theme/CLAUDE.md`「新增 token 规则」："终端主题例外仅限 `panels/terminal/theme.ts`——新面板的终端类渲染器配色照此登记例外，不扩默认"
- `src/panels/CLAUDE.md` 硬约束 #6："终端配色是历史遗留的独立主题定义"

**消费链**：`theme.ts` → `useXterm.ts:133`（**消费调用点**——`useTerminalInstance(container, terminalOptions, fontSize)`；theme 的 import 在 :23）→ `useTerminalInstance.ts:127-132` `new Terminal({ ...options, cols: DEFAULT_COLS, rows: DEFAULT_ROWS, fontSize: fontSize ?? DEFAULT_FONT_SIZE })`（展开进 xterm 构造——cols/rows 取模块常量，fontSize 有 `?? DEFAULT_FONT_SIZE` 兜底）。

### 4.2 CodeMirror oneDark

`@codemirror/theme-one-dark` ^6.1.3，导入点：`useCodeMirror.ts`、`GitShowPanel.tsx`、`DiffPanel.tsx`、`JsonMode.tsx`。直接导入第三方主题包，**未走 colors.ts**。语法高亮色 + 编辑器背景/选区/tooltip 全部由 oneDark 决定。

### 4.3 Dockview 内置暗色主题

`PageDockviewHost.tsx:369` `className="dockview-theme-dark"`——无自定义 theme 对象，直接用库内置暗色变量（`dockview.css` 的 `--dv-*`）。**关键**：dockview 变量的 `#1e1e1e/#252526/#2d2d2d/#444` 与 colors.ts token 数值一致但**不经过 token**（"碰巧同值"），换 token 不会联动（详见影响面 §8.4）。

> **生效路径**：应用实际加载 `dockview-react/dist/styles/dockview.css`（`src/App.tsx:23` import），与 `dockview/dist/styles/dockview.css` 同源副本——审计/改色须注意生效文件为 dockview-react 包的副本。

## 5. 暗色六层防线

| 层 | 位置 | 机制 |
|----|------|------|
| 1. Tauri 窗口 | `tauri.conf.json:19-20` | `theme: "Dark"` + `backgroundColor: "#1e1e2e"`（原生窗口底色，防启动白闪） |
| 2. HTML 首帧 | `index.html:10` | body 内联 `background: #1e1e2e`（React 挂载前） |
| 3. CSS 基础 | `App.css:9` | `color-scheme: dark`（驱动原生控件/滚动条暗色渲染，**唯一一处**） |
| 4. 运行时注入 | `main.tsx:38-40` | `ROOT_CSS_VARS` 写入 `document.documentElement` |
| 5. 组件 token | 全部组件 | 内联 style 暗色 token |
| 6. 三方库主题 | xterm/CM/Dockview | 各自暗色主题 |

> 改色时若改暗色基调，**前 4 层 + 主题层都要同步**——只有第 5 层走 token。

## 6. 未登记色值来源（不经过 colors.ts，共 4 处）

| # | 位置 | 内容 | 说明 |
|---|------|------|------|
| 1 | `node_modules/@codemirror/theme-one-dark` | 语法高亮全色板 | 第三方主题，见清单文档 §5.1 |
| 2 | `dockview.css` 的 `.dockview-theme-dark` | `--dv-*` 变量 | 与 token 同值但独立 |
| 3 | `index.html:10` + `main.tsx:31` | `#1e1e2e` / `#1e1e1e`+`#f44747` | 启动链 fail-safe，合理例外（React 未挂载无法用 token） |
| 4 | `App.css:6-7` | `--sl-fg-primary: #cdd6f4`、`--sl-fg-secondary: #a6adc8` | **唯一存在于 CSS 侧、不经 colors.ts 的色值**（Catppuccin 系，与 Darcula 体系风格不同源）。消费情况：`--sl-fg-primary` 为全局默认文字色（App.css:15/37 的 color），被组件 token 覆盖；`--sl-fg-secondary` 无消费方 |

## 7. 默认色机制（未显式配置处）

- **xterm 滚动条**：`scrollbarSliderBackground = foreground 20% 透明度`（**运行期算法计算**，主题未显式配置——lib/xterm.js 含 `.2/.4/.5` 透明度参数，参数源自 xterm 滚动条实现：foreground 派生 + 状态透明度渐变；foreground=#D4D4D4 → ≈ `rgba(212,212,212,0.2)`，hover 0.4 / active 0.5，以实际渲染为准）；xterm.css:229 CSS 变量兜底 `rgba(100,100,100,0.4)`
- **xterm 合成窗口**：`.composition-view` 背景 `#000`/文字 `#FFF`（IME 输入态）
- **xterm viewport 衬底**：`.xterm:not(.allow-transparency) .xterm-viewport` 背景 `#000`（xterm.css:103）——**选择器带 `:not(.allow-transparency)` 条件**，启用透明度则无此衬底；被渲染器主题背景覆盖，仅滚动条衬底可见
- **CodeMirror 剩余默认**：oneDark 已覆盖本体/tooltip/search panel/gutter/选区；placeholder 硬编码灰 `#888`（@codemirror/view baseTheme，editor/gitshow/diff/JsonMode 四面板真实渲染色）。另有两族真实渲染色不经 colors.ts：① **CM lint 诊断色系**（@codemirror/lint）：`.cm-lintRange-error` 下划线 `#f11`、warning `orange`、info `#999`、hint `#66d`、active 背景 `#ffdd9980`、tooltip 底色 `#2e343e`/`#444`——生效位置 JsonMode（:155-156 挂载 `jsonParseLinter`+`jsonSchemaLinter`）；② **highlightSelectionMatches 默认 5 色**（@codemirror/search）：`#ffff0054`（light 匹配）/`#00ffff8a`（dark 匹配）/`#ff6a0054`（light 选中）/`#ff00ff8a`（dark 选中）/`#99ff7780`（selectionMatch）——editor/diff/gitshow 4 个生产位置（useCodeMirror.ts:296、DiffPanel.tsx:526/571、GitShowPanel.tsx:151）
- **Dockview 滚动条**：`.dockview-theme-dark` 未显式定义 `--dv-scrollbar-background-color`——通用滚动条走全局规则 `var(..., rgba(255,255,255,0.25))` 兜底（dockview.css:29，dark 下实际生效）；light 主题显式 `rgba(0,0,0,0.25)`（:160）；页签滚动条 `--dv-tabs-container-scrollbar-color: #888`（:44）；另浮动组拖拽透明度 0.5
- **Allotment**：`--separator-border: rgba(128,128,128,0.35)`（分栏分割线）、`--focus-border: #007fd4`（拖拽 sash 高亮）
- **浏览器/WebView2 层**：原生滚动条、`::selection`、input focus ring、placeholder、右键菜单、表单控件——全部跟随 `color-scheme: dark`（App.css:9）+ 窗口 `theme: "Dark"` 渲染为暗色系，无显式定制

## 8. 改色影响面（核心）

### 8.1 消费文件清单（按区域）

> 清单共 36 项 = **30 项真实 colors 消费文件 + 6 项配色相关非消费方（标 `*`）**。`*` = 不 import colors token（硬编码定义方 / 主题定义方 / 第三方主题 / 启动链）。

| 区域 | 文件 |
|------|------|
| 全局 | `src/main.tsx`（ROOT_CSS_VARS 注入 + fail-safe）、`src/App.tsx`、`src/lib/ErrorBoundary.tsx`、`src/App.css`*（仅 `var(--sl-*)`，`--sl-fg-*` hex 定义方）、`index.html`*（启动链硬编码 `#1e1e2e`） |
| 活动栏/侧栏 | `features/sideViews/ActivityBar.tsx`、`SideBarArea.tsx`、`features/sidebar/SidebarTree.tsx` |
| 文件浏览器 | `features/explorer/ExplorerPanel.tsx`（含 ERROR_BANNER 横幅）、`FileTree.tsx`、`FileIcon.tsx` |
| Commit 视图 | `features/commit/CommitView.tsx`、`CommitFileList.tsx` |
| Agent/历史 | `features/agentStatus/AgentStatusView.tsx`、`AgentStatusRow.tsx`、`features/claudeHistory/ClaudeHistorySections.tsx`、`HistorySessionList.tsx`、`HistorySessionRow.tsx`、`SessionActionDialog.tsx` |
| 终端 | `panels/terminal/TerminalPanel.tsx`、`terminal/theme.ts`*（主题定义方，未走 colors.ts）、`terminal/useXterm.ts`*（import `./theme` 终端主题，非 colors）、`terminal/index.ts`*（re-export terminalOptions，经 theme.ts 间接） |
| 编辑器 | `panels/editor/EditorPanel.tsx`、`gitGutter.ts`、`useCodeMirror.ts`*（仅 `@codemirror/theme-one-dark`） |
| 面板 | `panels/html/HtmlPanel.tsx`、`gitshow/GitShowPanel.tsx`、`diff/DiffPanel.tsx` |
| hooks 配置 | `panels/hooksConfig/HooksConfigPanel.tsx`、`JsonMode.tsx`、`GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`、`MatcherTester.tsx` |
| 工作区 | `workspace/PageDockviewHost.tsx`（dockview 主题类 + Watermark） |

### 8.2 颜色相关测试守卫（改 token 必须同步）

> 完整测试守卫清单（17 文件 × 断言内容，按「改色即红 / token 联动」两分类）见 `color-inventory.md` §8——**唯一真值源，改色以 inventory 为准**。本节仅保留 inventory 未覆盖的独有注记：

- **CLAUDE.md 过时**：`src/theme/CLAUDE.md` 声称"无独立测试文件——token 正确性由消费方测试断言"，**已过时**——`colors.test.ts` 自 2026-06 起存在（创建于 2026-06-26 commit `5bdefdf`，最后修改 2026-08-05 commit `bc9ce8f`）且是全 token 断言。改色时以 `colors.test.ts` 为准
- **注释过时**：`colors.test.ts:7` 文件头注释仍写「通用 UI 色 20 个独立 token」，已过时（实际 25 项，:175 断言 `toHaveLength(25)`）——改色时顺带修正注释

### 8.3 漏网点（改色时需人工同步，无守卫）

| 位置 | 内容 | 性质 |
|------|------|------|
| `src/panels/hooksConfig/JsonMode.tsx:213` | `style.color = "#FFFFFF"`（事件导航 hover） | **真违规**，应落 token |
| `src/main.tsx:31` | `background:#1e1e1e;color:#f44747` | 合理例外（React 未挂载） |
| `src/App.css:6-7` | `--sl-fg-primary/secondary` hex | 定义源在 CSS，不在 token |
| `index.html:10` | body `#1e1e2e` | 启动链，与 token 同值但独立 |
| `src-tauri/tauri.conf.json:20` | `backgroundColor: "#1e1e2e"` | 窗口层，与 token 同值但独立 |

### 8.4 数值重复但不联动（改一处不会带动另一处）

> 色值统一大写表示（各来源实际大小写见括号注明——`#1e1e2e` 各来源均为小写，无分歧）。

| 色值 | 出现位置 |
|------|---------|
| `#1e1e2e` | colors.ts（APP_BG/APP_BG_PRIMARY）/ index.html body / tauri.conf.json backgroundColor（三处均小写） |
| `#1E1E1E` | colors.ts（PANEL_BG/EXPLORER_COLORS.bg，大写）/ xterm theme background（大写）/ dockview.css `--dv-group-view-background-color`（`#1e1e1e` 小写）/ main.tsx:31（`#1e1e1e` 小写，fail-safe 背景） |
| `#252526`、`#2D2D2D` | colors.ts（SIDEBAR_BG/SECONDARY_BG，大写）/ dockview.css（tabs 容器/inactive tab 背景，`#2d2d2d` 小写） |
| `#282C34` | colors.ts（EDITOR_BG，大写）/ oneDark background（`#282c34` 小写） |
| `#D4D4D4` | colors.ts（SIDEBAR_FG/EXPLORER_COLORS.fg/arrowOpen）/ xterm theme foreground（各来源均大写，无分歧） |

### 8.5 死 token

- **`DROPDOWN_BG`**（`colors.ts:48`）：仅定义 + re-export + `colors.test.ts` 断言，生产代码零消费——改色时可直接删除或复用
- **`GIT_GUTTER_COLORS.whitespaceOnly`**：对象内死键——`gitGutter.ts` 仅消费 added/modified/deleted，生产代码零消费（`colors.test.ts:80` 有断言引用；inventory §1.2 已标）
- **`EXPLORER_COLORS.selected`**：对象内死键——生产代码零消费（`colors.test.ts:100` 有断言引用）；文件树选中用 `EXPLORER_SELECTION_BG`、侧栏选中用 `SIDEBAR_COLORS.selected`（inventory §1.3 已标）
- `ERROR_BANNER_*` **不是**死 token：`ExplorerPanel.tsx:408-424` 沙箱错误横幅消费（背景/边框/文字）

### 8.6 内容层（不可配置，改色无需处理）

- emoji 图标（文件图标映射、F3 四态 ⚡🟡✅❌）：颜色由系统 emoji 字体决定
- 终端输出的 ANSI 内容色（claude/pwsh 输出）：渲染为 xterm 主题 16 色，但色值来源是子进程内容，非 UI 配置
- shell 集成脚本（`src-tauri/assets/shell-integration.ps1`）：**不含 ANSI 色序列**（仅 OSC 7/9;9/133 控制序列），无需处理
- `public/tauri.svg`（#FFC131/#24C8DB）：**孤儿文件**（从未被任何代码引用，改色可忽略）——但 Vite 会将 `public/` 原样拷贝进 `dist/`，文件实际随打包产物发布（彻底清理孤儿资产时可删除）；应用图标实际来自 `src-tauri/icons/`（PNG/ICO 二进制）；`index.html:5` favicon 指向不存在的 `/vite.svg`（404，与颜色无关）

## 9. 相关文档

- 颜色值全表（含默认色）：[`color-inventory.md`](./color-inventory.md)
- 配色单点约束：#6，见 `.claude/CLAUDE.md` 硬性开发约束
- 终端主题例外：`src/panels/terminal/CLAUDE.md`、`src/theme/CLAUDE.md`
