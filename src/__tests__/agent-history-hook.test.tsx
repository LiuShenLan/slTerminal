// agent-history-hook.test.tsx — FE-04 useAgentHistory 数据 hook 测试
//
// 覆盖：初始 idle / scan 成功 ready+sessions / scan 失败 error /
// removeLocal（不触发 scan）/
// TerminalRegistry.subscribe 事件驱动 activeStatuses 更新（复合键 cliId|sessionId——
// MC-313，旧数据无 cliId 按 CLAUDE_CLI_ID 回退）/ 卸载取消订阅 /
// generation 防竞（旧结果丢弃）/ rootPath 推导与切换不自动重扫。
//
// mock 模式：vi.hoisted() 共享状态 + 模块级 vi.mock（照 agent-status-hook.test.ts 先例）+
// 真实 Zustand stores（setState 种子）+ renderHook + act。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useAgentHistory } from "../features/agentHistory/useAgentHistory";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { resetProjectStores, seedExplorerProject } from "./helpers/workspace-setup";
import type { AgentHistorySession } from "../types/agentHistory";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => {
  const listeners = new Set<(e: { type: string; panelId: string }) => void>();
  const all = new Map<string, Record<string, unknown>>();
  const mockUnsubscribe = vi.fn();
  return {
    listeners,
    all,
    mockUnsubscribe,
    mockGetAll: vi.fn(() => new Map(all)),
    mockSubscribe: vi.fn((cb: (e: { type: string; panelId: string }) => void) => {
      listeners.add(cb);
      return mockUnsubscribe;
    }),
    mockScanHistory: vi.fn(),
  };
});

/** 通知全部 registry listener（模拟 register/remove/sessionChange 事件） */
function notifyListeners(event: { type: string; panelId: string }) {
  for (const fn of h.listeners) {
    fn(event);
  }
}

// ── mock IPC + TerminalRegistry（hook 仅消费 scanHistory / getAll / subscribe） ──
vi.mock("../ipc/agentHistory", () => ({
  scanHistory: h.mockScanHistory,
}));
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: h.mockGetAll,
    subscribe: h.mockSubscribe,
  },
}));

/** 最小 AgentHistorySession 工厂 */
function makeSession(
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: "session-1",
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId: "claude",
    ...overrides,
  };
}

beforeEach(() => {
  resetProjectStores();
  seedExplorerProject("C:\\project"); // 页面 cwd = "C:\project\src"
  h.listeners.clear();
  h.all.clear();
  h.mockUnsubscribe.mockClear();
  h.mockGetAll.mockClear();
  h.mockSubscribe.mockClear();
  h.mockScanHistory.mockReset();
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useAgentHistory 初始态", () => {
  it("初始 idle + 空 sessions + 空 activeStatuses，rootPath 推导自活跃页面 cwd", () => {
    const { result } = renderHook(() => useAgentHistory());
    expect(result.current.state).toBe("idle");
    expect(result.current.sessions).toEqual([]);
    expect(result.current.activeStatuses.size).toBe(0);
    expect(result.current.rootPath).toBe("C:\\project\\src");
    expect(h.mockScanHistory).not.toHaveBeenCalled();
  });

  it("页面 cwd 为空时回退项目 rootPath", () => {
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "测试项目",
          rootPath: "D:/root",
          pages: [
            {
              pageId: "page-1",
              name: "操作页面 1",
              layout: {},
              cwd: "",
              createdAt: 1,
              lastAccessedAt: 1,
            },
          ],
          activePageId: "page-1",
          version: 1,
        },
      },
    });
    const { result } = renderHook(() => useAgentHistory());
    expect(result.current.rootPath).toBe("D:/root");
  });

  it("activeStatuses 初值 = 挂载时注册表派生结果（Map<cliId|sessionId, status> 复合键，MC-313）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: {
        sessionId: "abc",
        usageSourcePath: "D:/proj/abc.jsonl",
        status: "working",
        lastEventAt: 1,
      },
    });
    const { result } = renderHook(() => useAgentHistory());
    expect(result.current.activeStatuses).toEqual(
      new Map([["claude|abc", "working"]]),
    );
  });
});

