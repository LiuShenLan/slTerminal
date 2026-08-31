// background-tasks-session-refresh.test.ts —— sessionRefresh 扫描执行体 L2 测试（F12）
//
// 覆盖：遍历聚合（history 能力 profile 各 scan 一次 force=true → 扁平列表）/ 无 history
// 能力 profile 被跳过 / 部分失败隔离（失败 provider 保留旧数据按 cliId 过滤）/ 全部
// 失败（tick → 快照不变不置 error；manual → state=error）/ force 恒 true。
//
// mock 模式：vi.hoisted() + 模块级 vi.mock（scanAgentHistory 可控 + listBackgroundTasks
// 恒返回 sessionRefresh 配置防 activate 真实 invoke）；每用例 backgroundTaskScheduler
// ._reset() + cliProfileRegistry._reset() 后重注册任务（runSessionRefresh 导出供重注册）；
// 任务注册触发点 = 文件顶部 side-effect import "./tasks"（硬约束 #13）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { backgroundTaskScheduler } from "../features/backgroundTasks/scheduler";
import { runSessionRefresh } from "../features/backgroundTasks/sessionRefreshTask";
import "../features/backgroundTasks/tasks"; // 注册触发点（side-effect import，禁止隐式初始化）
import { SESSION_REFRESH_TASK_ID } from "../types/backgroundTasks";
import type { CodingCliProfile } from "../features/cliProfiles/types";
import type { AgentHistorySession } from "../types/agentHistory";
import type { TaskSnapshot } from "../features/backgroundTasks/types";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => ({
  mockScan: vi.fn(),
  mockList: vi.fn(),
}));

vi.mock("../ipc/agentHistory", () => ({
  scanAgentHistory: h.mockScan,
}));
// list 恒返回 sessionRefresh 默认配置（enabled=true, intervalSec=3）——防 activate 真实 invoke
vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: h.mockList,
  setBackgroundTaskConfig: vi.fn().mockResolvedValue([]),
  onBackgroundTasksUpdated: vi.fn(() => () => {}),
}));

/** 最小 AgentHistorySession 工厂（照 agent-history-hook.test.tsx 先例） */
function makeSession(
  sessionId: string,
  cliId: string,
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId,
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId,
    ...overrides,
  };
}

/** 桩 profile：history 能力有无两种（capabilities.history 声明 = 参与扫描） */
function makeProfile(id: string, withHistory: boolean): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: withHistory
      ? {
          history: {
            supportsFork: false,
            buildResumeCommand: () => "",
            buildRestoreInput: () => "",
          },
        }
      : {},
  };
}

/** sessionRefresh 任务配置（后端 registry 默认：enabled=true, intervalSec=3） */
const SESSION_REFRESH_CONFIG = {
  taskId: "sessionRefresh",
  title: "会话历史刷新",
  enabled: true,
  intervalSec: 3,
  intervalMin: 2,
  intervalMax: 300,
};

/** 订阅 sessionRefresh 快照，收集全部广播 */
function subscribeToSessionRefresh(): {
  snapshots: TaskSnapshot<AgentHistorySession[]>[];
  unsubscribe: () => void;
} {
  const snapshots: TaskSnapshot<AgentHistorySession[]>[] = [];
  const unsubscribe = backgroundTaskScheduler.subscribe<AgentHistorySession[]>(
    SESSION_REFRESH_TASK_ID,
    (s) => snapshots.push(s),
  );
  return { snapshots, unsubscribe };
}

/** 刷新微任务链（list resolve → 首轮执行 → ready 广播） */
async function flushMicrotasks(times = 20) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

beforeEach(() => {
  vi.useFakeTimers();
  backgroundTaskScheduler._reset();
  cliProfileRegistry._reset();
  // 重注册任务（_reset 清空后恢复——runSessionRefresh 导出供测试重注册，注册触发点仍收敛 tasks.ts）
  backgroundTaskScheduler.register({ id: SESSION_REFRESH_TASK_ID, run: runSessionRefresh });
  h.mockScan.mockReset();
  h.mockList.mockReset();
  h.mockList.mockResolvedValue([SESSION_REFRESH_CONFIG]);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  backgroundTaskScheduler._reset();
  cliProfileRegistry._reset();
});

