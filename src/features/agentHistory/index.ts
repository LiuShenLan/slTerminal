// index.ts — agent 历史会话模块 barrel export（照 commit/index.ts 模式）
// 导出视图所需公共 API：弹窗、数据 hook、恢复编排、右键菜单策略、模型纯函数。
// NAV-08：AgentHistorySections 已删除（三区结构随 AgentStatusView 退役，
// 历史区迁入导航树 NavTree——useNavTree 内建聚合，见 ../navTree/useNavTree.ts）。
// FE-25：HistorySessionList/HistorySessionRow 已删除（退役组件，生产零消费方，
// 历史行由导航树 NavHistoryRow 承担——见 ../navTree/NavHistoryRow.tsx）。

export { useAgentHistory } from "./useAgentHistory";
export type { AgentHistoryState } from "./useAgentHistory";
export { restoreHistorySession } from "./restoreSession";
export { SessionActionDialog } from "./SessionActionDialog";
export type { SessionActionDialogProps } from "./SessionActionDialog";
export { getHistoryContextMenuItems, buildResumeCommand } from "./historyContextMenu";
export type { HistoryMenuItem, HistoryContextMenuOpts } from "./historyContextMenu";
export {
  UNKNOWN_CWD_KEY,
  isCurrentProject,
  groupByCwd,
  matchesSearch,
  formatRelativeTime,
  keyOf,
  deriveActiveSessionStatuses,
} from "./historyModel";
export type { CwdGroup } from "./historyModel";
