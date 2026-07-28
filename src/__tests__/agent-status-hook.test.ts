// agent-status-hook.test.ts — P2-TE-03：useAgentStatus 状态联动测试
//
// 覆盖：事件到达插入/更新行、Stop 事件状态变 done 且保留、
// SessionEnd/Exit 移除行、过滤非当前项目事件、按 lastEventAt 倒序、
// contextUsage 在含 transcriptPath 时被调用、
// FE-03 TerminalRegistry 订阅增删、FE-04 标题查找、FE-05 null 状态跳过、
// FE-06 onHookEvent 重订阅次数稳定。
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
  const map = new Map<string, unknown>();
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

// ── mock TerminalRegistry（含 subscribe + register/remove 通知） ──
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn((panelId: string, entry: unknown) => {
      terminalMap.set(panelId, entry);
      for (const fn of registryListeners) {
        fn({ type: "register", panelId });
      }
    }),
    get: vi.fn((panelId: string) => terminalMap.get(panelId)),
    remove: vi.fn((panelId: string) => {
      const existed = terminalMap.delete(panelId);
      if (existed) {
        for (const fn of registryListeners) {
          fn({ type: "remove", panelId });
        }
      }
      return existed;
    }),
    has: vi.fn((panelId: string) => terminalMap.has(panelId)),
    getAll: vi.fn(() => new Map(terminalMap)),
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
  // 禁用 debounce 持久化，避免 hooks 内部副作用干扰测试
  useLayout.setState({ activePageId: pageId });
  return { projectId, pageId };
}

