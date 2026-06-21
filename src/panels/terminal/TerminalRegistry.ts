// TerminalRegistry — 模块级 PTY 会话注册表
//
// 脱离 React 渲染循环 —— 终端实例在面板卸载后仍存活。
// 用途：页面切换时保留 PTY 进程（不 kill），页面/workspace 删除时级联清理。

interface PtySessionEntry {
  sessionId: string;
  panelId: string;
  cwd?: string;
  createdAt: number;
}

const registry = new Map<string, PtySessionEntry>();

export const TerminalRegistry = {
  /** spawn 成功后注册 */
  register(panelId: string, sessionId: string, cwd?: string): void {
    registry.set(panelId, { sessionId, panelId, cwd, createdAt: Date.now() });
  },

  /** 面板挂载时查询有无存活 session */
  get(panelId: string): PtySessionEntry | undefined {
    return registry.get(panelId);
  },

  /** 用户主动关闭面板 — 杀 PTY + 注销 */
  async destroy(panelId: string): Promise<void> {
    const entry = registry.get(panelId);
    if (entry) {
      try {
        await import("../../ipc/pty").then((m) =>
          m.kill(entry!.sessionId),
        );
      } catch {
        // PTY 可能已被销毁
      }
      registry.delete(panelId);
    }
  },

  /** 删除 worktree 时级联清理所有关联 session */
  getByWorktree(worktreePath: string): PtySessionEntry[] {
    const results: PtySessionEntry[] = [];
    for (const [, entry] of registry) {
      if (entry.cwd?.startsWith(worktreePath)) {
        results.push(entry);
      }
    }
    return results;
  },

  /** 获取所有已注册 panelId */
  getAll(): Map<string, PtySessionEntry> {
    return registry;
  },
};
