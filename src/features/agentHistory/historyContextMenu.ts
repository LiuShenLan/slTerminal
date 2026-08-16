// historyContextMenu.ts — 历史会话右键菜单策略（FE-07，照 commitContextMenu.ts 策略模式）
//
// 契约（workflows/stage-05-frontend-ui.js 脚本头，逐字）：
//   getHistoryContextMenuItems(
//     session: AgentHistorySession,
//     opts: { active: boolean; orphan: boolean; noCwd: boolean;
//             onCopy(): void; onFork(): void; onDelete(): void }
//   ): { label: string; disabled?: boolean; action(): void }[]
//
// 与 commitContextMenu 的差异：本策略不直接做 IPC——三项操作的 action 由调用方
// （HistorySessionList）经 opts 回调注入（onCopy/onFork/onDelete），
// 策略层只负责禁用态判定与菜单项构造。调用方回调内的流程（写剪贴板 / fork 恢复 /
// ask 确认后删除 + removeLocal）见 HistorySessionList.tsx 与 README 4.4 操作矩阵。
//
// 操作矩阵（README 4.4，重命名功能已整体移除——问题 7 修复）：
//   复制恢复命令 —— 全行可用（含孤儿/运行中）
//   分支恢复     —— orphan / noCwd 禁用（孤儿目录已删除、无 cwd 无法编排）；
//                   profile.history.supportsFork=false（能力未声明）→ 不展示该菜单项（MC-316）
//   删除         —— active 禁用（运行中文件句柄占用删除失败 + 外部进程续写幽灵文件）
//
// 命令构造（buildResumeCommand）委托 profile.history.buildResumeCommand（MC-316）——
// 命令形态（含 cwd 单引号路径等 CLI 专属限制）由各 CLI 的 history 能力实现负责。

import type { AgentHistorySession } from "../../types/agentHistory";
import { cliProfileRegistry } from "../cliProfiles";

/** 菜单项（契约：label + disabled? + action） */
export interface HistoryMenuItem {
  label: string;
  /** true 时灰显不可点（无点击回调） */
  disabled?: boolean;
  action(): void;
}

/** 策略入参（契约逐字：active/orphan/noCwd + 三个操作回调） */
export interface HistoryContextMenuOpts {
  /** working 运行中会话标记 */
  active: boolean;
  /** 孤儿会话标记（cwd 目录已删除，不可恢复；IconClose 标记） */
  orphan: boolean;
  /** 无 cwd（恢复类操作禁用，不显示孤儿标记） */
  noCwd: boolean;
  /** 复制恢复命令到剪贴板 */
  onCopy(): void;
  /** 分支恢复（fork 编排） */
  onFork(): void;
  /** 删除（ask 确认 → IPC → 局部刷新） */
  onDelete(): void;
}

/**
 * 复制恢复命令构造（README 4.4）——委托 profile.history.buildResumeCommand（MC-316）：
 * 按 session.cliId 查 profile；history 能力未声明（或 profile 未注册）→ 空串
 * （复制空串无害的优雅降级）；具体命令形态由各 CLI 的 history 能力实现负责。
 */
export function buildResumeCommand(session: AgentHistorySession): string {
  return (
    cliProfileRegistry
      .get(session.cliId)
      ?.capabilities?.history?.buildResumeCommand(session) ?? ""
  );
}

/**
 * 右键菜单项构造（策略查询，禁用态按操作矩阵）。
 *
 * @param session 历史会话（契约参数——经 cliId 查 profile 取分支恢复能力与命令构造）
 * @param opts    会话状态 + 四个操作回调（action 由调用方注入）
 */
export function getHistoryContextMenuItems(
  session: AgentHistorySession,
  opts: HistoryContextMenuOpts,
): HistoryMenuItem[] {
  // 分支恢复能力（MC-316）：profile.history.supportsFork 缺省 false——能力未声明
  // = 该 CLI 不支持分支恢复 → 不展示「分支恢复」菜单项
  const supportsFork =
    cliProfileRegistry
      .get(session.cliId)
      ?.capabilities?.history?.supportsFork ?? false;

  const items: HistoryMenuItem[] = [
    { label: "复制恢复命令", action: opts.onCopy },
  ];
  if (supportsFork) {
    items.push({
      label: "分支恢复",
      disabled: opts.orphan || opts.noCwd,
      action: opts.onFork,
    });
  }
  items.push({ label: "删除", disabled: opts.active, action: opts.onDelete });
  return items;
}
