// index.ts —— 导航树模块 barrel export（NAV-01/02/03/04/09）
//
// 导出主组件（供 sideViewDefs 注册 nav 视图——NAV-05）+ 数据 hook + 行组件 +
// 右键菜单 + 纯辅助。makeEmptyLayout 迁自 SidebarTree（NAV-06 承接约定——
// SidebarTree 退役后 restoreSession 等消费点改引用本导出）。

export { NavTree, makeEmptyLayout } from "./NavTree";
export type { NavTreeProps } from "./NavTree";
export { useNavTree } from "./useNavTree";
export type {
  UseNavTreeResult,
  NavProjectModel,
  NavPageModel,
  NavSessionModel,
  NavHistoryModel,
} from "./useNavTree";
export { NavProjectRow } from "./NavProjectRow";
export { NavPageRow } from "./NavPageRow";
export { NavSessionRow } from "./NavSessionRow";
export { NavHistoryNode } from "./NavHistoryNode";
export { NavHistoryRow } from "./NavHistoryRow";
export { NavContextMenu } from "./NavContextMenu";
export type { NavMenuItem, NavMenuState } from "./NavContextMenu";
