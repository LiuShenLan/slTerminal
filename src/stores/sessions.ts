// sessions — 会话状态存储
//
// 会话单点：会话真值（id/活动状态）只在此存储（硬约束 #8）。
// 面板只订阅，不自存。终端会话用模块级 Map（绕过 React 渲染循环）。

import { create } from "zustand";

export interface SessionInfo {
  sessionId: string;
  panelId: string;
  cwd?: string;
  /** 绑定的 worktree ID（路径），用于关联 git 上下文 */
  worktreeId?: string;
  isActive: boolean;
}

interface SessionsState {
  /** panelId → SessionInfo */
  sessions: Record<string, SessionInfo>;
  setSession: (panelId: string, info: SessionInfo) => void;
  removeSession: (panelId: string) => void;
  setActive: (panelId: string, active: boolean) => void;
}

export const useSessions = create<SessionsState>((set) => ({
  sessions: {},
  setSession: (panelId, info) =>
    set((state) => ({
      sessions: { ...state.sessions, [panelId]: info },
    })),
  removeSession: (panelId) =>
    set((state) => {
      const next = { ...state.sessions };
      delete next[panelId];
      return { sessions: next };
    }),
  setActive: (panelId, active) =>
    set((state) => {
      const existing = state.sessions[panelId];
      if (!existing) return state;
      return {
        sessions: {
          ...state.sessions,
          [panelId]: { ...existing, isActive: active },
        },
      };
    }),
}));

// ── 查询辅助函数（模块级，从 useSessions.getState() 取快照） ──

/** 获取属于指定 worktree 的所有会话 */
export function getSessionsByWorktree(worktreeId: string): SessionInfo[] {
  const { sessions } = useSessions.getState();
  return Object.values(sessions).filter(
    (s) => s.worktreeId === worktreeId,
  );
}

/** 获取属于指定 worktree 的所有面板 ID */
export function getPanelIdsByWorktree(worktreeId: string): string[] {
  return getSessionsByWorktree(worktreeId).map((s) => s.panelId);
}
