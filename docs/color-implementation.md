# 前端颜色配置实现方式

> 本文档回答：**项目中所有前端颜色配置是如何实现的**。为修改客户端颜色配置提供机制全貌与改色影响面分析。
> 配套文档：[`color-inventory.md`](./color-inventory.md)（全部颜色值清单，本文不重复列值）。
> 状态：长期参考文档。2026-08 配色系统重构为方案系统（schemes/ 值文件 + SchemeRegistry + colors.ts facade + overrides 四通道）后本文更新为新架构现状；设计与决策见 [`color-scheme-refactor-spec.md`](./color-scheme-refactor-spec.md) 与 `.claude/adr.md` ADR-0002。

## 1. 总览

配色体系 = **方案值文件 + 注册表 + facade 代理 + 五条生效通道**：

```
src/theme/schemes/darcula.ts（值文件：ColorScheme 四段 ui/terminal/editor/libraries）
  │ 经 src/theme/schemes/index.ts side-effect 注册进 schemeRegistry（照 tabRules.ts 模式）
  ▼
schemeRegistry（模块级单例：register/get/getAll/getActive/setActive；未知 id 回退 darcula + console.warn）
  ├─ 通道 1（React 组件）：colors.ts facade —— ui 段逐 token 代理导出（31 个）
  │    → 组件内联 style / style.cssText（30 个生产文件、369 处引用——2026-08-06 摸底口径，改色以 grep 为准）
  │    + ROOT_CSS_VARS（--sl-bg-primary / --sl-fg-primary）→ main.tsx 注入 documentElement → App.css var() 引用
  ├─ 通道 2（终端）：panels/terminal/theme.ts adapter —— terminal 段 25 键展开进 xterm ITheme
  ├─ 通道 3（编辑器）：overrides.ts editorTheme + editorColorOverrides() —— editor 段 → CM6（4 个消费点）
  ├─ 通道 4（dockview）：overrides.ts dockviewVarStyle() —— libraries.dockview 20 条 --dv-* 内联注入 PageDockviewHost
  └─ 通道 5（allotment）：overrides.ts allotmentVarStyle() —— libraries.allotment 2 键内联注入 Workspace 根容器
```

方案系统之外仅存**启动链 fail-safe 硬编码**（`index.html:10` / `tauri.conf.json:20` / `main.tsx` 超时页——React 挂载前的静态层无法用 TS token，属合理例外）。darcula.ts 文件头交叉引用，改 ui 对应值时手动同步。

**方案切换**：`~/.slterminal/settings.json` 写 `colorScheme: "<id>"` → 重载窗口生效。main.tsx 启动序列先注册内置方案 + `setActive`，再 import facade/overrides（求值时机保证见 §4.5）。

## 2. 方案系统：`src/theme/`

| 文件 | 职责 |
|------|------|
| `schemes/types.ts` | `ColorScheme` 接口（四段）+ `UiTokens`/`TerminalPalette`/`EditorScheme`/`LibraryOverrides` 槽位定义；**消费注释单点**（决策 D8——消费位置注释只在槽位 JSDoc，值文件零注释负担） |
| `schemes/darcula.ts` | 内置默认方案值文件（id `"darcula"`，即 settings.json `colorScheme` 段缺省取值）。值 = 重构前现状搬运（D1 零视觉变化）：ui 段 = 原 colors.ts 值 + App.css `--sl-fg-primary` 收编（`appFg`）+ JsonMode 硬编码收编（`onAccentFg`）；libraries 与 ui 同值条目引用 ui 槽位构造（值单点） |
| `schemes/index.ts` | side-effect 注册文件：import 时向 `schemeRegistry.register(darcula)`。新增方案在此追加 import + register 一行 |
| `schemeRegistry.ts` | `SchemeRegistry` 模块级单例（项目注册表惯例第 6 例）：`register`（同 id 覆盖）/`get`/`getAll`/`getActive`（activeId 异常时回退 darcula）/`setActive`（未知 id → `console.warn` + 回退 darcula）/`getDefaultId`/`_reset`（仅测试） |
| `colors.ts` | **facade，不定义任何颜色值**——import 时取 `schemeRegistry.getActive().ui` 逐 token 代理导出（31 个）；组件只引用本文件 token（硬约束 #6） |
| `overrides.ts` | 组件库配色注入四导出：`dockviewVarStyle()`（20 条 `--dv-*`）/`allotmentVarStyle()`（2 键）/`editorTheme`（模块级常量 = active 方案 editor.theme）/`editorColorOverrides()`（CM6 `EditorView.theme` 扩展） |
| `index.ts` | barrel：31 个 facade 导出 + schemeRegistry + darcula + 类型 + overrides 四导出 |