/** 在 mock TerminalRegistry 中注册一个终端（直接操作 Map，不触发 subscribe 通知） */
function registerTerminal(panelId: string) {
  terminalMap.set(panelId, {
    term: {} as unknown,
    sessionId: `session-${panelId}`,
    webglAddon: null,
    fitAddon: {} as unknown,
  });
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

// ═══════════════════════════════════════════════════════════════════
// 测试用例
// ═══════════════════════════════════════════════════════════════════

describe("useAgentStatus", () => {
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
    // 确保订阅已清理
    capturedCallback.current = null;
  });

  // ── 状态机派生 ──

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

  // ── T1：TerminalRegistry 初始扫描 ──

  it("T1：TerminalRegistry 含已注册终端 → 初始扫描生成对应行", () => {
    const { pageId } = seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].pageId).toBe(pageId);
    expect(result.current.rows[0].projectId).toBe("proj-1");
    // 初始态为 attention（代表命令运行中）
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.state).toEqual({ kind: "ready" });
  });

  it("初始扫描过滤非当前项目的 panelId", () => {
    // 当前项目只有 page1
    seedProject("proj-1", "page1");
    // 注册两个终端——page1 应入选，page2（其他项目）应被过滤
    registerTerminal("terminal-page1-0");
    registerTerminal("terminal-page2-0");

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
  });

  // ── T2：事件到达更新行状态 ──

  it("T2：PermissionRequest 事件 → 行状态变为 attention 且 lastEventAt 更新", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    // 初始状态由 TerminalRegistry 扫描设定
    const initialTimestamp = result.current.rows[0]?.lastEventAt;

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PermissionRequest",
          timestamp: 5000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].lastEventAt).toBe(5000);
    expect(result.current.rows[0].lastEventAt).not.toBe(initialTimestamp);
  });

  it("PreToolUse 事件 → 行状态变为 working", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          event: "PreToolUse",
          timestamp: 3000,
        }),
      );
    });

    expect(result.current.rows[0].status).toBe("working");
  });

  it("SessionStart 事件到达 → 插入新行（TerminalRegistry 中无该 terminal），状态为 attention", () => {
    seedProject();
    // 不预先注册 terminal——测试事件驱动插入

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

    // SessionStart 不在 Stop/SessionEnd/Exit 特判中 → 真实 eventToStatus: SessionStart → "attention"
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
  });

  // ── T3：Stop 置 done 且保留 ──

  it("T3：Stop 事件 → 行状态变为 done 且仍保留在列表中", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "Stop", timestamp: 4000 }),
      );
    });

    expect(result.current.rows).toHaveLength(1);
    // 真实 eventToStatus: Stop → "done"（不再依赖 Stop 特判）
    expect(result.current.rows[0].status).toBe("done");
    expect(result.current.rows[0].lastEventAt).toBe(4000);
  });

  // ── T4：SessionEnd / Exit 移除行 ──

  it("T4：SessionEnd 事件到达 → 行被移除", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "SessionEnd", timestamp: 5000 }),
      );
    });

    expect(result.current.rows).toHaveLength(0);
    expect(result.current.state).toEqual({ kind: "empty" });
  });

  it("Exit 事件到达 → 行被移除", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "Exit", timestamp: 5000 }),
      );
    });

    expect(result.current.rows).toHaveLength(0);
    expect(result.current.state).toEqual({ kind: "empty" });
  });

  it("SessionEnd 到达时 TerminalRegistry 中无对应 entry → 旧行仍被移除", () => {
    seedProject();
    // 通过事件插入行，不预注册 terminal
    const { result } = renderHook(() => useAgentStatus());

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

  // ── T5：过滤非当前项目事件 ──

  it("T5：事件来自其他项目 pageId → 不进入当前项目 rows", () => {
    seedProject("proj-1", "page1");
    registerTerminal("terminal-page1-0");

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

    // 仍只有 page1 的行，page2 未插入
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
  });

  // ── T6：多行按 lastEventAt 倒序 ──

  it("T6：多行时按 lastEventAt 倒序排列", () => {
    seedProject();
    registerTerminal("terminal-page1-0");
    registerTerminal("terminal-page1-1");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(2);

    // 初始 scan 的 lastEventAt = Date.now()，先更新两行到受控时间戳覆盖初始值
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          timestamp: 1000,
        }),
      );
    });
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-1",
          timestamp: 2000,
        }),
      );
    });
    // 此时 page1-1(2000) 应在 page1-0(1000) 之前（倒序）
    expect(result.current.rows[0].panelId).toBe("terminal-page1-1");

    // 更新 page1-0 到更大时间戳 → 应排到前面
    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          timestamp: 3000,
        }),
      );
    });

    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].lastEventAt).toBe(3000);
    expect(result.current.rows[1].panelId).toBe("terminal-page1-1");
    expect(result.current.rows[1].lastEventAt).toBe(2000);
  });

  it("新行插入也有序——较新事件在较旧事件之前", () => {
    seedProject();
    // 不预注册——通过事件驱动插入两行

    const { result } = renderHook(() => useAgentStatus());

    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-0",
          event: "SessionStart",
          timestamp: 1000,
        }),
      );
    });

    act(() => {
      capturedCallback.current?.(
        makePayload({
          panelId: "terminal-page1-1",
          event: "SessionStart",
          timestamp: 2000,
        }),
      );
    });

    expect(result.current.rows).toHaveLength(2);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-1");
    expect(result.current.rows[1].panelId).toBe("terminal-page1-0");
  });

  // ── T7：contextUsage 在含 transcriptPath 时被调用 ──

  it("T7：事件含 transcriptPath → 调用 contextUsage 拉取用量", async () => {
    seedProject();
    registerTerminal("terminal-page1-0");

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

    // 同步验证：contextUsage 已被调用
    expect(mockContextUsage).toHaveBeenCalledWith(
      "/path/to/transcript.jsonl",
    );
    expect(mockContextUsage).toHaveBeenCalledTimes(1);

    // 异步验证：usage 字段在 promise resolve 后更新
    await waitFor(() => {
      expect(result.current.rows[0].usage).toEqual({
        inputTokens: 12000,
        outputTokens: 4500,
      });
    });
  });

  it("contextUsage 返回 null → usage 字段为 null", async () => {
    seedProject();
    registerTerminal("terminal-page1-0");

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

    // 同步验证调用
    expect(mockContextUsage).toHaveBeenCalledWith(
      "/path/to/transcript.jsonl",
    );

    await waitFor(() => {
      expect(result.current.rows[0].usage).toBeNull();
    });
  });

  it("contextUsage 报错 → 行仍存在且 usage 保持为 undefined（降级不崩）", async () => {
    seedProject();
    registerTerminal("terminal-page1-0");

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

    expect(mockContextUsage).toHaveBeenCalledWith("/bad/path.jsonl");

    // 降级：行仍存在，usage 保持 undefined（初始值）
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].usage).toBeUndefined();
  });

  it("无 transcriptPath 的事件不触发 contextUsage", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

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

  // ── 行更新细节 ──

  it("重复事件更新同一行——不创建重复条目", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

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

    // 应只有一行（同一 panelId），且状态和时间戳已更新
    expect(result.current.rows).toHaveLength(1);
    // PostToolUse → "working"（真实映射）
    expect(result.current.rows[0].status).toBe("working");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  it("Stop 后新事件也能更新该行", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    // Stop → done（真实 eventToStatus）
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "Stop", timestamp: 1000 }),
      );
    });
    expect(result.current.rows[0].status).toBe("done");

    // 新事件（如 PreToolUse）→ 更新为 working，保留上一轮的 usage
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 2000 }),
      );
    });
    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].status).toBe("working");
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  it("remove 不存在的 panelId 不抛异常", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    // SessionEnd 对不存在的 panelId（无影响）
    expect(() => {
      act(() => {
        capturedCallback.current?.(
          makePayload({
            panelId: "terminal-nonexistent-0",
            event: "SessionEnd",
          }),
        );
      });
    }).not.toThrow();

    expect(result.current.rows).toHaveLength(1);
  });

  // ═══════════════════════════════════════════════════════════════════
  // FE-05: null 状态不覆盖已有行状态
  // ═══════════════════════════════════════════════════════════════════

  it("FE-05：⚡ 行收到 Notification(auth_success) 后状态仍为 ⚡（null 不覆盖）", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    // 先发送 PreToolUse → working (⚡)
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 1000 }),
      );
    });
    expect(result.current.rows[0].status).toBe("working");

    // 发送 Notification(auth_success) → 真实 eventToStatus 返回 null
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
    // 但 lastEventAt 应刷新
    expect(result.current.rows[0].lastEventAt).toBe(2000);
  });

  it("FE-05：未知事件（eventToStatus 返回 null）不覆盖状态", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    // 初始状态 attention
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

  // ═══════════════════════════════════════════════════════════════════
  // FE-03: TerminalRegistry 订阅——register 插入行、remove 移除行
  // ═══════════════════════════════════════════════════════════════════

  it("FE-03：TerminalRegistry.register 触发订阅 → 插入 🟡 行", () => {
    seedProject();

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    // 通过 TerminalRegistry.register 注册终端（触发 subscribe 通知）
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).register("terminal-page1-0", {
        term: {},
        sessionId: "s-new",
        webglAddon: null,
        fitAddon: {},
      });
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-0");
    expect(result.current.rows[0].status).toBe("attention");
    expect(result.current.rows[0].pageId).toBe("page1");
  });

  it("FE-03：TerminalRegistry.register 另一个项目终端 → 不插入（过滤）", () => {
    seedProject("proj-1", "page1");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(0);

    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).register("terminal-page2-0", {
        term: {},
        sessionId: "s-other",
        webglAddon: null,
        fitAddon: {},
      });
    });

    // page2 不在当前项目 → 不插入
    expect(result.current.rows).toHaveLength(0);
  });

  it("FE-03：TerminalRegistry.remove 触发订阅 → 移除对应行", () => {
    seedProject();
    registerTerminal("terminal-page1-0");
    registerTerminal("terminal-page1-1");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(2);

    act(() => {
      TerminalRegistry.remove("terminal-page1-0");
    });

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].panelId).toBe("terminal-page1-1");
  });

  it("FE-03：TerminalRegistry.register 已存在的 panelId → 不重复插入", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows).toHaveLength(1);

    // 再次 register 同一 panelId（幂等覆盖）
    act(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (TerminalRegistry as any).register("terminal-page1-0", {
        term: {},
        sessionId: "s-dup",
        webglAddon: null,
        fitAddon: {},
      });
    });

    // 不应创建重复行
    expect(result.current.rows).toHaveLength(1);
  });

  // ═══════════════════════════════════════════════════════════════════
  // FE-04: 行标题查 dockviewApi
  // ═══════════════════════════════════════════════════════════════════

  it("FE-04：getPageApi 返回带 title 面板 → 行标题为页签标题", () => {
    mockGetPageApi.mockImplementation(() => ({
      getPanel: (panelId: string) =>
        panelId === "terminal-page1-0"
          ? { title: "我的终端", focus: vi.fn() }
          : undefined,
    }));

    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].title).toBe("我的终端");
  });

  it("FE-04：getPageApi 返回 undefined → 回退标题 终端 {pageId}", () => {
    mockGetPageApi.mockReturnValue(undefined);

    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    expect(result.current.rows).toHaveLength(1);
    expect(result.current.rows[0].title).toBe("终端 page1");
  });

  it("FE-04：事件到达时刷新已有行标题", () => {
    // 初始时 getPageApi 不存在 → 回退标题
    mockGetPageApi.mockReturnValue(undefined);

    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());
    expect(result.current.rows[0].title).toBe("终端 page1");

    // 后面 getPageApi 就绪 → 事件到达后标题应刷新
    mockGetPageApi.mockImplementation(() => ({
      getPanel: (panelId: string) =>
        panelId === "terminal-page1-0"
          ? { title: "新标题", focus: vi.fn() }
          : undefined,
    }));

    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 3000 }),
      );
    });

    expect(result.current.rows[0].title).toBe("新标题");
  });

  // ═══════════════════════════════════════════════════════════════════
  // FE-06: onHookEvent 订阅不因渲染重建
  // ═══════════════════════════════════════════════════════════════════

  it("FE-06：行更新触发重渲染后 onHookEvent 调用次数不增", () => {
    seedProject();
    registerTerminal("terminal-page1-0");

    const { result } = renderHook(() => useAgentStatus());

    // 记下初始 onHookEvent 调用次数
    const callCountBefore = (onHookEvent as ReturnType<typeof vi.fn>).mock.calls.length;

    // 发送事件 → 行更新 → 重渲染
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PreToolUse", timestamp: 1000 }),
      );
    });

    // 发送第二个事件
    act(() => {
      capturedCallback.current?.(
        makePayload({ event: "PostToolUse", timestamp: 2000 }),
      );
    });

    // 行更新触发 React 重渲染，但 onHookEvent 不应被重新调用
    // （handleHookEvent 的 deps 由 useMemo 稳定，useEffect 不重建）
    expect((onHookEvent as ReturnType<typeof vi.fn>).mock.calls.length).toBe(callCountBefore);
    expect(result.current.rows).toHaveLength(1);
  });
});
