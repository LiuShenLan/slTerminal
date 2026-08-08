// darcula — 内置默认配色方案（JetBrains IDEA Darcula 暗色主题）
//
// UI 区域速查（2026-08-08 优化）：
//   ui 段        → 应用自绘 UI 全部区域（侧栏/活动栏/文件树/编辑器容器/弹窗/表单/提示文案），
//                  组件经 theme/colors.ts facade 引用 token，禁止硬编码颜色
//   terminal 段  → xterm 终端画面（panels/terminal/theme.ts adapter 展开进 ITheme）
//   editor 段    → CodeMirror 6 编辑器（overrides.ts editorTheme + editorColorOverrides 应用）
//   libraries 段 → dockview 页签/布局 + allotment 三栏分割线（overrides.ts VarStyle 内联注入）
// 消费注释权威在 types.ts 各槽位 JSDoc（决策 D8）——本文件为 UI 区域速查，改值前对照 types.ts。
//
// 交叉引用（启动链 fail-safe，spec §9.1）：React 挂载前的静态硬编码色不在方案系统内，
// 改本文件对应 ui 值时必须手动同步——
//   ui.appBgPrimary (#1e1e2e) ↔ index.html:10 body background
//   ui.appBgPrimary (#1e1e2e) ↔ src-tauri/tauri.conf.json:20 window backgroundColor
//   ui.panelBg (#1e1e1e) / ui.errorFg (#f44747) ↔ src/main.tsx:31 超时错误页
//
// 值一律搬运现状（D1 零视觉变化，禁止新造）。

import { oneDark } from "@codemirror/theme-one-dark";
import type { ColorScheme, UiTokens } from "./types";

// --- ui 段：应用自绘 UI 配色（值 = src/theme/colors.ts 现状 + App.css:6 appFg 收编 + JsonMode:213 onAccentFg 收编）---

const ui: UiTokens = {
  // 文件名 git 状态色：Commit 视图文件名 + 文件树 FileIcon + FileTree 行内状态色
  gitFile: {
    modified: "#6897BB",    // 已修改文件
    added: "#629755",       // 已暂存/新增文件
    untracked: "#D1675A",   // 未跟踪文件
    deleted: "#6C6C6C",     // 已删除文件
    renamed: "#3A8484",     // 已重命名文件
    conflict: "#D5756C",    // 冲突文件
    ignored: "#848504",     // 被忽略文件
  },
  // 行内 diff 边栏色：编辑器/双栏 diff 面板左侧 gutter 的增删改标记
  gitGutter: {
    modified: "#374752",    // 修改行标记
    added: "#384C38",       // 新增行标记
    deleted: "#656E76",     // 删除行三角标记
  },
  // 文件浏览器通用色：左侧文件树（背景/文字/悬停/箭头）
  explorer: {
    bg: "#1E1E1E",          // 文件树背景
    fg: "#D4D4D4",          // 文件树文字
    hover: "#2A2D2E",       // 行悬停背景
    arrowClosed: "#6C6C6C", // 折叠箭头
    arrowOpen: "#D4D4D4",   // 展开箭头
  },
  // 侧栏配色：侧栏树/活动栏/Agent 状态行/右键菜单/树形引导线
  sidebar: {
    bg: "#252526",                // 侧栏背景
    fg: "#D4D4D4",                // 侧栏文字
    hover: "#2A2D2E",             // 行悬停背景
    selected: "#37373D",          // 行选中背景（侧栏树/活动栏激活态）
    border: "#444",               // 侧栏边框
    contextMenuBorder: "#454545", // 右键菜单边框
    contextMenuShadow: "0 4px 12px rgba(0,0,0,0.5)", // 右键菜单阴影
    treeGuide: "#3C3C3C",         // Agent 状态侧栏层级缩进竖线
  },
  // 沙箱错误横幅：ExplorerPanel 顶部路径沙箱拒绝提示条
  errorBanner: {
    bg: "#5A1D1D",     // 横幅背景
    border: "#8B0000", // 横幅边框
    fg: "#F48771",     // 横幅文字
  },
  // 用量条分段色：Agent 状态侧栏上下文用量条（阈值由组件逻辑决定 <50/50-80/>80）
  agentStatusUsage: {
    low: "#629755",     // 低用量 <50%
    medium: "#BBB529",  // 中用量 50-80%
    high: "#F44747",    // 高用量 >80%
  },

  // --- 标量（23 键）：全部自绘 UI 通用色 ---
  panelBg: "#1E1E1E",           // 全部面板背景（终端/编辑器/diff/侧栏视图等容器底色）
  sidebarBg: "#252526",         // 右键菜单/弹窗底色
  secondaryBg: "#2D2D2D",       // 页签按钮/弹窗次级背景
  appBg: "#1e1e2e",             // App 根容器背景（窗口最底层）
  appBgPrimary: "#1e1e2e",      // 全局背景 → --sl-bg-primary（防白闪底色，见文件头 fail-safe 交叉引用）
  appFg: "#cdd6f4",             // 全局默认文字色 → --sl-fg-primary
  editorBg: "#282C34",          // 编辑器类面板容器背景（编辑器/gitshow/diff/JsonMode 容器）
  sidebarFg: "#D4D4D4",         // 侧栏/hooks 配置面板主要文字
  errorFg: "#F44747",           // 错误文案/错误状态文字（全局）
  placeholderFg: "#808080",     // 占位符/禁用项灰显/页签关闭按钮
  buttonFg: "#CCCCCC",          // 按钮文字
  dimFg: "#999999",             // 次要说明文字（用量/时间戳等）
  inputBg: "#3C3C3C",           // 输入框/下拉框背景
  inputBorder: "#6C6C6C",       // 输入框/卡片边框（全应用最高频 token）
  focusBorder: "#007ACC",       // 聚焦边框/活动栏选中指示条/重命名输入框边框
  activeSelectionBg: "#094771", // 列表/树选中背景（VS Code 风格）
  separatorBg: "#444",          // 分隔线（diff 分栏线/侧栏分隔等）
  contextMenuBorder: "#454545", // 右键菜单边框
  shadowMenu: "rgba(0,0,0,0.5)", // 弹窗遮罩阴影（SessionActionDialog 等）
  htmlPanelLoadingFg: "#6C6C6C", // 「加载中…」文案（HTML 预览/diff/gitshow 等加载态）
  htmlPanelIframeBg: "#FFFFFF",  // HTML 预览 iframe 白底
  onAccentFg: "#FFFFFF",         // 强调底色上的文字（JsonMode 事件导航 hover）
  explorerSelectionBg: "#094771", // 文件树/历史会话行选中背景
};