### 2.1 方案接口（schemes/types.ts）

`ColorScheme` 四段：

| 段 | 槽位 | 生效通道 |
|----|------|---------|
| `ui` | gitFile 7 键 / gitGutter 3 键 / explorer 5 键 / sidebar 8 键 / errorBanner 3 键 / agentStatusUsage 3 键 + **23 个标量**（panelBg…explorerSelectionBg） | 通道 1（colors.ts facade → 组件） |
| `terminal` | 25 键（6 基础 + 3 滚动条 + ANSI 16 色） | 通道 2（terminal adapter → xterm ITheme） |
| `editor` | `theme`（CM 基础主题引用）+ `overrides`（background / lint 7 键 / searchMatch 4 键） | 通道 3（overrides → CM6） |
| `libraries` | `dockview` 20 条 + `allotment` 2 键 | 通道 4/5（overrides → 内联 CSS 变量） |

**死配置清理结果**（spec §9.2 回写，Stage 02 实施）：原 colors.ts 的 `DROPDOWN_BG`（零消费）、`APP_BG_SECONDARY`（零消费，连带 CSS 变量 `--sl-bg-secondary`）、`GIT_GUTTER_COLORS.whitespaceOnly`（对象内死键）、`EXPLORER_COLORS.selected`（对象内死键）已删除；`App.css` 的 `--sl-fg-secondary`（零消费）已删除。新增 `ON_ACCENT_FG`（`ui.onAccentFg`，收编 JsonMode 事件导航 hover 硬编码 `#FFFFFF`）。清理后 32 导出 → 31 导出。

### 2.2 命名与导出规则

- facade 全部 `export const` 命名导出；对象带 `as const`；经 `src/theme/index.ts` barrel 统一 re-export（31 个名字）
- 命名 = 语义域大写前缀 + 用途（`PANEL_BG`/`SIDEBAR_FG`/`FOCUS_BORDER`）；多值域用对象 `*_COLORS` + 小写键（`EXPLORER_COLORS.fg`）
- 新增 token 槽位 = types.ts 加字段 → darcula.ts 填值 → colors.ts facade 加导出 → 消费点/测试同步（硬约束 #6）

## 3. 消费机制

**TS token + 内联 style，不用 CSS 变量**（组件层）。组件经 facade 引用，两种 import 路径混用：

- barrel：`import { PANEL_BG, APP_BG } from "./theme"`（大多数组件，`src/App.tsx` 等）
- 直连：`SideBarArea.tsx:12`、`ActivityBar.tsx:24`、`AgentStatusRow.tsx:12`、`gitGutter.ts:15`——均 `from "../../theme/colors"`；其余 26 个生产文件走 barrel `theme`

应用方式三种：

| 方式 | 示例 |
|------|------|
| 内联 style 对象 | `style={{ background: PANEL_BG }}`（`SideBarArea.tsx`） |
| style.cssText 字符串 | `gitGutter.ts` 注入 GutterMarker DOM（gutter 色） |
| 内联 CSS 变量赋值 | `SidebarTree.tsx` 把 `PANEL_BG`（`--sb-bg`）+ `SIDEBAR_COLORS.fg/hover/selected`（`--sb-fg/--sb-hover/--sb-selected`）映射为内联 CSS 变量，组件内 `var()` 引用 |

`var(--sl-*)` 只在 `App.css` 出现（:root 与 html,body,#root 的 `color: var(--sl-fg-primary)` / `background: var(--sl-bg-primary)`，共 4 处），组件层不用——`ROOT_CSS_VARS` 桥接由 `main.tsx` 在 React 挂载前注入（两键均被消费，注释见 `App.css:5`）。

无 styled-components/emotion/tailwind（package.json 确认），只有原生 CSS + 内联样式。

## 4. 四条组件库通道

