// useClaudeHistory.ts — claude 历史会话数据 hook（FE-04）
//
// 返回形状契约（跨 Stage 契约写死，Stage 05 消费）：
//   { state, sessions, activeIds, rootPath, scan, removeLocal, updateLocalTitle }
//
// 设计要点：
// - 状态机 idle | loading | ready | error，初始 idle（未扫描）
// - scan() 由历史区首次展开与手动刷新按钮触发（规格 4.3.5）
// - removeLocal / updateLocalTitle 纯本地即时刷新，不触发重扫
//   （删除/重命名 IPC 由调用方先执行，成功后调本函数同步 UI）
// - activeIds 实时跟随 TerminalRegistry（register/remove/sessionChange），不重扫（规格 4.5）
// - rootPath 变化不自动重扫——历史区数据与项目弱相关（checklist FE-04）

import { useState, useEffect, useRef, useCallback } from "react";
import { scanHistory } from "../../ipc/claudeHistory";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { deriveActiveSessionIds } from "./historyModel";
import type { HistorySession } from "../../types/claudeHistory";

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
  const [sessions, setSessions] = useState<HistorySession[]>([]);
  const [activeIds, setActiveIds] = useState<Set<string>>(() =>
    deriveActiveSessionIds(),
  );
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

  /** 局部重命名：调用方已执行重命名 IPC，此处仅即时更新标题（不重扫） */
  const updateLocalTitle = useCallback((sessionId: string, title: string) => {
    setSessions((prev) =>
      prev.map((s) =>
        s.sessionId === sessionId
          ? { ...s, title, titleSource: "customTitle" as const }
          : s,
      ),
    );
  }, []);

  // 订阅 TerminalRegistry：register/remove/sessionChange 任一事件 → 重算 ⚡ 集合
  // （订阅回调只触发重算，不重扫；卸载时取消订阅）
  useEffect(() => {
    const unsubscribe = TerminalRegistry.subscribe(() => {
      setActiveIds(deriveActiveSessionIds());
    });
    return unsubscribe;
  }, []);

  return { state, sessions, activeIds, rootPath, scan, removeLocal, updateLocalTitle };
}
