// index.ts — agent 历史会话模块 barrel export（照 commit/index.ts 模式）
// 导出视图所需公共 API：组合件、列表、行、弹窗、数据 hook、恢复编排、
// 右键菜单策略、模型纯函数。

export { AgentHistorySections } from "./AgentHistorySections";
export type { AgentHistorySectionsProps } from "./AgentHistorySections";
export { HistorySessionList } from "./HistorySessionList";
export type { HistorySessionListProps } from "./HistorySessionList";
export { useAgentHistory } from "./useAgentHistory";
export type { AgentHistoryState } from "./useAgentHistory";
export { restoreHistorySession } from "./restoreSession";
export { HistorySessionRow } from "./HistorySessionRow";
export type { HistorySessionRowProps } from "./HistorySessionRow";
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
