// useClaudeHistory.ts — claude 历史会话数据 hook（FE-04）
//
// 返回形状契约（跨 Stage 契约写死，Stage 05 消费）：
//   { state, sessions, activeStatuses, rootPath, scan, removeLocal }
//
// 设计要点：
// - 状态机 idle | loading | ready | error，初始 idle（未扫描）
// - scan() 由历史区首次展开与手动刷新按钮触发（规格 4.3.5）
// - removeLocal 纯本地即时刷新，不触发重扫（删除 IPC 由调用方先执行）
// - activeStatuses 实时跟随 TerminalRegistry（register/remove/sessionChange），
//   不重扫（规格 4.5）——Map<sessionId, 四态 status>，历史区行显示与活跃区一致（问题 2）
// - rootPath 变化不自动重扫——历史区数据与项目弱相关（checklist FE-04）

import { useState, useEffect, useRef, useCallback } from "react";
import { scanHistory } from "../../ipc/agentHistory";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { deriveActiveSessionStatuses } from "./historyModel";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";

/** 加载状态机：idle 初始未扫描 / loading 扫描中 / ready 成功 / error 失败 */
export type ClaudeHistoryState = "idle" | "loading" | "ready" | "error";

export function useClaudeHistory() {
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

  const [state, setState] = useState<ClaudeHistoryState>("idle");
  const [sessions, setSessions] = useState<AgentHistorySession[]>([]);
  const [activeStatuses, setActiveStatuses] = useState<
    Map<string, AgentStatus>
  >(() => deriveActiveSessionStatuses());
  const genRef = useRef(0);

  /** 扫描全部历史会话——generation 防竞：进行中再次触发，旧结果丢弃（照 useFileTree genRef 模式） */
  const scan = useCallback(async () => {
    const gen = ++genRef.current;
    setState("loading");
    try {
      const result = await scanHistory();
      if (gen !== genRef.current) return; // 过期结果丢弃
      setSessions(result);
      setState("ready");
    } catch (err) {
      console.error("[slTerminal] claude 历史扫描失败:", err);
      if (gen !== genRef.current) return;
      setState("error");
    }
  }, []);

  /** 局部删除：调用方已执行删除 IPC，此处仅即时移除列表项（不重扫） */
  const removeLocal = useCallback((sessionId: string) => {
    setSessions((prev) => prev.filter((s) => s.sessionId !== sessionId));
  }, []);

  // 订阅 TerminalRegistry：register/remove/sessionChange 任一事件 → 重算四态映射
  // （订阅回调只触发重算，不重扫；卸载时取消订阅）
  useEffect(() => {
    const unsubscribe = TerminalRegistry.subscribe(() => {
      setActiveStatuses(deriveActiveSessionStatuses());
    });
    return unsubscribe;
  }, []);

  return { state, sessions, activeStatuses, rootPath, scan, removeLocal };
}
