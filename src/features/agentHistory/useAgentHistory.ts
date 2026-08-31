// useAgentHistory.ts — agent 历史会话数据 hook（FE-04；F12 订阅化改造）
//
// 返回形状契约（跨 Stage 契约写死）：
//   { state, sessions, activeStatuses, rootPath, triggerNow, removeLocal }
//
// 设计要点：
// - sessions/state 真值源上移 backgroundTaskScheduler（F12）：本 hook 订阅
//   sessionRefresh 任务快照（状态机 idle|loading|ready|error 语义不变）；
//   首个订阅者出现 → 立即执行一轮（接管「挂载即扫」语义）+ 按配置频率定时刷新；
//   最后订阅者退订 → 停 interval（调度器全局单例与 UI 解耦，NavTree 卸载无碍，ADR-0001）
// - triggerNow() = 手动刷新（刷新钮）——与 tick 共用同一扫描执行体（规格 §1 单一执行体）
// - removeLocal 经调度器 applyLocal 透传（删除会话后本地移除列表项不重扫）
// - activeStatuses 实时跟随 TerminalRegistry（register/remove/sessionChange），
//   不重扫；rootPath 推导保持 hook 本地不变（activePageId → 所属 project）

import { useState, useEffect, useCallback } from "react";
import { backgroundTaskScheduler } from "../backgroundTasks";
import "../backgroundTasks/tasks"; // side-effect：任务注册触发点之一（硬约束 #13）
import type { TaskSnapshot } from "../backgroundTasks";
import { SESSION_REFRESH_TASK_ID } from "../../types/backgroundTasks";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { deriveActiveSessionStatuses } from "./historyModel";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";

/** 加载状态机：idle 初始未扫描 / loading 扫描中 / ready 成功 / error 失败 */
export type AgentHistoryState = "idle" | "loading" | "ready" | "error";

export function useAgentHistory() {
  const projects = useProjects((s) => s.projects);
  const activePageId = useLayout((s) => s.activePageId);

  // rootPath 推导：activePageId → 所属 project（照 useCommitStatus 先例）
  let rootPath: string | null = null;
  if (activePageId) {
    for (const [, proj] of Object.entries(projects)) {
      const activePage = proj.pages.find((p) => p.pageId === activePageId);
      if (activePage) {
        rootPath = activePage.cwd || proj.rootPath;
        break;
      }
    }
  }

  // 订阅调度器快照（首个订阅者 → 立即执行一轮 + 启动定时刷新；卸载退订）
  const [snapshot, setSnapshot] = useState<TaskSnapshot<AgentHistorySession[]>>({
    state: "idle",
    data: undefined,
  });
  useEffect(
    () =>
      backgroundTaskScheduler.subscribe<AgentHistorySession[]>(
        SESSION_REFRESH_TASK_ID,
        setSnapshot,
      ),
    [],
  );

  const [activeStatuses, setActiveStatuses] = useState<Map<string, AgentStatus>>(
    () => deriveActiveSessionStatuses(),
  );

  /** 手动刷新（刷新钮）——与 tick 同一执行体，仅触发来源不同（manual 失败置 error 态） */
  const triggerNow = useCallback(() => {
    void backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID);
  }, []);

  /** 局部删除：调用方已执行删除 IPC，此处仅即时移除列表项（不重扫） */
  const removeLocal = useCallback((sessionId: string) => {
    backgroundTaskScheduler.applyLocal<AgentHistorySession[]>(
      SESSION_REFRESH_TASK_ID,
      (prev) => (prev ?? []).filter((s) => s.sessionId !== sessionId),
    );
  }, []);

  // 订阅 TerminalRegistry：register/remove/sessionChange 任一事件 → 重算四态映射
  useEffect(() => {
    const unsubscribe = TerminalRegistry.subscribe(() => {
      setActiveStatuses(deriveActiveSessionStatuses());
    });
    return unsubscribe;
  }, []);

  return {
    state: snapshot.state,
    sessions: snapshot.data ?? [],
    activeStatuses,
    rootPath,
    triggerNow,
    removeLocal,
  };
}
