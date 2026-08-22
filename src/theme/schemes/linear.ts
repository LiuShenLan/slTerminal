// linear — 内置默认配色方案（Linear 极黑克制，ADR-0003 定稿）
//
// 值契约锚点 = .claude/adr.md ADR-0003（执行 agent 只准照抄本文件现有值，禁止自估色值）。
// 消费注释权威在 types.ts 各槽位 JSDoc（决策 D8）——本文件只含 fail-safe 交叉引用。
//
// 交叉引用（启动链 fail-safe）：React 挂载前的静态硬编码色不在方案系统内，
// 改本文件对应 ui 值时必须手动同步——
//   ui.appBgPrimary (#0a0a0b) ↔ index.html:10 body background
//   ui.appBgPrimary (#0a0a0b) ↔ src-tauri/tauri.conf.json:21 window backgroundColor
//   ui.panelBg (#0a0a0b) / ui.errorFg (#d9706b) ↔ src/main.tsx:28 超时错误页（文字 #ece9e4）
//
// linear 为 UI 重设计定稿方案，值以附录 A 为契约随意图演进。

import { oneDark } from "@codemirror/theme-one-dark";
import type { ColorScheme, UiTokens } from "./types";

// --- ui 段：应用自绘 UI 配色（消费方经 colors.ts facade 引用，禁止硬编码颜色）---

const ui: UiTokens = {
  // 文件名 git 状态色：Commit 视图文件名 + 文件树 FileIcon + FileTree 行内状态色
  gitFile: {
    modified: "#d6b25e",    // 已修改文件
    added: "#86bb7a",       // 已暂存/新增文件
    untracked: "#6fbfc4",   // 未跟踪文件
    deleted: "#d9706b",     // 已删除文件
    renamed: "#6e9ff2",     // 已重命名文件
    conflict: "#d9706b",    // 冲突文件
    ignored: "#6b675f",     // 被忽略文件
  },
  // 行内 diff 边栏色：编辑器/双栏 diff 面板左侧 gutter 的增删改标记
  gitGutter: {
    modified: "#d6b25e",    // 修改行标记
    added: "#86bb7a",       // 新增行标记
    deleted: "#d9706b",     // 删除行三角标记
  },
  // 文件浏览器通用色：左侧文件树（背景/文字/悬停/箭头）
  explorer: {
    bg: "#101012",          // 文件树背景
    fg: "#b3aea6",          // 文件树文字
    hover: "#222227",       // 行悬停背景
    arrowClosed: "#8a857d", // 折叠箭头
    arrowOpen: "#8a857d",   // 展开箭头
  },
  // 侧栏配色：侧栏树/活动栏/Agent 状态行/右键菜单/树形引导线
  sidebar: {
    bg: "#101012",                      // 侧栏背景
    fg: "#b3aea6",                      // 侧栏文字
    hover: "#222227",                   // 行悬停背景
    selected: "rgba(110,159,242,0.13)", // 行选中背景（侧栏树/活动栏激活态）
    border: "rgba(255,255,255,0.055)",  // 侧栏边框
    contextMenuBorder: "rgba(255,255,255,0.09)", // 右键菜单边框
    contextMenuShadow: "0 8px 32px rgba(0,0,0,0.35)", // 右键菜单阴影
    treeGuide: "rgba(255,255,255,0.055)", // 侧栏层级缩进竖线
  },
  // 沙箱错误横幅：ExplorerPanel 顶部路径沙箱拒绝提示条
  errorBanner: {
    bg: "rgba(217,112,107,0.12)", // 横幅背景
    border: "#d9706b",            // 横幅边框
    fg: "#ece9e4",                // 横幅文字
  },
  // 用量条分段色：Agent 状态侧栏上下文用量条（阈值由组件逻辑决定 ≥90/≥70/≥50）
  agentStatusUsage: {
    low: "#86bb7a",      // 低用量 <50%
    medium: "#a9c686",   // 中用量 50-70%
    high: "#d6b25e",     // 高用量 70-90%
    critical: "#d9706b", // 临界用量 ≥90%
  },

  // --- 标量（23 既有 + 4 新增）：全部自绘 UI 通用色 ---
  panelBg: "#0a0a0b",                    // 全部面板背景（终端/编辑器/diff/侧栏视图等容器底色）
  sidebarBg: "#1a1a1e",                  // 右键菜单/弹窗底色
  secondaryBg: "#222227",                // 页签按钮/弹窗次级背景
  appBg: "#0a0a0b",                      // App 根容器背景（窗口最底层）
  appBgPrimary: "#0a0a0b",               // 全局背景 → --sl-bg-primary（防白闪底色，见文件头 fail-safe 交叉引用）
  appFg: "#b3aea6",                      // 全局默认文字色 → --sl-fg-primary
  editorBg: "#0a0a0b",                   // 编辑器类面板容器背景（编辑器/gitshow/diff/JsonMode 容器）
  sidebarFg: "#ece9e4",                  // 侧栏/hooks 配置面板主要文字
  errorFg: "#d9706b",                    // 错误文案/错误状态文字（全局）
  placeholderFg: "#6b675f",              // 占位符/禁用项灰显/页签关闭按钮
  buttonFg: "#ece9e4",                   // 按钮文字
  dimFg: "#8a857d",                      // 次要说明文字（用量/时间戳等）
  inputBg: "#1a1a1e",                    // 输入框/下拉框背景
  inputBorder: "rgba(255,255,255,0.09)", // 输入框/卡片边框（全应用最高频 token）
  focusBorder: "#6e9ff2",                // 聚焦边框/活动栏选中指示条/重命名输入框边框
  activeSelectionBg: "rgba(110,159,242,0.13)", // 列表/树选中背景
  separatorBg: "rgba(255,255,255,0.055)",      // 分隔线（diff 分栏线/侧栏分隔等）
  contextMenuBorder: "rgba(255,255,255,0.09)", // 右键菜单边框
  shadowMenu: "rgba(0,0,0,0.55)",              // 弹窗遮罩阴影（ConfirmDialog 等）
  htmlPanelLoadingFg: "#8a857d",               // 「加载中…」文案（HTML 预览/diff/gitshow 等加载态）
  htmlPanelIframeBg: "#FFFFFF",                // HTML 预览 iframe 白底
  onAccentFg: "#0c1220",                       // 强调底色上的文字
  explorerSelectionBg: "rgba(110,159,242,0.13)", // 文件树/历史会话行选中背景
  accentFg: "#8fb4f5",                          // 强调派生前景色（活动栏激活图标/状态行模型段）
  selectionHoverBg: "rgba(110,159,242,0.22)",   // 选中行 hover（accent-dim-2）
  titlebarBg: "#141416",                        // 自绘标题栏 chrome 底（明度阶梯 l2）
  titlebarCloseHover: "#c04747",                // 自绘标题栏关闭钮 hover 底（UI-301 定值）
};

