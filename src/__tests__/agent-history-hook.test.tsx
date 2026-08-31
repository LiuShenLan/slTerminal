// agent-history-hook.test.tsx — FE-04 useAgentHistory 数据 hook 测试（F12 订阅化改造）
//
// 覆盖：初始 idle / 订阅后自动执行一轮（挂载即扫语义，scanAgentHistory("claude", true)）/
// manual 失败 → error（triggerNow 后 scanAgentHistory reject，旧 data 保留）/
// error 后再次成功恢复 ready / removeLocal 不重扫 /
// TerminalRegistry.subscribe 事件驱动 activeStatuses 更新（复合键 cliId|sessionId——
// MC-313，旧数据无 cliId 按 CLAUDE_CLI_ID 回退）/ 卸载取消订阅 /
// rootPath 推导与切换不自动重扫。
//
// mock 模式：vi.hoisted() 共享状态 + 模块级 vi.mock（照 background-tasks-session-refresh
// 先例——listBackgroundTasks 恒返回 sessionRefresh 配置防 activate 真实 invoke；
// intervalSec=300 大间隔防 tick 干扰断言）+ 真实 Zustand stores（setState 种子）+
// renderHook + act。任务注册触发点 = 文件顶部 side-effect import "./tasks"（硬约束 #13）；
// beforeEach _reset 后重注册任务（runSessionRefresh 导出供测试重注册，照 Stage 02 先例）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useAgentHistory } from "../features/agentHistory/useAgentHistory";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { resetProjectStores, seedExplorerProject } from "./helpers/workspace-setup";
import { backgroundTaskScheduler } from "../features/backgroundTasks/scheduler";
import { runSessionRefresh } from "../features/backgroundTasks/sessionRefreshTask";
import "../features/backgroundTasks/tasks"; // 注册触发点（side-effect import，禁止隐式初始化）
import { SESSION_REFRESH_TASK_ID } from "../types/backgroundTasks";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { claudeProfile } from "../features/cliProfiles/profiles/claude";
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
    mockListBackgroundTasks: vi.fn(),
  };
});

/** 通知全部 registry listener（模拟 register/remove/sessionChange 事件） */
function notifyListeners(event: { type: string; panelId: string }) {
  for (const fn of h.listeners) {
    fn(event);
  }
}

// ── mock IPC + TerminalRegistry（hook 仅消费 scanAgentHistory / getAll / subscribe） ──
vi.mock("../ipc/agentHistory", () => ({
  scanAgentHistory: h.mockScanHistory,
}));
// list 恒返回 sessionRefresh 配置（enabled=true, intervalSec=300 大间隔防 tick 干扰断言）
vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: h.mockListBackgroundTasks,
  setBackgroundTaskConfig: vi.fn().mockResolvedValue([]),
  onBackgroundTasksUpdated: vi.fn(() => () => {}),
}));
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: h.mockGetAll,
    subscribe: h.mockSubscribe,
  },
}));

/** sessionRefresh 任务配置（大间隔防 tick 干扰——300s 远超用例时长） */
const SESSION_REFRESH_CONFIG = {
  taskId: "sessionRefresh",
  title: "会话历史刷新",
  enabled: true,
  intervalSec: 300,
  intervalMin: 2,
  intervalMax: 300,
};

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

/** 刷新微任务链（list resolve → 首轮执行 → ready 广播）——照 Stage 02 先例 */
async function flushMicrotasks(times = 20) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

beforeEach(() => {
  resetProjectStores();
  seedExplorerProject("C:\\project"); // 页面 cwd = "C:\project\src"
  // 调度器/注册表每用例重置 + 任务重注册（_reset 清空后恢复——runSessionRefresh 导出
  // 供测试重注册，注册触发点仍收敛 tasks.ts）+ 注册 claude profile（history 能力参与扫描）
  backgroundTaskScheduler._reset();
  backgroundTaskScheduler.register({ id: SESSION_REFRESH_TASK_ID, run: runSessionRefresh });
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
  h.listeners.clear();
  h.all.clear();
  h.mockUnsubscribe.mockClear();
  h.mockGetAll.mockClear();
  h.mockSubscribe.mockClear();
  h.mockScanHistory.mockReset();
  h.mockScanHistory.mockResolvedValue([]);
  h.mockListBackgroundTasks.mockReset();
  h.mockListBackgroundTasks.mockResolvedValue([SESSION_REFRESH_CONFIG]);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  backgroundTaskScheduler._reset();
  cliProfileRegistry._reset();
  vi.restoreAllMocks();
});

