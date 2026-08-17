// theme barrel —— 配色 token facade（colors.ts）+ 方案注册表 + 方案定义 + 库覆盖
//
// colors.ts 段导出随 C1 清单：35 个（5 组 + 3 ERROR_BANNER 标量 + 26 其他标量 + ROOT_CSS_VARS）；
// 方案系统段：schemeRegistry（注册表单例）+ schemes 类型与 linear + overrides 四导出。

export {
  GIT_FILE_COLORS,
  GIT_GUTTER_COLORS,
  EXPLORER_COLORS,
  SIDEBAR_COLORS,
  PANEL_BG,
  SIDEBAR_BG,
  SECONDARY_BG,
  APP_BG,
  APP_BG_PRIMARY,
  EDITOR_BG,
  SIDEBAR_FG,
  ERROR_FG,
  PLACEHOLDER_FG,
  BUTTON_FG,
  DIM_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ACTIVE_SELECTION_BG,
  EXPLORER_SELECTION_BG,
  SEPARATOR_BG,
  CONTEXT_MENU_BORDER,
  SHADOW_MENU,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
  ON_ACCENT_FG,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
  ROOT_CSS_VARS,
  AGENT_STATUS_USAGE_COLORS,
  ACCENT_FG,
  SELECTION_HOVER_BG,
  TITLEBAR_BG,
  TITLEBAR_CLOSE_HOVER_BG,
} from "./colors";

// 方案注册表（单例 + 类型）
export { schemeRegistry, SchemeRegistry } from "./schemeRegistry";

// 方案定义（linear 值 + ColorScheme 等类型）
export { linear } from "./schemes/linear";
export type {
  ColorScheme,
  UiTokens,
  TerminalPalette,
  EditorScheme,
  LibraryOverrides,
} from "./schemes/types";

// 组件库配色注入（dockview / allotment / CM6）
export {
  dockviewVarStyle,
  allotmentVarStyle,
  editorTheme,
  editorColorOverrides,
  editorSyntaxHighlight,
} from "./overrides";