// --- 方案组装（四段）---

export const darcula: ColorScheme = {
  id: "darcula",
  label: "Darcula",
  ui,
  // terminal 段：xterm 终端画面配色（值 = src/panels/terminal/theme.ts theme 段现状，25 键）
  //   foreground/background/cursor/选区 → 终端画面基础色
  //   ANSI 16 色（black~white + bright*）→ 终端输出内容的颜色（claude/pwsh 命令输出按 ANSI 映射到此）
  //   scrollbar 三键 → 终端右侧滚动条滑块（默认/hover/激活）
  terminal: {
    foreground: "#D4D4D4",          // 终端默认文字
    background: "#1E1E1E",          // 终端底色
    cursor: "#D4D4D4",              // 光标色
    cursorAccent: "#1E1E1E",        // 光标悬于字符上时字符的前景色
    selectionBackground: "#264F78", // 选中文本背景
    selectionForeground: "#D4D4D4", // 选中文本前景
    // 滚动条三键 = foreground 20%/40%/50% 等价（ITheme 显式化原运行期派生值，spec §4.3）
    scrollbarSliderBackground: "rgba(212,212,212,0.2)",      // 滚动条滑块默认
    scrollbarSliderHoverBackground: "rgba(212,212,212,0.4)", // 滚动条滑块 hover
    scrollbarSliderActiveBackground: "rgba(212,212,212,0.5)", // 滚动条滑块拖拽中
    // ANSI 基本色（0-7）：终端输出低亮度色
    black: "#000000",
    red: "#CD3131",
    green: "#0DBC79",
    yellow: "#E5E510",
    blue: "#2472C8",
    magenta: "#BC3FBC",
    cyan: "#11A8CD",
    white: "#E5E5E5",
    // ANSI 亮色系（8-15）：终端输出高亮度色（粗体经 drawBoldTextInBrightColors 映射到此）
    brightBlack: "#666666",
    brightRed: "#F14C4C",
    brightGreen: "#23D18B",
    brightYellow: "#F5F543",
    brightBlue: "#3B8EEA",
    brightMagenta: "#D670D6",
    brightCyan: "#29B8DB",
    brightWhite: "#FFFFFF",
  },
  // editor 段：CM6 编辑器配色（值 = spec §4.4 现行有效值，测试 mock @codemirror/theme-one-dark 包继续生效）
  //   theme → CM6 基础主题（oneDark 透出：语法高亮 + 编辑器底色）
  //   overrides → 编辑器颜色覆盖（overrides.ts editorColorOverrides 应用，层叠规则见 theme/CLAUDE.md）
  editor: {
    theme: oneDark,
    overrides: {
      background: "#282C34", // 编辑器底色（对齐 ui.editorBg）
      // lint 诊断色：JsonMode（hooks 配置 JSON 编辑）语法/schema 校验波浪线与提示框（@codemirror/lint）
      lint: {
        error: "#f11",                // 错误波浪线
        warning: "orange",            // 警告波浪线
        info: "#999",                 // 信息波浪线
        hint: "#66d",                 // 提示波浪线
        activeBackground: "#ffdd9980", // lint 消息激活背景
        tooltipBackground: "#2e343e", // lint 提示框背景
        tooltipBorder: "#444",        // lint 提示框边框
      },
      // 搜索匹配高亮：Ctrl+F 搜索（editor/diff/gitshow 面板）
      searchMatch: {
        match: "#72a1ff59",           // 匹配文本背景
        matchOutline: "#457dff",      // 匹配文本描边
        selected: "#6199ff2f",        // 选中匹配背景
        selectionMatch: "#aafe661a",  // 多匹配整体背景
      },
    },
  },
  // libraries 段：三方库 CSS 变量覆盖（值 = spec §4.5 现状）；与 ui 同值的条目引用 ui 槽位构造，值单点定义
  //   dockview → 页签/面板布局（PageDockviewHost 根 div 内联注入，className="dockview-theme-dark" 保留）
  //   allotment → 三栏布局分割线（Workspace 根容器注入，CSS 变量继承覆盖内层 SideBarArea）
  libraries: {
    dockview: {
      "--dv-group-view-background-color": ui.panelBg,     // 面板组背景（页签+内容区整体底色）
      "--dv-tabs-and-actions-container-background-color": ui.sidebarBg, // 页签栏背景
      "--dv-activegroup-visiblepanel-tab-background-color": "#1E1E1E", // 聚焦组当前页签背景
      "--dv-activegroup-hiddenpanel-tab-background-color": ui.secondaryBg, // 聚焦组其他页签背景
      "--dv-inactivegroup-visiblepanel-tab-background-color": "#1E1E1E", // 非聚焦组当前页签背景
      "--dv-inactivegroup-hiddenpanel-tab-background-color": ui.secondaryBg, // 非聚焦组其他页签背景
      "--dv-tab-divider-color": "#1E1E1E",                // 页签分隔线
      "--dv-separator-border": ui.separatorBg,            // 面板组间分隔线（分屏拖动条）
      "--dv-paneview-header-border-color": "rgba(204,204,204,0.2)", // 页眉边框
      "--dv-activegroup-visiblepanel-tab-color": "#FFFFFF", // 聚焦组当前页签文字
      "--dv-activegroup-hiddenpanel-tab-color": "#969696",  // 聚焦组其他页签文字
      "--dv-inactivegroup-visiblepanel-tab-color": "#8F8F8F", // 非聚焦组当前页签文字
      "--dv-inactivegroup-hiddenpanel-tab-color": "#626262", // 非聚焦组其他页签文字
      "--dv-drag-over-background-color": "rgba(83,89,93,0.5)", // 拖拽悬停目标面板背景
      "--dv-icon-hover-background-color": "rgba(90,93,94,0.31)", // 页签图标 hover 背景
      "--dv-floating-box-shadow": "0 8px 32px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.25)", // 浮动组阴影
      "--dv-floating-border": "1px solid rgba(255,255,255,0.1)", // 浮动组边框
      "--dv-tabs-container-scrollbar-color": "#888888",   // 页签栏滚动条
      "--dv-scrollbar-background-color": "rgba(255,255,255,0.25)", // 面板滚动条背景
      "--dv-paneview-active-outline-color": "dodgerblue", // 拖拽 outline
    },
    allotment: {
      separatorBorder: "rgba(128,128,128,0.35)", // 三栏布局分割线（活动栏/侧栏/主区之间）
      focusBorder: "#007fd4",                    // 拖拽分割线时 sash 高亮边框
    },
  },
};