describe("useAgentHistory 初始态", () => {
  it("初始 idle + 空 sessions + 空 activeStatuses，rootPath 推导自活跃页面 cwd", () => {
    const { result } = renderHook(() => useAgentHistory());
    expect(result.current.state).toBe("idle");
    expect(result.current.sessions).toEqual([]);
    expect(result.current.activeStatuses.size).toBe(0);
    expect(result.current.rootPath).toBe("C:\\project\\src");
    // 订阅已触发激活（配置读取在途）——同步时刻尚未发扫描
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

describe("useAgentHistory 订阅驱动扫描", () => {
  it("订阅后自动执行一轮（挂载即扫语义）→ ready + sessions，scanAgentHistory('claude', true)", async () => {
    const sessions = [makeSession({ sessionId: "s1", cwd: "D:/a" })];
    h.mockScanHistory.mockResolvedValue(sessions);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    });
    expect(result.current.state).toBe("ready");
    expect(result.current.sessions).toEqual(sessions);
    // 扫描执行体恒 force=true（绕过后端 (mtime, 文件数) 缓存，规格 §8）
    expect(h.mockScanHistory).toHaveBeenCalledWith("claude", true);
  });

  it("manual 失败 → error（triggerNow 后 scanAgentHistory reject，旧 data 保留）", async () => {
    const sessions = [makeSession({ sessionId: "s1" })];
    h.mockScanHistory.mockResolvedValue(sessions); // 首轮 tick 成功
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    });
    expect(result.current.state).toBe("ready");
    // 第二轮 manual 失败 → error（规格 §7：manual 失败置 error 态，data 保留）
    h.mockScanHistory.mockRejectedValue(new Error("scan failed"));
    act(() => {
      result.current.triggerNow();
    });
    await act(async () => {
      await flushMicrotasks();
    });
    expect(result.current.state).toBe("error");
    expect(result.current.sessions).toEqual(sessions);
    expect(console.error).toHaveBeenCalled();
  });

  it("error 后再次 triggerNow 成功 → 恢复 ready", async () => {
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    }); // 首轮 tick 成功（空列表）
    h.mockScanHistory.mockRejectedValueOnce(new Error("boom"));
    act(() => {
      result.current.triggerNow();
    });
    await act(async () => {
      await flushMicrotasks();
    });
    expect(result.current.state).toBe("error");
    const sessions = [makeSession({ sessionId: "s2" })];
    h.mockScanHistory.mockResolvedValueOnce(sessions);
    act(() => {
      result.current.triggerNow();
    });
    await act(async () => {
      await flushMicrotasks();
    });
    expect(result.current.state).toBe("ready");
    expect(result.current.sessions).toEqual(sessions);
  });
});

describe("useAgentHistory 局部更新", () => {
  /** 首轮自动执行落地 sessions（订阅触发） */
  async function seedSessions() {
    const sessions = [
      makeSession({ sessionId: "s1", title: "标题一" }),
      makeSession({ sessionId: "s2", title: "标题二" }),
    ];
    h.mockScanHistory.mockResolvedValue(sessions);
    const { result } = renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    });
    return result;
  }

  it("removeLocal 删除条目且不触发重扫", async () => {
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

  it("注册表事件不触发重扫（订阅首轮之后扫描次数恒 1）", async () => {
    renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    });
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    act(() => {
      notifyListeners({ type: "register", panelId: "panel-x" });
    });
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
  });

  it("卸载取消订阅", () => {
    const { unmount } = renderHook(() => useAgentHistory());
    expect(h.mockSubscribe).toHaveBeenCalledTimes(1);
    unmount();
    expect(h.mockUnsubscribe).toHaveBeenCalledTimes(1);
  });
});

describe("useAgentHistory rootPath", () => {
  it("rootPath 变化不自动重扫（历史区数据与项目弱相关）", async () => {
    const { result, rerender } = renderHook(() => useAgentHistory());
    await act(async () => {
      await flushMicrotasks();
    });
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1); // 仅订阅首轮
    act(() => {
      useLayout.setState({ activePageId: null });
    });
    rerender();
    expect(result.current.rootPath).toBeNull();
    expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
  });
});
