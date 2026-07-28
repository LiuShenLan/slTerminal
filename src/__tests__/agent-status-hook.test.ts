// agent-status-hook.test.ts — P2-TE-01：useAgentStatus 行建模新语义测试
//
// 行 = 运行中的 claude 会话（非全部终端）。
// 建行双通道：sessionChange（session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）。
// 删行三通道：sessionChange（session null）∨ SessionEnd/Exit ∨ remove。
// 初始扫描只建 claudeSession 非 null 的行；携 transcriptPath 时主动拉 contextUsage。
//
// mock 模式：vi.hoisted() 共享状态 + 模块级 vi.mock() +
// 真实 Zustand stores（setState 种子）+ renderHook + act/waitFor。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  capturedCallback,
  mockContextUsage,
  terminalMap,
  registryListeners,
  mockGetPageApi,
} = vi.hoisted(() => {
  const map = new Map<string, Record<string, unknown>>();
  const listeners = new Set<(e: { type: string; panelId: string }) => void>();
  return {
    // 保存 onHookEvent 注册的回调引用，供测试手动触发事件
    capturedCallback: {
      current: null as ((payload: Record<string, unknown>) => void) | null,
    },
    mockContextUsage: vi.fn(),
    terminalMap: map,
    registryListeners: listeners,
    mockGetPageApi: vi.fn(),
  };
});

/** 通知全部 registry listener */
function notifyListeners(event: { type: string; panelId: string }) {
  for (const fn of registryListeners) {
    fn(event);
  }
}

// ── mock TerminalRegistry（含 setClaudeSession + sessionChange 通知） ──
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn((panelId: string, entry: Record<string, unknown>) => {
      // 幂等覆盖：claudeSession 缺省时保留旧值（契约 1）
      const old = terminalMap.get(panelId);
      if (old && entry.claudeSession === undefined) {
        entry = { ...entry, claudeSession: old.claudeSession };
      }
      terminalMap.set(panelId, entry);
      notifyListeners({ type: "register", panelId });
    }),
    get: vi.fn((panelId: string) => terminalMap.get(panelId)),
    remove: vi.fn((panelId: string) => {
      const existed = terminalMap.delete(panelId);
      if (existed) {
        notifyListeners({ type: "remove", panelId });
      }
      return existed;
    }),
    has: vi.fn((panelId: string) => terminalMap.has(panelId)),
    getAll: vi.fn(() => new Map(terminalMap)),
    // setClaudeSession：merge 语义 + null 清空 + 不存在 no-op
    setClaudeSession: vi.fn(
      (panelId: string, patch: Record<string, unknown> | null) => {
        const entry = terminalMap.get(panelId);
        if (!entry) return; // no-op，不 notify

        if (patch === null) {
          entry.claudeSession = null;
        } else {
          const prev = entry.claudeSession as Record<string, unknown> | null | undefined;
          entry.claudeSession = {
            transcriptPath:
              patch.transcriptPath !== undefined
                ? patch.transcriptPath
                : prev?.transcriptPath,
            matchedCommand:
              patch.matchedCommand !== undefined
                ? patch.matchedCommand
                : prev?.matchedCommand,
            lastEventAt: patch.lastEventAt ?? Date.now(),
          };
        }

        notifyListeners({ type: "sessionChange", panelId });
      },
    ),
    subscribe: vi.fn(
      (listener: (e: { type: string; panelId: string }) => void) => {
        registryListeners.add(listener);
        return () => {
          registryListeners.delete(listener);
        };
      },
    ),
    _reset: vi.fn(() => {
      terminalMap.clear();
      registryListeners.clear();
    }),
    _size: vi.fn(() => terminalMap.size),
    _dump: vi.fn(() => Array.from(terminalMap.keys())),
  },
}));

// ── mock workspace/pageApis（getPageApi 供标题查找） ──
vi.mock("../workspace/pageApis", () => ({
  getPageApi: (pageId: string) => mockGetPageApi(pageId),
  registerPageApi: vi.fn(),
  unregisterPageApi: vi.fn(),
  switchToPageShared: vi.fn(),
  switchToPageAndFocus: vi.fn(),
}));

