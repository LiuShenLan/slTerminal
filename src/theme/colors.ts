// colors.ts —— 配色 token facade（代理 active 配色方案）
//
// 本文件不定义任何颜色值——颜色定义于 schemes/<scheme>.ts 的 ColorScheme.ui 段，
// 本文件在 import 时取 schemeRegistry.getActive() 的 ui 段并逐 token 代理导出。
// 组件只引用本文件 token（硬约束 #6）；方案切换（D2）后本文件导出值随 active 方案变化。
//
// 求值时机保证：本文件首次求值发生在 setActive 之后——
// main.tsx 启动序列先注册内置方案 + setActive，再 import 本文件；
// 测试环境无启动序列，getActive() 默认 darcula，值正确。

import { schemeRegistry } from "./schemeRegistry";
import "./schemes"; // side-effect：注册内置方案（darcula），保证 getActive() 恒有值（测试环境无 main.tsx）

// 取当前 active 方案的 ui 段（模块加载时求值一次，方案切换后重新 import 生效）
const { ui } = schemeRegistry.getActive();

// --- 文件名 git 状态色（ui.gitFile）---
// 用于文件浏览器文件名着色

export const GIT_FILE_COLORS = ui.gitFile;

// --- 行内 diff 边栏色（ui.gitGutter）---
// 用于编辑器 git diff 行标记

export const GIT_GUTTER_COLORS = ui.gitGutter;

// --- 文件浏览器通用色（ui.explorer）---

export const EXPLORER_COLORS = ui.explorer;

// --- 通用 UI 色（ui 标量段）---
// 架构约束第 6 条：组件引用 token，禁止硬编码颜色。

// 背景色
export const PANEL_BG = ui.panelBg;
export const SIDEBAR_BG = ui.sidebarBg;
export const SECONDARY_BG = ui.secondaryBg;
export const APP_BG = ui.appBg;
export const APP_BG_PRIMARY = ui.appBgPrimary;
export const EDITOR_BG = ui.editorBg;

// 前景/文字色
export const SIDEBAR_FG = ui.sidebarFg;
export const ERROR_FG = ui.errorFg;
export const PLACEHOLDER_FG = ui.placeholderFg;
export const BUTTON_FG = ui.buttonFg;
export const DIM_FG = ui.dimFg;

// 交互控件色
export const INPUT_BG = ui.inputBg;
export const INPUT_BORDER = ui.inputBorder;
export const FOCUS_BORDER = ui.focusBorder;
export const ACTIVE_SELECTION_BG = ui.activeSelectionBg;
export const SEPARATOR_BG = ui.separatorBg;
export const CONTEXT_MENU_BORDER = ui.contextMenuBorder;

// 阴影
export const SHADOW_MENU = ui.shadowMenu;

// HTML 面板色
export const HTML_PANEL_LOADING_FG = ui.htmlPanelLoadingFg;
export const HTML_PANEL_IFRAME_BG = ui.htmlPanelIframeBg;

// 强调底色上的前景色（收编 JsonMode 事件导航 hover 硬编码）
export const ON_ACCENT_FG = ui.onAccentFg;

// 侧栏配色 token 组（ui.sidebar）
export const SIDEBAR_COLORS = ui.sidebar;

// 错误提示色（ui.errorBanner）
export const ERROR_BANNER_BG = ui.errorBanner.bg;
export const ERROR_BANNER_BORDER = ui.errorBanner.border;
export const ERROR_BANNER_FG = ui.errorBanner.fg;

// --- Explorer 选中高亮色（ui.explorerSelectionBg）---
export const EXPLORER_SELECTION_BG = ui.explorerSelectionBg;

// --- Agent Status 用量条分段色（ui.agentStatusUsage）---
// 阈值由组件逻辑决定：≥90 critical，≥70 high，≥50 medium，else low。

export const AGENT_STATUS_USAGE_COLORS = ui.agentStatusUsage;

// --- CSS 变量桥接（供 App.css :root 变量从 token 取值）---
// main.tsx 将本对象注入 document.documentElement，App.css 仅通过 var() 引用。

export const ROOT_CSS_VARS = {
  "--sl-bg-primary": ui.appBgPrimary,
  "--sl-fg-primary": ui.appFg,
} as const;