// --- 方案组装（四段）---

export const linear: ColorScheme = {
  id: "linear",
  label: "Linear",
  ui,
  // terminal 段：xterm 终端画面配色（25 键；经 panels/terminal/theme.ts adapter 展开进 ITheme）
  //   foreground/background/cursor/选区 → 终端画面基础色
  //   ANSI 16 色（black~white + bright*）→ 终端输出内容的颜色（claude/pwsh 命令输出按 ANSI 映射到此）
  //   scrollbar 三键 → 终端右侧滚动条滑块（默认/hover/激活）
  terminal: {
    foreground: "#cfcac1",                          // 终端默认文字
    background: "#0a0a0b",                          // 终端底色
    cursor: "#6e9ff2",                              // 光标色
    cursorAccent: "#0a0a0b",                        // 光标悬于字符上时字符的前景色
    selectionBackground: "rgba(110,159,242,0.28)",  // 选中文本背景
    selectionForeground: "#f0ede8",                 // 选中文本前景
    // 滚动条三键：实色值（默认 / hover / 拖拽中）
    scrollbarSliderBackground: "rgba(255,255,255,0.10)",      // 滚动条滑块默认
    scrollbarSliderHoverBackground: "rgba(255,255,255,0.20)", // 滚动条滑块 hover
    scrollbarSliderActiveBackground: "rgba(255,255,255,0.28)", // 滚动条滑块拖拽中
    // ANSI 基本色（0-7）：终端输出低亮度色
    black: "#0a0a0b",
    red: "#d9706b",
    green: "#93b573",
    yellow: "#d6b25e",
    blue: "#7fa8e8",
    magenta: "#b48ce0",
    cyan: "#6fbfc4",
    white: "#cfcac1",
    // ANSI 亮色系（8-15）：终端输出高亮度色（粗体经 drawBoldTextInBrightColors 映射到此）
    brightBlack: "#7d7871",
    brightRed: "#e2877f",
    brightGreen: "#a8c98d",
    brightYellow: "#e3c67f",
    brightBlue: "#9dbfee",
    brightMagenta: "#c6a6e8",
    brightCyan: "#8dd0d4",
    brightWhite: "#f0ede8",
  },
  // editor 段：CM6 编辑器配色（theme 仅作底座，语法色经 overrides.syntax 覆盖）
  //   theme → CM6 基础主题（oneDark 透出：语法高亮底座 + 编辑器底色）
  //   overrides → 编辑器颜色覆盖（overrides.ts editorTheme/editorColorOverrides 应用，层叠规则见 theme/CLAUDE.md）
  editor: {
    theme: oneDark,
    overrides: {
      background: "#0a0a0b", // 编辑器底色（对齐 ui.editorBg）
      // lint 诊断色：JsonMode（hooks 配置 JSON 编辑）语法/schema 校验波浪线与提示框（@codemirror/lint）
      lint: {
        error: "#d9706b",                            // 错误波浪线
        warning: "#d6b25e",                          // 警告波浪线
        info: "#6e9ff2",                             // 信息波浪线
        hint: "#8a857d",                             // 提示波浪线
        activeBackground: "rgba(110,159,242,0.13)",  // lint 消息激活背景
        tooltipBackground: "#1a1a1e",                // lint 提示框背景
        tooltipBorder: "rgba(255,255,255,0.09)",     // lint 提示框边框
      },
      // 搜索匹配高亮：Ctrl+F 搜索（editor/diff/gitshow 面板）
      searchMatch: {
        match: "rgba(214,178,94,0.25)",  // 匹配文本背景（半透明叠加，不遮挡选区/高亮）
        matchOutline: "transparent",     // 匹配文本描边
        selected: "rgba(214,178,94,0.45)", // 选中匹配背景（半透明叠加）
        selectionMatch: "rgba(214,178,94,0.25)", // 多匹配整体背景（半透明叠加）
      },
      // 语法高亮 token 色：CM 正文 token 着色（HighlightStyle 映射，经 overrides.ts editorSyntaxHighlight 应用）
      syntax: {
        property: "#d9827e",  // 属性名
        string: "#93b573",    // 字符串
        number: "#d89a66",    // 数字
        keyword: "#b48ce0",   // 关键字
        function: "#7fa8e8",  // 函数名
        type: "#6fbfc4",      // 类型名
        operator: "#6fbfc4",  // 运算符
        punctuation: "#7d7871", // 标点
        comment: "#6b675f",   // 注释
      },
      plainText: "#b3aea6", // 正文前景色
      lineNumber: "#6b675f", // 行号前景色
      lineNumberActive: "#b3aea6", // 活跃行行号前景色
    },
  },
  // libraries 段：三方库 CSS 变量覆盖（libraries 段独立硬编码，有意与 ui 段解耦）
  //   dockview → 页签/面板布局（PageDockviewHost 根 div 内联注入，className="dockview-theme-dark" 保留）
  //   allotment → 三栏布局分割线（Workspace 根容器注入，CSS 变量继承覆盖内层 SideBarArea）
  libraries: {
    dockview: {
      "--dv-group-view-background-color": "#0a0a0b",     // 面板组背景（页签+内容区整体底色）
      "--dv-tabs-and-actions-container-background-color": "#101012", // 页签栏背景
      "--dv-activegroup-visiblepanel-tab-background-color": "#0a0a0b", // 聚焦组当前页签背景
      "--dv-activegroup-hiddenpanel-tab-background-color": "transparent", // 聚焦组其他页签背景
      "--dv-inactivegroup-visiblepanel-tab-background-color": "#0a0a0b", // 非聚焦组当前页签背景
      "--dv-inactivegroup-hiddenpanel-tab-background-color": "transparent", // 非聚焦组其他页签背景
      "--dv-tab-divider-color": "transparent",        // 页签分隔线
      "--dv-separator-border": "rgba(255,255,255,0.055)", // 面板组间分隔线（分屏拖动条）
      "--dv-paneview-header-border-color": "rgba(255,255,255,0.055)", // 页眉边框
      "--dv-activegroup-visiblepanel-tab-color": "#ece9e4", // 聚焦组当前页签文字
      "--dv-activegroup-hiddenpanel-tab-color": "#8a857d",  // 聚焦组其他页签文字
      "--dv-inactivegroup-visiblepanel-tab-color": "#ece9e4", // 非聚焦组当前页签文字
      "--dv-inactivegroup-hiddenpanel-tab-color": "#8a857d", // 非聚焦组其他页签文字
      "--dv-drag-over-background-color": "rgba(110,159,242,0.13)", // 拖拽悬停目标面板背景
      "--dv-icon-hover-background-color": "#2b2b31",  // 页签图标 hover 背景
      "--dv-floating-box-shadow": "0 8px 32px rgba(0,0,0,0.35)", // 浮动组阴影
      "--dv-floating-border": "1px solid rgba(255,255,255,0.09)", // 浮动组边框
      "--dv-tabs-container-scrollbar-color": "rgba(255,255,255,0.20)", // 页签栏滚动条
      "--dv-scrollbar-background-color": "rgba(255,255,255,0.10)", // 面板滚动条背景
      "--dv-paneview-active-outline-color": "#6e9ff2", // 拖拽 outline
    },
    allotment: {
      separatorBorder: "rgba(255,255,255,0.055)", // 三栏布局分割线（活动栏/侧栏/主区之间）
      focusBorder: "#6e9ff2",                     // 拖拽分割线时 sash 高亮边框
    },
  },
};