### 4.1 终端：terminal 段 → xterm ITheme（adapter）

`src/panels/terminal/theme.ts` 是 **adapter 而非独立主题定义**：`terminalOptions.theme = { ...schemeRegistry.getActive().terminal }`（25 键展开进 xterm ITheme）。非色选项（fontSize/fontFamily/cursorBlink/cursorStyle/scrollback/drawBoldTextInBrightColors/vtExtensions/allowProposedApi）原位保留。

- 消费链：`theme.ts` → `useXterm.ts`（import `./theme`）→ `useTerminalInstance.ts` `new Terminal({ ...options, cols: DEFAULT_COLS, rows: DEFAULT_ROWS, fontSize: fontSize ?? DEFAULT_FONT_SIZE })`
- 25 键 = 6 基础（foreground/background/cursor/cursorAccent/selectionBackground/selectionForeground）+ **3 滚动条键**（scrollbarSliderBackground/Hover/Active——原运行期算法派生值显式化，spec §9.2 勘误 2）+ ANSI 16 色
- OSC 8 链接色**不可配置**：xterm 6.x `ITheme` 无 `link` 键（spec §9.2 勘误 1）

### 4.2 编辑器：editor 段 → CM6（overrides）

4 个 oneDark 导入点全部替换为 `editorTheme` + `editorColorOverrides()`（`overrides.ts`）：

- `useCodeMirror.ts:289-290`（编辑器）、`GitShowPanel.tsx:142-143`（gitshow）、`JsonMode.tsx:162-163`（JSON 模式）、`DiffPanel.tsx:520-521/566-567`（双栏 diff 左右各一）
- `editorTheme` = active 方案 `editor.theme`（darcula 为 `@codemirror/theme-one-dark` 直 import 透出，决策 D6）——语法高亮全色板仍由 oneDark 包提供
- `editorColorOverrides()` = active 方案 `editor.overrides` → `EditorView.theme(..., { dark: true })`：background（对齐 ui.editorBg）+ lint 7 键（JsonMode 波浪线/tooltip）+ searchMatch 4 键（editor/diff/gitshow 搜索高亮）

### 4.3 dockview：libraries.dockview → 内联 CSS 变量

`PageDockviewHost.tsx:368` 根 div style 展开 `dockviewVarStyle()`（20 条 `--dv-*` 变量内联注入，值来自 active 方案 `libraries.dockview`），`className="dockview-theme-dark"` 保留（布局样式仍由 dockview.css 提供）。与 ui 同值的条目在 darcula.ts 引用 ui 槽位构造（值单点）。

> **生效文件**：应用实际加载 `dockview-react/dist/styles/dockview.css`（`src/App.tsx` import），与 `dockview/dist/styles/dockview.css` 同源副本。20 条变量内联注入挂载点，样式表加载顺序免疫。

### 4.4 allotment：libraries.allotment → 内联 CSS 变量

`Workspace.tsx:228` 根容器 style 合并 `allotmentVarStyle()`（2 键：`--separator-border` / `--focus-border`）——CSS 变量继承同时覆盖外层 Allotment 与 `SideBarArea.tsx` 内层 Allotment 两处。

### 4.5 方案切换与启动序列（main.tsx）

方案切换 = 手编 `~/.slterminal/settings.json` 的 `colorScheme` 段 + 重载窗口生效（无运行期热切换——token 常量消费架构下热切换代价不成比例，决策 D2）。main.tsx 启动序列（React 挂载前）：

```
① import "./ipc/settings"（动态）→ loadSettings() 读 colorScheme
② import "./theme/schemeRegistry" + await import("./theme/schemes")（side-effect 注册 darcula）
③ schemeRegistry.setActive(schemeId)（settings 缺省/未知 → "darcula"）
④ await import("./theme") → ROOT_CSS_VARS 注入 documentElement（替代 App.css :root 硬编码 hex）
⑤ E2E helpers（DEV/VITE_E2E 门控）
⑥ await import("./App")
```

求值时机保证：colors.ts facade / overrides.ts 的模块级取值（`editorTheme`、ROOT_CSS_VARS）**首次求值发生在 setActive 之后**；测试环境无启动序列，registry 默认 active = darcula，直接 import 即得正确值。

## 5. 暗色六层防线