describe("useAgentHistory scan", () => {
  it("scan 成功 → ready + sessions", async () => {
    const sessions = [makeSession({ sessionId: "s1", cwd: "D:/a" })];
    h.mockScanHistory.mockResolvedValue(sessions);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await result.current.scan();
    });
    expect(result.current.state).toBe("ready");
    expect(result.current.sessions).toEqual(sessions);
  });

  it("scan 失败 → error + console.error 留痕，不静默吞", async () => {
    const error = new Error("scan failed");
    h.mockScanHistory.mockRejectedValue(error);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await result.current.scan();
    });
    expect(result.current.state).toBe("error");
    expect(console.error).toHaveBeenCalled();
  });

  it("error 后再次 scan 成功 → 恢复 ready", async () => {
    h.mockScanHistory.mockRejectedValueOnce(new Error("boom"));
    const sessions = [makeSession({ sessionId: "s2" })];
    h.mockScanHistory.mockResolvedValueOnce(sessions);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await result.current.scan();
    });
    expect(result.current.state).toBe("error");
    await act(async () => {
      await result.current.scan();
    });
    expect(result.current.state).toBe("ready");
    expect(result.current.sessions).toEqual(sessions);
  });

  it("generation 防竞：scan 进行中再触发，旧结果丢弃", async () => {
    let resolveFirst: ((v: AgentHistorySession[]) => void) | undefined;
    const first = [makeSession({ sessionId: "old" })];
    const second = [makeSession({ sessionId: "new" })];
    h.mockScanHistory
      .mockImplementationOnce(
        () => new Promise<AgentHistorySession[]>((r) => { resolveFirst = r; }),
      )
      .mockResolvedValueOnce(second);

    const { result } = renderHook(() => useAgentHistory());
    let p1: Promise<void>;
    act(() => {
      p1 = result.current.scan();
    });
    let p2: Promise<void>;
    act(() => {
      p2 = result.current.scan();
    });
    // 第二次先完成 → 新结果生效
    await act(async () => {
      await p2;
    });
    expect(result.current.state).toBe("ready");
    expect(result.current.sessions).toEqual(second);
    // 旧结果后到 → 被 generation 丢弃
    await act(async () => {
      resolveFirst!(first);
      await p1;
    });
    expect(result.current.sessions).toEqual(second);
  });

  it("rootPath 变化不自动重扫（历史区数据与项目弱相关）", () => {
    const { result, rerender } = renderHook(() => useAgentHistory());
    expect(h.mockScanHistory).not.toHaveBeenCalled();
    act(() => {
      useLayout.setState({ activePageId: null });
    });
    rerender();
    expect(result.current.rootPath).toBeNull();
    expect(h.mockScanHistory).not.toHaveBeenCalled();
  });
});

describe("useAgentHistory 局部更新", () => {
  async function seedSessions() {
    const sessions = [
      makeSession({ sessionId: "s1", title: "标题一" }),
      makeSession({ sessionId: "s2", title: "标题二" }),
    ];
    h.mockScanHistory.mockResolvedValue(sessions);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await result.current.scan();
    });
    return result;
  }

  it("removeLocal 删除条目且不触发 scan", async () => {
    const result = await seedSessions();
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    act(() => {
      result.current.removeLocal("s1");
    });
    expect(result.current.sessions.map((s) => s.sessionId)).toEqual(["s2"]);
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
  });

  it("removeLocal 不存在的 id → 列表不变", async () => {
    const result = await seedSessions();
    act(() => {
      result.current.removeLocal("nope");
    });
    expect(result.current.sessions).toHaveLength(2);
  });
});

describe("useAgentHistory 订阅", () => {
  it("注册表事件后 activeStatuses 重算（sessionChange 加/删，复合键）", () => {
    const { result } = renderHook(() => useAgentHistory());
    // sessionChange：claude 会话建立（sessionId + status 出现）
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: {
        sessionId: "aaa",
        status: "attention",
        lastEventAt: 1,
      },
    });
    act(() => {
      notifyListeners({ type: "sessionChange", panelId: "panel-1" });
    });
    expect(result.current.activeStatuses).toEqual(
      new Map([["claude|aaa", "attention"]]),
    );
    // sessionChange：会话结束（agentSession 清空）
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: null,
    });
    act(() => {
      notifyListeners({ type: "sessionChange", panelId: "panel-1" });
    });
    expect(result.current.activeStatuses.size).toBe(0);
  });

  it("订阅不触发 scan（规格 4.5：不重扫）", () => {
    renderHook(() => useAgentHistory());
    act(() => {
      notifyListeners({ type: "register", panelId: "panel-x" });
    });
    expect(h.mockScanHistory).not.toHaveBeenCalled();
  });

  it("卸载取消订阅", () => {
    const { unmount } = renderHook(() => useAgentHistory());
    expect(h.mockSubscribe).toHaveBeenCalledTimes(1);
    unmount();
    expect(h.mockUnsubscribe).toHaveBeenCalledTimes(1);
  });
});
