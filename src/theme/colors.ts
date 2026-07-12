// colors.ts —— 全局配色 token 中心
//
// 架构约束第 6 条：组件引用 token，禁止硬编码颜色。
// 所有配色来自 JetBrains IDEA 暗色主题 (Darcula)。

// --- 文件名 git 状态色 (JetBrains 暗色) ---
// 用于文件浏览器文件名着色

export const GIT_FILE_COLORS = {
  modified: "#6897BB",
  added: "#629755",
  untracked: "#D1675A",
  deleted: "#6C6C6C",
  renamed: "#3A8484",
  conflict: "#D5756C",
  ignored: "#848504",
} as const;

// --- 行内 diff 边栏色 (JetBrains 暗色) ---
// 用于编辑器 git diff 行标记

export const GIT_GUTTER_COLORS = {
  modified: "#374752",
  added: "#384C38",
  deleted: "#656E76",
  whitespaceOnly: "#4C4638",
} as const;

// --- 文件浏览器通用色 ---

export const EXPLORER_COLORS = {
  bg: "#1E1E1E",
  fg: "#D4D4D4",
  hover: "#2A2D2E",
  selected: "#37373D",
  arrowClosed: "#6C6C6C",
  arrowOpen: "#D4D4D4",
} as const;

// --- 通用 UI 色（暗色主题全局 token）---
// 架构约束第 6 条：组件引用 token，禁止硬编码颜色。
// 新增 token 按语义域分组，覆盖所有 UI 层面颜色需求。

// 背景色
export const PANEL_BG = "#1E1E1E";
export const SIDEBAR_BG = "#252526";
export const SECONDARY_BG = "#2D2D2D";
export const DROPDOWN_BG = "#2A2D2E";
export const APP_BG = "#1e1e2e";
export const EDITOR_BG = "#282C34";

// 前景/文字色
export const SIDEBAR_FG = "#D4D4D4";
export const ERROR_FG = "#F44747";
export const PLACEHOLDER_FG = "#808080";
export const BUTTON_FG = "#CCCCCC";
export const DIM_FG = "#999999";

// 交互控件色
export const INPUT_BG = "#3C3C3C";
export const INPUT_BORDER = "#6C6C6C";
export const FOCUS_BORDER = "#007ACC";
export const ACTIVE_SELECTION_BG = "#094771";
export const SEPARATOR_BG = "#444";
export const CONTEXT_MENU_BORDER = "#454545";

// HTML 面板色
export const HTML_PANEL_LOADING_FG = "#6C6C6C";
export const HTML_PANEL_IFRAME_BG = "#FFFFFF";

// 错误提示色
export const ERROR_BANNER_BG = "#5A1D1D";
export const ERROR_BANNER_BORDER = "#8B0000";
export const ERROR_BANNER_FG = "#F48771";