// ── mock ipc/hooks（覆盖 setup.ts 全局 mock，捕获回调 + 可控 contextUsage） ──
vi.mock("../ipc/hooks", () => ({
  onHookEvent: vi.fn((cb: (payload: Record<string, unknown>) => void) => {
    capturedCallback.current = cb;
    return () => {
      capturedCallback.current = null;
    };
  }),
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  contextUsage: ((path: string) => mockContextUsage(path)) as any,
  inject: () =>
    Promise.resolve({ status: "notInjected" as const, version: null }),
  uninstall: () => Promise.resolve(),
  getInjectionStatus: () =>
    Promise.resolve({ status: "notInjected" as const, version: null }),
}));

import { renderHook, act, waitFor } from "@testing-library/react";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { useAgentStatus } from "../features/agentStatus/useAgentStatus";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { onHookEvent } from "../ipc/hooks";

// ═══════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════

/** 种子 stores：创建项目 + 操作页面 + 激活页 */
function seedProject(
  projectId = "proj-1",
  pageId = "page1",
  rootPath = "C:/test",
) {
  useProjects.setState({
    projects: {
      [projectId]: {
        projectId,
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId,
            name: "操作页面 1",
            layout: {},
            cwd: undefined,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: pageId,
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { [projectId]: true },
  });
  useLayout.setState({ activePageId: pageId });
  return { projectId, pageId };
}

/**
 * 在 mock TerminalRegistry 中注册一个终端（直接操作 Map，不触发 subscribe 通知）。
 * claudeSession 为 null 表示纯 shell（无会话），非 null 表示运行中的 claude 会话。
 */
function registerTerminal(
  panelId: string,
  claudeSession?: Record<string, unknown> | null,
) {
  const entry: Record<string, unknown> = {
    term: {} as unknown,
    sessionId: `session-${panelId}`,
    webglAddon: null,
    fitAddon: {} as unknown,
  };
  if (claudeSession !== undefined) {
    entry.claudeSession = claudeSession;
  }
  terminalMap.set(panelId, entry);
}

/**
 * 通过 TerminalRegistry API 注册终端（触发 register 通知）。
 * claudeSession 为 null 表示纯 shell。
 */
function registerTerminalWithNotify(
  panelId: string,
  claudeSession?: Record<string, unknown> | null,
) {
  const entry: Record<string, unknown> = {
    term: {} as unknown,
    sessionId: `session-${panelId}`,
    webglAddon: null,
    fitAddon: {} as unknown,
  };
  if (claudeSession !== undefined) {
    entry.claudeSession = claudeSession;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (TerminalRegistry as any).register(panelId, entry);
}

/** 构造 HookEventPayload（字段对齐 src/ipc/hooks.ts HookEventPayload） */
function makePayload(
  overrides: Partial<{
    panelId: string;
    event: string;
    timestamp: number;
    sessionId: string;
    transcriptPath: string;
    cwd: string;
    toolName: string | null;
    notificationType: string | null;
  }> = {},
) {
  return {
    panelId: "terminal-page1-0",
    event: "PreToolUse",
    timestamp: Date.now(),
    sessionId: "s1",
    transcriptPath: "",
    cwd: "C:/test",
    toolName: null,
    notificationType: null,
    ...overrides,
  };
}

/** 构造 claudeSession 对象 */
function makeSession(overrides: {
  lastEventAt?: number;
  matchedCommand?: string;
  transcriptPath?: string;
} = {}): Record<string, unknown> {
  return {
    lastEventAt: overrides.lastEventAt ?? Date.now(),
    matchedCommand: overrides.matchedCommand ?? "claude",
    transcriptPath: overrides.transcriptPath,
  };
}

// ═══════════════════════════════════════════════════════════════════
// 测试用例
// ═══════════════════════════════════════════════════════════════════

describe("useAgentStatus（行建模新语义）", () => {
  beforeEach(() => {
    // 重置 stores
    useLayout.setState({ activePageId: null });
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    // 重置 terminalMap + listeners
    terminalMap.clear();
    registryListeners.clear();
    // 重置 hooks mock
    capturedCallback.current = null;
    mockContextUsage.mockReset();
    mockContextUsage.mockResolvedValue(null);
    // 重置 pageApis mock（默认无 api）
    mockGetPageApi.mockReset();
    mockGetPageApi.mockReturnValue(undefined);
  });

  afterEach(() => {
    capturedCallback.current = null;
  });

  // ──────────────────────────────────────────────────
  // 状态机派生
  // ──────────────────────────────────────────────────

  it("无活跃项目时返回 no-root 态且 rows 为空", () => {
    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.state).toEqual({ kind: "no-root" });
    expect(result.current.rows).toEqual([]);
  });

  it("有项目但无终端时返回 empty 态", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.state).toEqual({ kind: "empty" });
    expect(result.current.rows).toEqual([]);
  });

  it("有项目且终端全为纯 shell（claudeSession 为 null）→ 返回 empty 态", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null); // 纯 shell，无 claude 会话

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.state).toEqual({ kind: "empty" });
    expect(result.current.rows).toEqual([]);
  });

  // ──────────────────────────────────────────────────
  // 初始扫描——只建 claudeSession 非 null 的行
  // ──────────────────────────────────────────────────

  it("初始扫描：claudeSession 非 null → 建行", () => {
    const { pageId } = seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].pageId).toBe(pageId);
    expect(result.current.rows[0].projectId).toBe("proj-1");
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].lastEventAt).toBe(1000);
    expect(result.current.state).toEqual({ kind: "ready" });
  });

  it("初始扫描：混合终端——纯 shell 不建行，活会话建行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 2000 })); // 活会话
    registerTerminal("terminal-page1-1", null);  // 纯 shell——不建行
    registerTerminal("terminal-page1-2");         // undefined claudeSession——不建行

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
  });

  it("初始扫描过滤非当前项目的 panelId", () => {
    seedProject("proj-1", "page1");
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));
    registerTerminal("terminal-page2-0", makeSession({ lastEventAt: 2000 })); // 其他项目——过滤

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
  });

  it("初始扫描携 transcriptPath 时主动拉 contextUsage（修复问题 2b）", async () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({
      lastEventAt: 1000,
      transcriptPath: "/path/to/transcript.jsonl",
    }));

    mockContextUsage.mockResolvedValue({
      inputTokens: 5000,
      outputTokens: 2000,
    });

    renderHook(() => useAgentStatus());

    // 同步验证：contextUsage 在初始扫描时被调用（StrictMode 下 effect 双次触发，故不断言精确次数）
    expect(mockContextUsage).toHaveBeenCalledWith("/path/to/transcript.jsonl");
  });

  it("初始扫描无 transcriptPath → 不调 contextUsage", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({
      lastEventAt: 1000,
      // 无 transcriptPath
    }));

    renderHook(() => useAgentStatus());

    expect(mockContextUsage).not.toHaveBeenCalled();
  });

  // ──────────────────────────────────────────────────
  // sessionChange 建行（双通道之一）
  // ──────────────────────────────────────────────────

  it("sessionChange（非 null）→ 建行（带 matchedCommand）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null); // 先注册为纯 shell

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0); // 纯 shell 无行

    // sessionChange 触发——设置 claudeSession 非 null
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setClaudeSession("terminal-page1-0", {
        matchedCommand: "claude",
      });
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
  });

  it("sessionChange 建行幂等——行已存在时跳过不建重复行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // 再次 sessionChange（同一 panelId）——不应建重复行
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setClaudeSession("terminal-page1-0", {
        matchedCommand: "claude",
      });
    });

    expect(result.current.rows).toHaveLength(1);
  });

  it("sessionChange 建行携带 transcriptPath → 主动拉 usage", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null);

    mockContextUsage.mockResolvedValue({
      inputTokens: 8000,
      outputTokens: 3000,
    });

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setClaudeSession("terminal-page1-0", {
        matchedCommand: "claude",
        transcriptPath: "/t.json",
      });
    });

    expect(mockContextUsage).toHaveBeenCalledWith("/t.json");
    expect(result.current.rows).toHaveLength(1);
  });

  // ──────────────────────────────────────────────────
  // register 不建行（session null 时）——语义反转
  // ──────────────────────────────────────────────────

  it("register 触发通知但 claudeSession 为 null → 不建行", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      registerTerminalWithNotify("terminal-page1-0", null);
    });

    // register 事件不建行——建行由 sessionChange（非 null）负责
    expect(result.current.rows).toHaveLength(0);
  });

  // ──────────────────────────────────────────────────
  // hook 事件建行（双通道之二——行不存在时）
  // ──────────────────────────────────────────────────

  it("hook 事件（非 SessionEnd/Exit）且行不存在 → 建行", () => {
    seedProject();
    // 不预注册 terminal——hook 事件独立建行

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          event: "SessionStart",
          timestamp: 1000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
  });

  it("hook 事件且行已存在 → 更新不建新行（幂等）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 500 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          timestamp: 1000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].status).toBe("working"); // 更新为 working
    expect(result.current.rows[0].lastEventAt).toBe(1000);
  });

  // ──────────────────────────────────────────────────
  // 删行——三通道
  // ──────────────────────────────────────────────────

  it("sessionChange(null) → 删行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setClaudeSession("terminal-page1-0", null);
    });

    expect(result.current.rows).toHaveLength(0);
    expect(result.current.state).toEqual({ kind: "empty" });
  });

  it("SessionEnd hook 事件 → 删行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "SessionEnd", timestamp: 5000 }),
      );
    });

    expect(result.current.rows).toHaveLength(0);
  });

  it("Exit hook 事件 → 删行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "Exit", timestamp: 5000 }),
      );
    });

    expect(result.current.rows).toHaveLength(0);
  });

  it("remove 事件 → 删行（deps [] 稳定订阅——remove 事件不丢失，R4 根因修复）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).remove("terminal-page1-0");
    });

    expect(result.current.rows).toHaveLength(0);
  });

  it("SessionEnd 到达时行不存在（hook 事件建的行）→ 无副作用", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());

    // 通过 hook 事件建行
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          event: "SessionStart",
          timestamp: 1000,
        }),
      );
    });
    expect(result.current.rows).toHaveLength(1);

    // SessionEnd 删行
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          event: "SessionEnd",
          timestamp: 2000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(0);
  });

  // ──────────────────────────────────────────────────
  // 过滤非当前项目事件
  // ──────────────────────────────────────────────────

  it("事件来自其他项目 pageId → 不进入当前项目 rows", () => {
    seedProject("proj-1", "page1");
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // 发送 page2 的事件（page2 不在当前项目）
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page2-0",
          event: "PreToolUse",
          timestamp: 2000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
  });

  it("sessionChange 来自其他项目 → 不进入当前项目 rows", () => {
    seedProject("proj-1", "page1");
    registerTerminal("terminal-page2-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    // sessionChange 对 page2——不建行
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setClaudeSession("terminal-page2-0", {
        matchedCommand: "claude",
      });
    });

    expect(result.current.rows).toHaveLength(0);
  });

  // ──────────────────────────────────────────────────
  // reconcile 对账——行在 registry 中不存在或 session 为 null → 移除
  // ──────────────────────────────────────────────────

  it("reconcile 对账：行在 registry 中不存在 → 项目切换时被移除", () => {
    // 先种子项目 A
    seedProject("proj-1", "pageA", "C:/projA");
    registerTerminal("terminal-pageA-0", makeSession({ lastEventAt: 1000 }));

    const { result, rerender } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // 切换到项目 B（无 pageA 的终端）
    useProjects.setState({
      projects: {
        "proj-2": {
          projectId: "proj-2",
          name: "项目 B",
          rootPath: "C:/projB",
          pages: [{ pageId: "pageB", name: "页面 B", layout: {}, cwd: undefined, createdAt: 1, lastAccessedAt: 1 }],
          activePageId: "pageB",
          version: 1,
        },
      },
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: { "proj-2": true },
    });
    useLayout.setState({ activePageId: "pageB" });
    rerender();

    // 项目 B 无终端 → empty
    expect(result.current.rows).toHaveLength(0);
    expect(result.current.state).toEqual({ kind: "empty" });
  });

  // ──────────────────────────────────────────────────
  // 多行按 lastEventAt 倒序
  // ──────────────────────────────────────────────────

  it("多行时按 lastEventAt 倒序排列", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));
    registerTerminal("terminal-page1-1", makeSession({ lastEventAt: 2000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(2);

    // 倒序：较晚时间在前
    expect(result.current.rows[0].panelId).toBe("terminal-page1-1");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
    expect(result.current.rows[1].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[1].lastEventAt).toBe(1000);
  });

  // ──────────────────────────────────────────────────
  // contextUsage 拉取
  // ──────────────────────────────────────────────────

  it("事件含 transcriptPath → 调用 contextUsage 拉取用量", async () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    mockContextUsage.mockResolvedValue({
      inputTokens: 12000,
      outputTokens: 4500,
    });

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          transcriptPath: "/path/to/transcript.jsonl",
        }),
      );
    });

    expect(mockContextUsage).toHaveBeenCalledWith("/path/to/transcript.jsonl");

    await waitFor(() => {
      expect(result.current.rows[0].usage).toEqual({
        inputTokens: 12000,
        outputTokens: 4500,
      });
    });
  });

  it("contextUsage 返回 null → usage 字段为 null", async () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    mockContextUsage.mockResolvedValue(null);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          transcriptPath: "/path/to/transcript.jsonl",
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.rows[0].usage).toBeNull();
    });
  });

  it("contextUsage 报错 → 行仍存在且 usage 保持为 undefined（降级不崩）", async () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    mockContextUsage.mockRejectedValue(new Error("文件不存在"));

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          transcriptPath: "/bad/path.jsonl",
        }),
      );
    });

    // 降级：行仍存在，usage 保持 undefined（初始值）
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].usage).toBeUndefined();
  });

  it("无 transcriptPath 的事件不触发 contextUsage", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          transcriptPath: "",
        }),
      );
    });

    expect(mockContextUsage).not.toHaveBeenCalled();
  });

  // ──────────────────────────────────────────────────
  // 行更新细节
  // ──────────────────────────────────────────────────

  it("重复事件更新同一行——不创建重复条目", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 1000 }),
      );
    });

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PostToolUse", timestamp: 2000 }),
      );
    });

    // 应只有一行（同一 panelId），状态和时间戳已更新
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].status).toBe("working");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  it("Stop 后新事件也能更新该行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    // Stop → done
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "Stop", timestamp: 1000 }),
      );
    });
    expect(result.current.rows[0].status).toBe("done");

    // 新事件 → 更新为 working
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 2000 }),
      );
    });
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].status).toBe("working");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  // ──────────────────────────────────────────────────
  // null 状态不覆盖已有行状态
  // ──────────────────────────────────────────────────

  it("⚡ 行收到 Notification(auth_success) 后状态仍为 ⚡（null 不覆盖）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    // PreToolUse → working (⚡)
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 1000 }),
      );
    });
    expect(result.current.rows[0].status).toBe("working");

    // Notification(auth_success) → eventToStatus 返回 null
    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "Notification",
          notificationType: "auth_success",
          timestamp: 2000,
        }),
      );
    });

    // 状态应保持 working，不被 null 覆盖
    expect(result.current.rows[0].status).toBe("working");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  it("未知事件（eventToStatus 返回 null）不覆盖状态", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows[0].status).toBe("attention");

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "UnknownEvent",
          timestamp: 2000,
        }),
      );
    });

    // 状态保持 attention，不被 null 覆盖
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  // ──────────────────────────────────────────────────
  // FE-06: onHookEvent 订阅不因渲染重建
  // ──────────────────────────────────────────────────

  it("行更新触发重渲染后 onHookEvent 调用次数不增（deps [] 稳定订阅）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    const callCountBefore = (onHookEvent as ReturnType<typeof vi.fn>).mock.calls.length;

    // 发送事件 → 行更新 → 重渲染
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 1000 }),
      );
    });

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PostToolUse", timestamp: 2000 }),
      );
    });

    // onHookEvent 不应被重新调用（handleHookEvent deps [] 稳定）
    expect((onHookEvent as ReturnType<typeof vi.fn>).mock.calls.length).toBe(callCountBefore);
    expect(result.current.rows).toHaveLength(1);
  });

  // ──────────────────────────────────────────────────
  // FE-04: 行标题查 dockviewApi
  // ──────────────────────────────────────────────────

  it("getPageApi 返回带 title 面板 → 行标题为页签标题", () => {
    mockGetPageApi.mockImplementation(() => ({
      getPanel: (panelId: string) =>
        panelId === "terminal-page1-0"
          ? { title: "我的终端", focus: vi.fn() }
          : undefined,
    }));

    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows[0].title).toBe("我的终端");
  });

  it("getPageApi 返回 undefined → 回退标题 终端 {pageId}", () => {
    mockGetPageApi.mockReturnValue(undefined);

    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows[0].title).toBe("终端 page1");
  });

  // ──────────────────────────────────────────────────
  // remove 不存在的 panelId 不抛异常
  // ──────────────────────────────────────────────────

  it("remove 不存在的 panelId 不抛异常", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    renderHook(() => useAgentStatus());

    expect(() => {
      act(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (TerminalRegistry as any).remove("terminal-nonexistent-0");
      });
    }).not.toThrow();
  });
});
