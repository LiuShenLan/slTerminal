// panelRegistry — 面板类型注册表
//
// 面板只能是此处注册过的类型（硬约束 #5）。
// 新增面板类型 = 加目录 + 在此注册。

import { TerminalPanel } from "../panels/terminal";
import { EditorPanel } from "../panels/editor";

/** Dockview 面板组件注册表 */
export const panelRegistry = {
  terminal: TerminalPanel as React.FC<{
    params: { panelId: string; cwd?: string };
  }>,
  editor: EditorPanel as React.FC<{
    params: { panelId: string; filePath?: string; cwd?: string };
  }>,
};

/** 终端面板渲染策略：始终挂载（保持 PTY 存活） */
export const terminalTabConfig = {
  renderer: "always" as const,
};

/** 面板类型列表（用于 fromJSON 校验白名单） */
export const PANEL_TYPES = ["terminal", "editor"] as const;
export type PanelType = (typeof PANEL_TYPES)[number];

/** 检查面板类型是否有效 */
export function isValidPanelType(type: string): type is PanelType {
  return PANEL_TYPES.includes(type as PanelType);
}
