// index.ts — 侧栏视图模块 barrel export
//
// 导出全部公共 API：组件（ActivityBar、SideBarArea）、注册表（sideViewRegistry）、
// 类型（SideViewDef/SideViewComponentProps 等）、纯函数（sideBarState + dropTarget）。
//
// 注意：不 re-export sideViewDefs——由 Workspace 显式 side-effect import
// （同 tabRules 模式），组件内不 import 以防循环。

export { ActivityBar } from "./ActivityBar";

export { SideBarArea } from "./SideBarArea";
export type { SideBarAreaProps } from "./SideBarArea";

export { sideViewRegistry } from "./sideViewRegistry";
export type { SideViewDef, SideViewComponentProps } from "./sideViewRegistry";

// 纯函数——类型、常量、状态机函数
export {
  type Zone,
  type Zones,
  type OpenState,
  type LayoutKind,
  type SideBarSlice,
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  ACTIVITY_BAR_SIZE,
  WIDTH_DEFAULT,
  WIDTH_MIN,
  WIDTH_MAX,
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
  toggleViewPure,
  moveButtonPure,
  deriveLayout,
  reconcileZones,
  sanitizeSideBar,
} from "./sideBarState";

// dropTarget 纯函数
export { computeDropTarget } from "./dropTarget";
export type { ButtonRect, DropTarget } from "./dropTarget";
