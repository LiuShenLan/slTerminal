// panelRegistry — 面板类型注册表
//
// 面板只能是此处注册过的类型（硬约束 #5）。
// 新增面板类型 = 加目录 + 在此注册。

import { TerminalPanel } from "./panels/terminal";
import { EditorPanel } from "./panels/editor";
import { HtmlPanel } from "./panels/html";

/** 终端面板类型标识 */
export const PANEL_TERMINAL = "terminal" as const;
/** 编辑器面板类型标识 */
export const PANEL_EDITOR = "editor" as const;
/** HTML 预览面板类型标识 */
export const PANEL_HTML_VIEWER = "htmlviewer" as const;

/** Dockview 面板组件注册表 */
export const panelRegistry = {
  terminal: TerminalPanel as React.FC<{
    params: { panelId: string; cwd?: string };
  }>,
  editor: EditorPanel as React.FC<{
    params: { panelId: string; filePath?: string; cwd?: string };
  }>,
  htmlviewer: HtmlPanel as React.FC<{
    params: { panelId: string; filePath?: string };
  }>,
};

/** 终端面板渲染策略：始终挂载（保持 PTY 存活） */
export const terminalTabConfig = {
  renderer: "always" as const,
};

/** 面板类型列表（用于 fromJSON 校验白名单） */
export const PANEL_TYPES = [
  PANEL_TERMINAL,
  PANEL_EDITOR,
  PANEL_HTML_VIEWER,
] as const;
export type PanelType = (typeof PANEL_TYPES)[number];

/** 文件型面板类型集合——有 filePath、参与标题计算的面板 */
export const FILE_PANEL_TYPES: ReadonlySet<string> = new Set([
  PANEL_EDITOR,
  PANEL_HTML_VIEWER,
]);

/** 检查面板类型是否有效 */
export function isValidPanelType(type: string): type is PanelType {
  return PANEL_TYPES.includes(type as PanelType);
}

/**
 * 检查面板是否应使用 renderer="always" 模式。
 * terminal — 保持 PTY 存活；文件预览类（htmlviewer 等）— 避免 iframe/canvas browsing context 销毁重建导致白屏。
 * editor 故意排除——CodeMirror 重建无视觉闪屏，且大文件编辑器若始终挂载会显著增加内存开销。
 */
export function isAlwaysRenderPanel(type: string): boolean {
  return type === PANEL_TERMINAL || (type !== PANEL_EDITOR && isValidPanelType(type));
}