| 层 | 位置 | 机制 |
|----|------|------|
| 1. Tauri 窗口 | `tauri.conf.json:19-20` | `theme: "Dark"` + `backgroundColor: "#1e1e2e"`（原生窗口底色，防启动白闪；启动链 fail-safe） |
| 2. HTML 首帧 | `index.html:10` | body 内联 `background: #1e1e2e`（React 挂载前；启动链 fail-safe） |
| 3. CSS 基础 | `App.css:9` | `color-scheme: dark`（驱动原生控件/滚动条暗色渲染，**唯一一处**） |
| 4. 运行时注入 | `main.tsx` | `ROOT_CSS_VARS` 写入 `document.documentElement`（`--sl-bg-primary`/`--sl-fg-primary` 来自 active 方案 ui 段） |
| 5. 组件 token | 全部组件 | 内联 style 暗色 token（经 colors.ts facade） |
| 6. 三方库主题 | xterm/CM/Dockview/Allotment | 四通道经方案系统映射（§4） |

> 改色时若改暗色基调，**前 4 层 + 方案值文件都要同步**——第 5/6 层走方案系统自动联动，第 1/2 层需手动同步 darcula.ts 交叉引用列表。

## 6. 方案系统之外的硬编码色源（共 3 处启动链）

| # | 位置 | 内容 | 说明 |
|---|------|------|------|
| 1 | `index.html:10` | body `#1e1e2e` | 启动链 fail-safe，与 ui.appBgPrimary 同值但独立硬编码 |
| 2 | `src/main.tsx` 超时页 | `#1e1e1e` + `#f44747` | IPC 超时 fail-safe，React 未挂载 |
| 3 | `src-tauri/tauri.conf.json:20` | `backgroundColor: "#1e1e2e"` | 窗口原生层 |

> 重构前的不经 colors.ts 色源已收编去向：oneDark 语法色板 → editor.theme 引用（§4.2）；dockview 变量 → libraries.dockview（§4.3）；`App.css` `--sl-fg-primary` → ui.appFg（§2）；`--sl-fg-secondary`/`--sl-bg-secondary` → 死配置删除。

## 7. 默认色机制（未显式配置处）

- **xterm 滚动条**：滑块三键已显式配置（terminal 段 `scrollbarSliderBackground/Hover/Active` = `rgba(212,212,212,0.2/0.4/0.5)`，显式化原运行期算法派生值——spec §9.2 勘误 2）；xterm.css:229 CSS 变量兜底 `rgba(100,100,100,0.4)` 仍作用于滚动条**箭头按钮**（`.xterm-scra`）
- **xterm 合成窗口**：`.composition-view` 背景 `#000`/文字 `#FFF`（IME 输入态，库内部默认）
- **xterm viewport 衬底**：`.xterm:not(.allow-transparency) .xterm-viewport` 背景 `#000`（xterm.css:103）——选择器带 `:not(.allow-transparency)` 条件，启用透明度则无此衬底
- **CodeMirror 剩余默认**：oneDark 覆盖本体/tooltip/search panel/gutter/选区；placeholder 硬编码灰 `#888`（@codemirror/view baseTheme，editor/gitshow/diff/JsonMode 四面板真实渲染色）。lint 诊断色系与搜索匹配高亮 5 色**已收编**进 editor.overrides（§4.2）
- **Dockview 滚动条**：通用滚动条兜底规则 `var(..., rgba(255,255,255,0.25))`（dockview.css:29）已被 `libraries.dockview["--dv-scrollbar-background-color"]` 显式覆盖；页签滚动条 `--dv-tabs-container-scrollbar-color: #888888` 亦已收编；浮动组拖拽透明度 0.5 非色值不入方案
- **Allotment**：`--separator-border` / `--focus-border` 两色经 libraries.allotment 收编（§4.4）
- **浏览器/WebView2 层**：原生滚动条、`::selection`、input focus ring、placeholder、右键菜单、表单控件——全部跟随 `color-scheme: dark`（App.css:9）+ 窗口 `theme: "Dark"` 渲染为暗色系，无显式定制

## 8. 改色影响面（核心）

### 8.1 消费文件清单（按区域）