describe("遍历聚合", () => {
  it("两 history profile → scanAgentHistory 各调一次（force=true）→ 聚合扁平列表", async () => {
    cliProfileRegistry.register(makeProfile("claude", true));
    cliProfileRegistry.register(makeProfile("gemini", true));
    const a1 = makeSession("s-a1", "claude");
    const b1 = makeSession("s-b1", "gemini");
    h.mockScan.mockImplementation((cliId: string) =>
      Promise.resolve(cliId === "claude" ? [a1] : [b1]),
    );
    const { snapshots, unsubscribe } = subscribeToSessionRefresh();
    await flushMicrotasks();
    expect(h.mockScan).toHaveBeenCalledTimes(2);
    expect(h.mockScan).toHaveBeenNthCalledWith(1, "claude", true);
    expect(h.mockScan).toHaveBeenNthCalledWith(2, "gemini", true);
    // force 恒 true：断言全部调用第二参
    for (const call of h.mockScan.mock.calls) expect(call[1]).toBe(true);
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "ready", data: [a1, b1] });
    unsubscribe();
  });

  it("无 history 能力 profile 被跳过（不调 scanAgentHistory）", async () => {
    cliProfileRegistry.register(makeProfile("claude", true));
    cliProfileRegistry.register(makeProfile("gemini", false)); // 无 history 能力
    const a1 = makeSession("s-a1", "claude");
    h.mockScan.mockResolvedValue([a1]);
    const { snapshots, unsubscribe } = subscribeToSessionRefresh();
    await flushMicrotasks();
    expect(h.mockScan).toHaveBeenCalledTimes(1);
    expect(h.mockScan).toHaveBeenCalledWith("claude", true);
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "ready", data: [a1] });
    unsubscribe();
  });
});

describe("失败隔离（规格 §8）", () => {
  it("部分失败：失败 provider 保留旧数据（按 cliId 过滤 prev），成功 provider 采用新值", async () => {
    cliProfileRegistry.register(makeProfile("claude", true));
    cliProfileRegistry.register(makeProfile("gemini", true));
    const a1 = makeSession("s-a1", "claude");
    const b1 = makeSession("s-b1", "gemini");
    // 首轮全成功 → 建立 prev
    h.mockScan.mockImplementation((cliId: string) =>
      Promise.resolve(cliId === "claude" ? [a1] : [b1]),
    );
    const { snapshots, unsubscribe } = subscribeToSessionRefresh();
    await flushMicrotasks();
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "ready", data: [a1, b1] });
    // 第二轮：claude 新值，gemini 失败 → 结果 = claude 新值 + gemini 旧值
    const a2 = makeSession("s-a2", "claude");
    h.mockScan.mockImplementation((cliId: string) =>
      cliId === "claude"
        ? Promise.resolve([a2])
        : Promise.reject(new Error("gemini scan failed")),
    );
    await backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID);
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "ready", data: [a2, b1] });
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining("历史扫描失败（gemini）"),
      expect.anything(),
    );
    expect(h.mockScan).toHaveBeenNthCalledWith(3, "claude", true);
    expect(h.mockScan).toHaveBeenNthCalledWith(4, "gemini", true);
    unsubscribe();
  });

  it("全部失败：tick → 快照不变（不置 error，data 保留）；manual → state=error", async () => {
    cliProfileRegistry.register(makeProfile("claude", true));
    cliProfileRegistry.register(makeProfile("gemini", true));
    const a1 = makeSession("s-a1", "claude");
    const b1 = makeSession("s-b1", "gemini");
    // 首轮全成功 → 建立 prev
    h.mockScan.mockImplementation((cliId: string) =>
      Promise.resolve(cliId === "claude" ? [a1] : [b1]),
    );
    const { snapshots, unsubscribe } = subscribeToSessionRefresh();
    await flushMicrotasks();
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "ready", data: [a1, b1] });
    // 全部失败：tick → 静默（不置 error、data 保留——规格 §7）
    h.mockScan.mockRejectedValue(new Error("all down"));
    await vi.advanceTimersByTimeAsync(3000);
    const states = snapshots.map((s) => s.state);
    expect(states).not.toContain("error");
    expect(snapshots[snapshots.length - 1]!.data).toEqual([a1, b1]);
    // 全部失败：manual → state=error + data 保留
    await backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID);
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "error", data: [a1, b1] });
    expect(console.error).toHaveBeenCalled();
    unsubscribe();
  });

  it("全部失败（首轮即失败）：tick 首轮不置 error；manual 首轮 state=error", async () => {
    cliProfileRegistry.register(makeProfile("claude", true));
    cliProfileRegistry.register(makeProfile("gemini", true));
    h.mockScan.mockRejectedValue(new Error("all down"));
    const { snapshots, unsubscribe } = subscribeToSessionRefresh();
    await flushMicrotasks(); // 首轮 tick 失败（prev 空）
    expect(snapshots[snapshots.length - 1]!.state).not.toBe("error");
    expect(snapshots[snapshots.length - 1]!.data).toBeUndefined();
    // manual 首轮失败 → error（data 保留 undefined）
    await backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID);
    expect(snapshots[snapshots.length - 1]).toEqual({ state: "error", data: undefined });
    expect(console.error).toHaveBeenCalled();
    unsubscribe();
  });
});
