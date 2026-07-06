export { default as Workspace } from "./Workspace";
export {
  panelRegistry,
  PANEL_TYPES,
  PANEL_TERMINAL,
  PANEL_EDITOR,
  PANEL_HTML_VIEWER,
  FILE_PANEL_TYPES,
  isValidPanelType,
  isAlwaysRenderPanel,
} from "./panelRegistry";
export type { PanelType } from "./panelRegistry";
export { saveLayout, loadLayout } from "./layoutSerde";
