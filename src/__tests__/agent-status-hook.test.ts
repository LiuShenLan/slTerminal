// agent-status-hook.test.ts — P2-TE-01：useAgentStatus 行建模新语义测试
//
// 行 = 运行中的编码 CLI 会话（非全部终端）。
// 建行双通道：sessionChange（session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）。
// 删行三通道：sessionChange（session null）∨ SessionEnd/Exit ∨ remove。
// 初始扫描只建 agentSession 非 null 的行；ContextUsage 信号事件更新行 usage
// （行存在才更新，不建行/删行/不动状态——官方 used_percentage 口径）。
// 行 cliId（MC-410）：hook 事件通道按 MC-205 三级解析写入；OSC 133 通道经
// agentSession.cliId（setAgentSession sessionChange 自然驱动）。
//
// mock 模式：vi.hoisted() 共享状态 + 模块级 vi.mock() +
// 真实 Zustand stores（setState 种子）+ renderHook + act/waitFor。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  capturedCallback,
  terminalMap,
  registryListeners,
  mockGetPageApi,
} = vi.hoisted(() => {
  const map = new Map<string, Record<string, unknown>>();
  const listeners = new Set<(e: { type: string; panelId: string }) => void>();
  return {
    // 保存 onAgentEvent 注册的回调引用，供测试手动触发事件
    capturedCallback: {
      current: null as ((payload: Record<string, unknown>) => void) | null,
    },
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

// ── mock TerminalRegistry（含 setAgentSession + sessionChange 通知） ──
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn((panelId: string, entry: Record<string, unknown>) => {
      // 幂等覆盖：agentSession 缺省时保留旧值（契约 1）
      const old = terminalMap.get(panelId);
      if (old && entry.agentSession === undefined) {
        entry = { ...entry, agentSession: old.agentSession };
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
    // setAgentSession：merge 语义 + null 清空 + 不存在 no-op
    setAgentSession: vi.fn(
      (panelId: string, patch: Record<string, unknown> | null) => {
        const entry = terminalMap.get(panelId);
        if (!entry) return; // no-op，不 notify

        if (patch === null) {
          entry.agentSession = null;
        } else {
          const prev = entry.agentSession as Record<string, unknown> | null | undefined;
          entry.agentSession = {
            sessionId:
              patch.sessionId !== undefined
                ? patch.sessionId
                : prev?.sessionId,
            usageSourcePath:
              patch.usageSourcePath !== undefined
                ? patch.usageSourcePath
                : prev?.usageSourcePath,
            matchedCommand:
              patch.matchedCommand !== undefined
                ? patch.matchedCommand
                : prev?.matchedCommand,
            cliId:
              patch.cliId !== undefined ? patch.cliId : prev?.cliId,
            status:
              patch.status !== undefined ? patch.status : prev?.status,
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

// ── mock ipc/agentHooks（覆盖 setup.ts 全局 mock，捕获回调） ──
vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: vi.fn((cb: (payload: Record<string, unknown>) => void) => {
    capturedCallback.current = cb;
    return () => {
      capturedCallback.current = null;
    };
  }),
  inject: () =>
    Promise.resolve({ status: "notInjected" as const, version: null }),
  uninstall: () => Promise.resolve(),
  getInjectionStatus: () =>
    Promise.resolve({ status: "notInjected" as const, version: null }),
  restoreStatusline: () => Promise.resolve(),
}));

import { renderHook, act } from "@testing-library/react";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { useAgentStatus } from "../features/agentStatus/useAgentStatus";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { onAgentEvent } from "../ipc/agentHooks";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";

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
 * agentSession 为 null 表示纯 shell（无会话），非 null 表示运行中的编码 CLI 会话。
 */
function registerTerminal(
  panelId: string,
  agentSession?: Record<string, unknown> | null,
) {
  const entry: Record<string, unknown> = {
    term: {} as unknown,
    sessionId: `session-${panelId}`,
    webglAddon: null,
    fitAddon: {} as unknown,
  };
  if (agentSession !== undefined) {
    entry.agentSession = agentSession;
  }
  terminalMap.set(panelId, entry);
}

/**
 * 通过 TerminalRegistry API 注册终端（触发 register 通知）。
 * agentSession 为 null 表示纯 shell。
 */
function registerTerminalWithNotify(
  panelId: string,
  agentSession?: Record<string, unknown> | null,
) {
  const entry: Record<string, unknown> = {
    term: {} as unknown,
    sessionId: `session-${panelId}`,
    webglAddon: null,
    fitAddon: {} as unknown,
  };
  if (agentSession !== undefined) {
    entry.agentSession = agentSession;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (TerminalRegistry as any).register(panelId, entry);
}

/** 构造 AgentEventPayload（字段对齐 src/types/agent.ts AgentEventPayload，含可选 cliId/usedPercentage） */
function makePayload(
  overrides: Partial<{
    panelId: string;
    event: string;
    timestamp: number;
    sessionId: string;
    usageSourcePath: string;
    cwd: string;
    toolName: string | null;
    notificationType: string | null;
    cliId?: string;
    usedPercentage?: number;
  }> = {},
) {
  return {
    panelId: "terminal-page1-0",
    event: "PreToolUse",
    timestamp: Date.now(),
    sessionId: "s1",
    usageSourcePath: "",
    cwd: "C:/test",
    toolName: null,
    notificationType: null,
    ...overrides,
  };
}

/** 构造 agentSession 对象（sessionId/status/cliId 缺省 undefined——matchedCommand-only 形态） */
function makeSession(overrides: {
  lastEventAt?: number;
  matchedCommand?: string;
  usageSourcePath?: string;
  sessionId?: string;
  status?: string;
  cliId?: string;
} = {}): Record<string, unknown> {
  return {
    lastEventAt: overrides.lastEventAt ?? Date.now(),
    matchedCommand: overrides.matchedCommand ?? "claude",
    cliId: overrides.cliId,
    usageSourcePath: overrides.usageSourcePath,
    sessionId: overrides.sessionId,
    status: overrides.status,
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

  it("有项目且终端全为纯 shell（agentSession 为 null）→ 返回 empty 态", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null); // 纯 shell，无 agent 会话

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.state).toEqual({ kind: "empty" });
    expect(result.current.rows).toEqual([]);
  });

  // ──────────────────────────────────────────────────
  // now ticker（问题 1b 修复：idle 会话无 hook 事件时时间文本冻结，60s 定时重算）
  // ──────────────────────────────────────────────────

  it("now：初始存在且返回形状含 now 字段（契约）", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());

    expect(typeof result.current.now).toBe("number");
    expect(result.current.now).toBeGreaterThan(0);
    expect(Math.abs(result.current.now - Date.now())).toBeLessThan(100);
  });

  it("now ticker：推进 <60s 不变，推进到 60s 更新 +60000", () => {
    vi.useFakeTimers();
    try {
      seedProject();
      const { result } = renderHook(() => useAgentStatus());
      const initial = result.current.now;

      // 59s → 未到 tick，不变
      act(() => {
        vi.advanceTimersByTime(59_000);
      });
      expect(result.current.now).toBe(initial);

      // 再 1s（累计 60s）→ interval 触发，now 推进
      act(() => {
        vi.advanceTimersByTime(1_000);
      });
      expect(result.current.now).toBe(initial + 60_000);
    } finally {
      vi.useRealTimers();
    }
  });

  it("now ticker：unmount 后 interval 清理（advance 不再更新）", () => {
    vi.useFakeTimers();
    try {
      seedProject();
      const { result, unmount } = renderHook(() => useAgentStatus());
      const initial = result.current.now;

      unmount();
      act(() => {
        vi.advanceTimersByTime(120_000);
      });
      expect(result.current.now).toBe(initial);
    } finally {
      vi.useRealTimers();
    }
  });

  // ──────────────────────────────────────────────────
  // 初始扫描——只建 agentSession 非 null 的行
  // ──────────────────────────────────────────────────

  it("初始扫描：agentSession 非 null → 建行（携 sessionId）", () => {
    const { pageId } = seedProject();
    registerTerminal(
      "terminal-page1-0",
      makeSession({ lastEventAt: 1000, sessionId: "s1", status: "working" }),
    );

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].pageId).toBe(pageId);
    expect(result.current.rows[0].projectId).toBe("proj-1");
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].sessionId).toBe("s1");
    expect(result.current.rows[0].lastEventAt).toBe(1000);
    // 行 cliId：agentSession.cliId 缺省（makeSession 未设）→ 兜底 CLAUDE_CLI_ID
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
    expect(result.current.state).toEqual({ kind: "ready" });
  });

  it("初始扫描：matchedCommand-only（无 sessionId）→ 行 sessionId 缺省不报错", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].sessionId).toBeUndefined();
  });

  it("初始扫描：混合终端——纯 shell 不建行，活会话建行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 2000 })); // 活会话
    registerTerminal("terminal-page1-1", null);  // 纯 shell——不建行
    registerTerminal("terminal-page1-2");         // undefined agentSession——不建行

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

  // ──────────────────────────────────────────────────
  // sessionChange 建行（双通道之一）
  // ──────────────────────────────────────────────────

  it("sessionChange（非 null）→ 建行（带 matchedCommand + sessionId）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null); // 先注册为纯 shell

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0); // 纯 shell 无行

    // sessionChange 触发——设置 agentSession 非 null
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setAgentSession("terminal-page1-0", {
        matchedCommand: "claude",
        sessionId: "s1",
        status: "working",
      });
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].sessionId).toBe("s1");
  });

  it("sessionChange 建行携 cliId（OSC 133 通道：agentSession.cliId 自然驱动，MC-410）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", null); // 先注册为纯 shell

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    // OSC 133 命中后 setAgentSession 携 cliId（MC-107 写入 profile.id）
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setAgentSession("terminal-page1-0", {
        matchedCommand: "claude",
        cliId: CLAUDE_CLI_ID,
      });
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
  });

  it("sessionChange 建行幂等——行已存在时跳过不建重复行", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // 再次 sessionChange（同一 panelId）——不应建重复行
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).setAgentSession("terminal-page1-0", {
        matchedCommand: "claude",
      });
    });

    expect(result.current.rows).toHaveLength(1);
  });

  // ──────────────────────────────────────────────────
  // register 不建行（session null 时）——语义反转
  // ──────────────────────────────────────────────────

  it("register 触发通知但 agentSession 为 null → 不建行", () => {
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

  it("hook 事件（非 SessionEnd/Exit）且行不存在 → 建行（携 payload.sessionId）", () => {
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
          sessionId: "hook-s1",
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].sessionId).toBe("hook-s1");
    // 行 cliId（MC-410）：缺省分支——payload 无 cliId + registry 无 agentSession → CLAUDE_CLI_ID
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
  });

  it("hook 事件建行携 cliId（MC-205 显式分支：payload.cliId 经可选字段注入）", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "SessionStart",
          timestamp: 1000,
          cliId: CLAUDE_CLI_ID, // 显式分支（本 Stage 后端恒 undefined，经字段注入模拟）
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
  });

  it("hook 事件建行携 cliId（MC-205 反查分支：agentSession.cliId 优先于缺省）", () => {
    seedProject();
    // 注册表已有 agentSession（携 cliId）——SessionEnd 删行后行不存在，新事件建行走反查
    registerTerminal(
      "terminal-page1-0",
      makeSession({ lastEventAt: 1000, cliId: CLAUDE_CLI_ID }),
    );

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // SessionEnd 删行（hook 事件通道；registry agentSession 仍存在）
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "SessionEnd", timestamp: 2000 }),
      );
    });
    expect(result.current.rows).toHaveLength(0);

    // 新事件 → 建行 → cliId 反查 agentSession.cliId
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "SessionStart", timestamp: 3000 }),
      );
    });
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
  });

  it("hook 事件建行携 cliId（ZQ-2：空串/仅空白 cliId 与 null/undefined 同等回退缺省）", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "SessionStart",
          timestamp: 1000,
          cliId: "   ", // 仅空白——trim 后为空，必须回退而非按空串解析 profile
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    // 空串 cliId 不短路：回退缺省 CLAUDE_CLI_ID（原 ?? 链遇空串会解析出空串 profile 导致跳过）
    expect(result.current.rows[0].cliId).toBe(CLAUDE_CLI_ID);
  });

  it("null 映射事件首达建行 status=null 无图标（ZQ-3 决策 2）——SessionStart 丢失场景感知存活", () => {
    // 场景：SessionStart 事件丢失（进程间竞态），首个到达事件是 null 映射事件
    // （Notification(auth_success) → eventToStatus 返回 null）。ZQ-3 决策 2：
    // 建行但 status 置 null（无图标）——行出现证明会话存活，但不误标 attention。
    seedProject();
    // 不预注册 terminal——hook 事件独立建行

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "Notification",
          notificationType: "auth_success", // null 映射子类型
          timestamp: 1000,
        }),
      );
    });

    // 建行：会话感知存活
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    // status null = 无状态（StatusDot 不渲染圆点）——不误标 attention
    expect(result.current.rows[0].status).toBeNull();
  });

  it("未知 cliId（未注册）→ console.warn + 跳过（不建行，MC-206）", () => {
    seedProject();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const { result } = renderHook(() => useAgentStatus());

      act(() => {
        capturedCallback.current?.(
          makePayload({ event: "SessionStart", timestamp: 1000, cliId: "unknown-cli" }),
        );
      });

      // 跳过：不建行 + console.warn（不抛异常）
      expect(result.current.rows).toHaveLength(0);
      expect(warnSpy).toHaveBeenCalled();
    } finally {
      warnSpy.mockRestore();
    }
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
      (TerminalRegistry as any).setAgentSession("terminal-page1-0", null);
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
      (TerminalRegistry as any).setAgentSession("terminal-page2-0", {
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
  // ContextUsage 信号事件（statusline 桥接通道——官方 used_percentage 口径）
  // ──────────────────────────────────────────────────

  it("ContextUsage 信号 → 行存在时更新 usage（usedPercentage 原样写入）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows[0].usage).toBeUndefined();

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "ContextUsage",
          timestamp: 2000,
          usedPercentage: 23.6,
        }),
      );
    });

    expect(result.current.rows[0].usage).toEqual({ usedPercentage: 23.6 });
    // 不动状态/时间（usage 更新不视为会话活动）
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].lastEventAt).toBe(1000);
  });

  it("ContextUsage 信号 → 行不存在时不建行（先于建行到达时忽略）", () => {
    seedProject();
    // 不注册终端——无行

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "ContextUsage",
          usedPercentage: 50,
        }),
      );
    });

    // 不建行（usage 事件不是建行通道）
    expect(result.current.rows).toHaveLength(0);
    expect(result.current.state).toEqual({ kind: "empty" });
  });

  it("ContextUsage 信号字段缺失（usedPercentage undefined）→ 忽略不更新", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "ContextUsage",
          // 无 usedPercentage 字段
        }),
      );
    });

    expect(result.current.rows[0].usage).toBeUndefined();
    expect(result.current.rows).toHaveLength(1);
  });

  it("ContextUsage 信号不触发删除（非 SessionEnd/Exit 通道）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "ContextUsage",
          usedPercentage: 10,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
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

  it("working 行收到 Notification(auth_success) 后状态仍为 working（null 不覆盖）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    // PreToolUse → working
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
  // FE-06: onAgentEvent 订阅不因渲染重建
  // ──────────────────────────────────────────────────

  it("行更新触发重渲染后 onAgentEvent 调用次数不增（deps [] 稳定订阅）", () => {
    seedProject();
    registerTerminal("terminal-page1-0", makeSession({ lastEventAt: 1000 }));

    const { result } = renderHook(() => useAgentStatus());

    const callCountBefore = (onAgentEvent as ReturnType<typeof vi.fn>).mock.calls.length;

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

    // onAgentEvent 不应被重新调用（handleHookEvent deps [] 稳定）
    expect((onAgentEvent as ReturnType<typeof vi.fn>).mock.calls.length).toBe(callCountBefore);
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