> 清单共 36 项 = **30 项真实 colors 消费文件 + 6 项配色相关非消费方（标 `*`）**。`*` = 不 import colors token（启动链硬编码 / 方案定义方 / adapter / 主题包）。

| 区域 | 文件 |
|------|------|
| 全局 | `src/main.tsx`（启动序列 + ROOT_CSS_VARS 注入 + fail-safe）、`src/App.tsx`、`src/lib/ErrorBoundary.tsx`、`src/App.css`*（仅 `var(--sl-*)`，无 hex 定义）、`index.html`*（启动链硬编码 `#1e1e2e`） |
| 方案系统 | `theme/schemes/darcula.ts`*（值定义方）、`theme/schemeRegistry.ts`*、`theme/colors.ts`*（facade）、`theme/overrides.ts`*（四通道）、`theme/index.ts`*（barrel） |
| 活动栏/侧栏 | `features/sideViews/ActivityBar.tsx`、`SideBarArea.tsx`、`features/sidebar/SidebarTree.tsx` |
| 文件浏览器 | `features/explorer/ExplorerPanel.tsx`（含 ERROR_BANNER 横幅）、`FileTree.tsx`、`FileIcon.tsx` |
| Commit 视图 | `features/commit/CommitView.tsx`、`CommitFileList.tsx` |
| Agent/历史 | `features/agentStatus/AgentStatusView.tsx`、`AgentStatusRow.tsx`、`features/claudeHistory/ClaudeHistorySections.tsx`、`HistorySessionList.tsx`、`HistorySessionRow.tsx`、`SessionActionDialog.tsx` |
| 终端 | `panels/terminal/TerminalPanel.tsx`、`terminal/theme.ts`*（adapter，映射 active 方案 terminal 段）、`terminal/useXterm.ts`*（import `./theme`）、`terminal/index.ts`*（re-export terminalOptions） |
| 编辑器 | `panels/editor/EditorPanel.tsx`、`gitGutter.ts`、`useCodeMirror.ts`（editorTheme + editorColorOverrides） |
| 面板 | `panels/html/HtmlPanel.tsx`、`gitshow/GitShowPanel.tsx`、`diff/DiffPanel.tsx` |
| hooks 配置 | `panels/hooksConfig/HooksConfigPanel.tsx`、`JsonMode.tsx`（含 ON_ACCENT_FG）、`GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`、`MatcherTester.tsx` |
| 工作区 | `workspace/PageDockviewHost.tsx`（dockviewVarStyle + Watermark）、`workspace/Workspace.tsx`（allotmentVarStyle） |

### 8.2 颜色相关测试守卫（改 token 必须同步）

> 完整测试守卫清单（按「改色即红 / token 联动」两分类）见 `color-inventory.md` §8——**唯一真值源，改色以 inventory 为准**。本节仅保留独有注记：

- `colors.test.ts`（`src/__tests__/`）是全 token 断言（it.each 表驱动），**断言值 = darcula 方案值**——改 darcula 值必须同步；改其他方案不影响。用例「共 24 个 UI token」（21 通用 + 3 ERROR_BANNER）已随死配置清理与 ON_ACCENT_FG 新增同步

### 8.3 漏网点（改色时需人工同步，无守卫）

| 位置 | 内容 | 性质 |
|------|------|------|
| `src/main.tsx` 超时页 | `background:#1e1e1e;color:#f44747` | 启动链合理例外（React 未挂载） |
| `index.html:10` | body `#1e1e2e` | 启动链，与 ui.appBgPrimary 同值但独立（darcula.ts 头注释交叉引用） |
| `src-tauri/tauri.conf.json:20` | `backgroundColor: "#1e1e2e"` | 窗口层，同上 |
| `@codemirror/theme-one-dark` 包内语法色板 | 10 命名色 + 背景族 | 经 editor.theme 引用，色值仍由包定义（新方案可换包，见 §9） |

> 原 JsonMode 事件导航 hover 硬编码（`#FFFFFF`）已收敛为 `ON_ACCENT_FG` token——真违规清零。

### 8.4 数值重复但不联动（改一处不会带动另一处）

> 色值统一大写表示（各来源实际大小写见括号注明）。

