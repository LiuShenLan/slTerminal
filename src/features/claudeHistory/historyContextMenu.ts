// historyContextMenu.ts — 历史会话右键菜单策略（FE-07，照 commitContextMenu.ts 策略模式）
//
// 契约（workflows/stage-05-frontend-ui.js 脚本头，逐字）：
//   getHistoryContextMenuItems(
//     session: HistorySession,
//     opts: { active: boolean; orphan: boolean; noCwd: boolean;
//             onCopy(): void; onFork(): void; onDelete(): void; onRename(): void }
//   ): { label: string; disabled?: boolean; action(): void }[]
//
// 与 commitContextMenu 的差异：本策略不直接做 IPC——四项操作的 action 由调用方
// （HistorySessionList）经 opts 回调注入（onCopy/onFork/onDelete/onRename），
// 策略层只负责禁用态判定与菜单项构造。调用方回调内的流程（写剪贴板 / fork 恢复 /
// ask 确认后删除 + removeLocal / InputDialog 提交后重命名 + updateLocalTitle）见
// HistorySessionList.tsx 与 README 4.4 操作矩阵。
//
// 操作矩阵（README 4.4）：
//   复制恢复命令 —— 全行可用（含孤儿/运行中）
//   分支恢复     —— orphan / noCwd 禁用（孤儿目录已删除、无 cwd 无法编排）
//   删除         —— active 禁用（运行中文件句柄占用删除失败 + 外部进程续写幽灵文件）
//   重命名       —— 全行可用（ai-title 追加写与运行中写入无冲突）

import type { HistorySession } from "../../types/claudeHistory";

/** 菜单项（契约：label + disabled? + action） */
export interface HistoryMenuItem {
  label: string;
  /** true 时灰显不可点（无点击回调） */
  disabled?: boolean;
  action(): void;
}

/** 策略入参（契约逐字：active/orphan/noCwd + 四个操作回调） */
export interface HistoryContextMenuOpts {
  /** ⚡ 运行中会话标记 */
  active: boolean;
  /** ✗ 孤儿会话标记（cwd 目录已删除，不可恢复） */
  orphan: boolean;
  /** 无 cwd（恢复类操作禁用，不显示 ✗） */
  noCwd: boolean;
  /** 复制恢复命令到剪贴板 */
  onCopy(): void;
  /** 分支恢复（fork 编排） */
  onFork(): void;
  /** 删除（ask 确认 → IPC → 局部刷新） */
  onDelete(): void;
  /** 重命名（开 InputDialog → IPC → 局部刷新） */
  onRename(): void;
}

/**
 * 复制恢复命令构造（README 4.4）：
 * 有 cwd → `cd '<cwd>' && claude --resume <id>`（带单引号路径）；
 * 无 cwd → 仅 `claude --resume <id>`。
 */
export function buildResumeCommand(session: HistorySession): string {
  const resume = `claude --resume ${session.sessionId}`;
  return session.cwd ? `cd '${session.cwd}' && ${resume}` : resume;
}

/**
 * 右键菜单项构造（策略查询，禁用态按操作矩阵）。
 *
 * @param session 历史会话（契约参数——命令构造经 buildResumeCommand 由调用方完成，
 *                本函数仅做禁用态判定与回调接线）
 * @param opts    会话状态 + 四个操作回调（action 由调用方注入）
 */
export function getHistoryContextMenuItems(
  session: HistorySession,
  opts: HistoryContextMenuOpts,
): HistoryMenuItem[] {
  // session 为契约参数；复制命令的构造在调用方侧经 buildResumeCommand 完成
  void session;
  return [
    { label: "复制恢复命令", action: opts.onCopy },
    {
      label: "分支恢复",
      disabled: opts.orphan || opts.noCwd,
      action: opts.onFork,
    },
    { label: "删除", disabled: opts.active, action: opts.onDelete },
    { label: "重命名", action: opts.onRename },
  ];
}
