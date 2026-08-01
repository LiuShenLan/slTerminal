// index.ts — claude 历史会话模块 barrel export（照 commit/index.ts 模式）
// 导出视图所需公共 API：组合件、列表、行、弹窗、数据 hook、恢复编排、
// 右键菜单策略、模型纯函数。

export { ClaudeHistorySections } from "./ClaudeHistorySections";
export type { ClaudeHistorySectionsProps } from "./ClaudeHistorySections";
export { HistorySessionList } from "./HistorySessionList";
export type { HistorySessionListProps } from "./HistorySessionList";
export { useClaudeHistory } from "./useClaudeHistory";
export type { ClaudeHistoryState } from "./useClaudeHistory";
export { restoreHistorySession } from "./restoreSession";
export { HistorySessionRow } from "./HistorySessionRow";
export type { HistorySessionRowProps } from "./HistorySessionRow";
export { InputDialog } from "./InputDialog";
export type { InputDialogProps } from "./InputDialog";
export { getHistoryContextMenuItems, buildResumeCommand } from "./historyContextMenu";
export type { HistoryMenuItem, HistoryContextMenuOpts } from "./historyContextMenu";
export {
  UNKNOWN_CWD_KEY,
  isCurrentProject,
  groupByCwd,
  matchesSearch,
  formatRelativeTime,
  deriveActiveSessionIds,
} from "./historyModel";
export type { CwdGroup } from "./historyModel";