| 色值 | 出现位置 |
|------|---------|
| `#1e1e2e` | darcula.ts ui.appBg/appBgPrimary / index.html body / tauri.conf.json backgroundColor（三处均小写；后两处为启动链独立硬编码） |
| `#1E1E1E` | darcula.ts ui.panelBg/explorer.bg（大写）/ xterm terminal 段 background（大写）/ libraries.dockview `--dv-group-view-background-color` 引用 ui.panelBg（**值单点**，重构后已联动）/ main.tsx 超时页（小写，fail-safe） |
| `#252526`、`#2D2D2D` | darcula.ts ui.sidebarBg/secondaryBg（大写）/ libraries.dockview 对应条目引用 ui 槽位（**已联动**） |
| `#282C34` | darcula.ts ui.editorBg（大写）/ editor.overrides.background（同源）/ oneDark 包内 background（小写，包内独立） |
| `#D4D4D4` | darcula.ts ui.sidebarFg/explorer.fg/arrowOpen/terminal.foreground（同源值单点）/ oneDark 无关 |

### 8.5 死配置清理结果（spec §9.2 回写）

| 死配置 | 处置 |
|--------|------|
| `DROPDOWN_BG`（原 colors.ts） | **已删除**——生产零消费 |
| `APP_BG_SECONDARY`（原 colors.ts） | **已删除**——零消费，连带 CSS 变量 `--sl-bg-secondary` |
| `GIT_GUTTER_COLORS.whitespaceOnly` | **已删除**——gitGutter 收敛 3 键 |
| `EXPLORER_COLORS.selected` | **已删除**——explorer 收敛 5 键；选中语义由 `EXPLORER_SELECTION_BG`/`SIDEBAR_COLORS.selected` 承担 |
| `--sl-fg-secondary`（原 App.css:7） | **已删除**——零消费 |
| `ERROR_BANNER_*` | **不是**死 token：`ExplorerPanel.tsx` 沙箱错误横幅消费（背景/边框/文字），保留 |

### 8.6 内容层（不可配置，改色无需处理）

- emoji 图标（文件图标映射、F3 四态 ⚡🟡✅❌）：颜色由系统 emoji 字体决定
- 终端输出的 ANSI 内容色（claude/pwsh 输出）：渲染为 xterm 主题 16 色，但色值来源是子进程内容，非 UI 配置
- shell 集成脚本（`src-tauri/assets/shell-integration.ps1`）：**不含 ANSI 色序列**（仅 OSC 7/9;9/133 控制序列），无需处理
- `public/tauri.svg`（#FFC131/#24C8DB）：**孤儿文件**（从未被任何代码引用，改色可忽略）——但 Vite 会将 `public/` 原样拷贝进 `dist/`，文件实际随打包产物发布（彻底清理孤儿资产时可删除）；应用图标实际来自 `src-tauri/icons/`（PNG/ICO 二进制）；`index.html:5` favicon 指向不存在的 `/vite.svg`（404，与颜色无关）

## 9. 新增配色方案步骤

1. `src/theme/schemes/<name>.ts` 新建值文件，实现 `ColorScheme` 接口（四段齐全；与 ui 同值的库条目引用 ui 槽位；终端/编辑器段按需取现状值或自定——`editor.theme` 默认复用 oneDark 透出，见决策 D6）
2. `schemes/index.ts` 追加 `import { <name> } from "./<name>"` + `schemeRegistry.register(<name>)` 一行
3. `~/.slterminal/settings.json` 写 `"colorScheme": "<name>"` → 重载窗口生效（五通道全部映射；未知 id 回退 darcula）
4. 消费方（colors.ts facade / overrides.ts / 组件）与测试守卫**零改动**——仅若新增 token 槽位才需 types.ts + darcula.ts + facade + 消费点/测试四方同步
5. 验证：ACC-05 冒烟（临时方案改单色 → 五通道生效 → 删除还原）

## 10. 相关文档

- 颜色值全表（含默认色）：[`color-inventory.md`](./color-inventory.md)
- 重构规格与决策：[`color-scheme-refactor-spec.md`](./color-scheme-refactor-spec.md)、`.claude/adr.md` ADR-0002
- 配色单点约束：#6，见 `.claude/CLAUDE.md` 硬性开发约束；方案系统细节见 `src/theme/CLAUDE.md`
- 终端配色 adapter：`src/panels/terminal/CLAUDE.md`
