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
