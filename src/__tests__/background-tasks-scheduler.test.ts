// background-tasks-scheduler.test.ts —— BackgroundTaskScheduler L2 测试（F12）
//
// 覆盖：注册表契约（register/getAll 注册序/同 id 覆盖/_reset/未注册订阅）/ subscribe
// 立即回调 + 首个订阅者激活（读配置 → 立即一轮 / enabled=false 不执行 / list 失败仍
// 首轮不启动 interval）/ 订阅者计数启停（interval 触发/全部退订停止/重订阅立即一轮
// 不重复读配置）/ tick 防重入（挂起时 tick 与 manual 均被闸门跳过）/ 失败处理
// （tick 失败不置 error + data 保留 + console.error；manual 失败 state=error + data
// 保留）/ applyConfig（改频率重启 timer/禁用停/启用立即一轮/无订阅者不启动）/
// applyLocal（updater 变换 + 广播，state 不变）。
//
// mock 模式：vi.hoisted() + 模块级 vi.mock("../ipc/backgroundTasks")（照
// agent-history-hook.test.tsx 先例）；每用例 backgroundTaskScheduler._reset() 隔离
// （注册表家族契约 #13）；fake timers 断言 interval 行为（Promise 微任务不受
// fake timers 影响，flush 用多层 await Promise.resolve()）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { backgroundTaskScheduler } from "../features/backgroundTasks/scheduler";
import type { BackgroundTaskDef } from "../features/backgroundTasks/types";

// ── vi.hoisted()：listBackgroundTasks mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => ({
  mockList: vi.fn(),
}));

vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: h.mockList,
}));

/** 刷新微任务链（list resolve → 首轮执行 → ready 广播，约 2-3 层依赖） */
async function flushMicrotasks(times = 20) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

/** 任务配置夹具（六键契约，intervalSec 可调） */
function makeTaskInfo(
  taskId: string,
  enabled: boolean,
  intervalSec: number,
): Record<string, unknown> {
  return {
    taskId,
    title: taskId,
    enabled,
    intervalSec,
    intervalMin: 1,
    intervalMax: 100,
  };
}

/** 注册自动成功任务（run 每次 resolve ["ok"]），list 返回对应配置 */
function registerAutoTask(
  id: string,
  enabled = true,
  intervalSec = 30,
): { run: ReturnType<typeof vi.fn>; def: BackgroundTaskDef<string[]> } {
  h.mockList.mockResolvedValue([makeTaskInfo(id, enabled, intervalSec)]);
  const run = vi.fn().mockResolvedValue(["ok"]);
  const def: BackgroundTaskDef<string[]> = { id, run };
  backgroundTaskScheduler.register(def);
  return { run, def };
}

/** 注册挂起任务（run 挂起直到外部 resolve/reject——防重入用例用）。
    control 对象引用传递（解构时 resolve 尚未赋值，须经对象属性取用） */
function registerDeferredTask(
  id: string,
  enabled = true,
  intervalSec = 30,
): {
  run: ReturnType<typeof vi.fn>;
  control: { resolve?: (v: string[]) => void; reject?: (e: Error) => void };
} {
  h.mockList.mockResolvedValue([makeTaskInfo(id, enabled, intervalSec)]);
  const control: { resolve?: (v: string[]) => void; reject?: (e: Error) => void } = {};
  const run = vi.fn(
    () =>
      new Promise<string[]>((res, rej) => {
        control.resolve = res;
        control.reject = rej;
      }),
  );
  backgroundTaskScheduler.register({ id, run });
  return { run, control };
}

beforeEach(() => {
  vi.useFakeTimers();
  backgroundTaskScheduler._reset();
  h.mockList.mockReset();
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  backgroundTaskScheduler._reset();
});

describe("注册表契约（家族 #13）", () => {
  it("register 后 getAll 按注册序返回全部任务", () => {
    backgroundTaskScheduler.register({ id: "a", run: vi.fn() });
    backgroundTaskScheduler.register({ id: "b", run: vi.fn() });
    backgroundTaskScheduler.register({ id: "c", run: vi.fn() });
    expect(backgroundTaskScheduler.getAll().map((d) => d.id)).toEqual(["a", "b", "c"]);
  });

  it("同 id 覆盖旧条目（运行时状态随条目重建清零）", () => {
    const run1 = vi.fn();
    backgroundTaskScheduler.register({ id: "a", run: run1 });
    const run2 = vi.fn();
    backgroundTaskScheduler.register({ id: "a", run: run2 });
    expect(backgroundTaskScheduler.getAll()).toHaveLength(1);
    expect(backgroundTaskScheduler.getAll()[0].run).toBe(run2);
    // 覆盖后订阅拿到全新 idle 快照（旧 listeners/状态清零）
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("a", listener);
    expect(listener).toHaveBeenCalledWith({ state: "idle", data: undefined });
  });

  it("_reset 清空全部任务", () => {
    backgroundTaskScheduler.register({ id: "a", run: vi.fn() });
    backgroundTaskScheduler._reset();
    expect(backgroundTaskScheduler.getAll()).toEqual([]);
  });

  it("subscribe 未注册 id → console.error + no-op 退订", () => {
    const unsub = backgroundTaskScheduler.subscribe("missing", vi.fn());
    expect(console.error).toHaveBeenCalledWith("[slTerminal] 后台任务未注册: missing");
    expect(() => unsub()).not.toThrow();
  });
});

describe("subscribe 激活", () => {
  it("subscribe 立即回调当前快照（idle，同步）", () => {
    backgroundTaskScheduler.register({ id: "t1", run: vi.fn() });
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({ state: "idle", data: undefined });
  });

  it("首个订阅者 → 读配置 + enabled=true 立即执行一轮", async () => {
    const { run } = registerAutoTask("t1", true, 30);
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks();
    expect(h.mockList).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledTimes(1);
    // 首轮执行完成 → ready
    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ state: "ready", data: ["ok"] }),
    );
  });

  it("enabled=false 配置 → 不执行首轮不启动定时", async () => {
    registerAutoTask("t1", false, 30);
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks();
    expect(h.mockList).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledTimes(1); // 仅同步 idle 回调
    await vi.advanceTimersByTimeAsync(30000);
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("list 失败 → 仍执行首轮 + console.error + 不启动 interval", async () => {
    const { run } = registerAutoTask("t1", true, 30);
    h.mockList.mockRejectedValueOnce(new Error("network"));
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks();
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining("后台任务配置读取失败"),
      expect.anything(),
    );
    // enabled 保持注册默认 true → 首轮执行
    expect(run).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ state: "ready", data: ["ok"] }),
    );
    // intervalSec 未读到（0）→ 不启动 interval
    await vi.advanceTimersByTimeAsync(60000);
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("订阅者计数启停（fake timers）", () => {
  it("interval 按 intervalSec 触发；全部退订 → 不再触发；重订阅 → 立即一轮且不重复读配置", async () => {
    const { run } = registerAutoTask("t1", true, 1);
    const l1 = vi.fn();
    const unsub1 = backgroundTaskScheduler.subscribe("t1", l1);
    await flushMicrotasks();
    expect(h.mockList).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledTimes(1); // 首轮立即
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(3);
    // 全部退订 → 停 interval
    unsub1();
    await vi.advanceTimersByTimeAsync(10000);
    expect(run).toHaveBeenCalledTimes(3);
    // 重订阅：同步回调当前快照 + 立即一轮 + 不重复读配置（configReady 已 true）
    const l2 = vi.fn();
    backgroundTaskScheduler.subscribe("t1", l2);
    expect(l2).toHaveBeenCalledWith(expect.objectContaining({ state: "ready" }));
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(4);
    expect(h.mockList).toHaveBeenCalledTimes(1);
    // 定时重启
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(5);
  });
});

describe("防重入", () => {
  it("run 挂起时 tick 与 manual 均被闸门跳过（run 调用次数不增）", async () => {
    const { run, control } = registerDeferredTask("t1", true, 1);
    backgroundTaskScheduler.subscribe("t1", vi.fn());
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(1); // 首轮挂起（loading）
    // 定时 tick 被闸门跳过
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(1);
    // 手动 triggerNow 也被同一闸门跳过
    await backgroundTaskScheduler.triggerNow("t1");
    expect(run).toHaveBeenCalledTimes(1);
    // 完成后下一轮 tick 正常触发
    control.resolve!(["ok"]);
    await flushMicrotasks();
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(2);
  });
});

describe("失败处理（规格 §7）", () => {
  it("tick 失败静默：不置 error + data 保留 + console.error", async () => {
    const { run } = registerAutoTask("t1", true, 1);
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks(); // 首轮成功 → ready
    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ state: "ready", data: ["ok"] }),
    );
    run.mockRejectedValueOnce(new Error("boom"));
    await vi.advanceTimersByTimeAsync(1000); // 第二轮 tick 失败
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining("后台任务执行失败（t1, tick）"),
      expect.anything(),
    );
    // 静默 = 不置 error、不额外广播；data 保留
    const calls = listener.mock.calls.map(([s]) => s.state as string);
    expect(calls).not.toContain("error");
    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ data: ["ok"] }),
    );
  });

  it("manual 失败 → state=error + data 保留 + console.error", async () => {
    const { run } = registerAutoTask("t1", true, 30);
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks(); // 首轮成功 → ready
    run.mockRejectedValueOnce(new Error("boom"));
    await backgroundTaskScheduler.triggerNow("t1");
    expect(listener).toHaveBeenLastCalledWith({
      state: "error",
      data: ["ok"],
    });
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining("后台任务执行失败（t1, manual）"),
      expect.anything(),
    );
  });
});

describe("applyConfig 运行期改配", () => {
  it("改频率 → timer 重启（新间隔生效）", async () => {
    const { run } = registerAutoTask("t1", true, 2);
    backgroundTaskScheduler.subscribe("t1", vi.fn());
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(2000);
    expect(run).toHaveBeenCalledTimes(2);
    // 改频率 2s → 1s
    backgroundTaskScheduler.applyConfig("t1", { enabled: true, intervalSec: 1 });
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(3); // 新间隔触发
  });

  it("禁用 → 停 timer；启用 → 立即一轮 + 启动 timer", async () => {
    const { run } = registerAutoTask("t1", true, 1);
    backgroundTaskScheduler.subscribe("t1", vi.fn());
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(1);
    // 禁用 → 停 timer
    backgroundTaskScheduler.applyConfig("t1", { enabled: false, intervalSec: 1 });
    await vi.advanceTimersByTimeAsync(10000);
    expect(run).toHaveBeenCalledTimes(1);
    // 启用 → 立即一轮（timer 此前未跑）+ 启动 timer
    backgroundTaskScheduler.applyConfig("t1", { enabled: true, intervalSec: 1 });
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(3);
  });

  it("无订阅者时 applyConfig 不启动（配置已记，订阅时生效）", async () => {
    const { run } = registerAutoTask("t1", true, 1);
    backgroundTaskScheduler.applyConfig("t1", { enabled: true, intervalSec: 1 });
    await vi.advanceTimersByTimeAsync(10000);
    expect(run).toHaveBeenCalledTimes(0); // 无订阅者不空转
    // 订阅时按已记配置激活
    backgroundTaskScheduler.subscribe("t1", vi.fn());
    await flushMicrotasks();
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("applyLocal 本地变更透传", () => {
  it("updater 变换 data + 广播（state 不变）", async () => {
    registerAutoTask("t1", true, 30);
    const listener = vi.fn();
    backgroundTaskScheduler.subscribe("t1", listener);
    await flushMicrotasks();
    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ state: "ready", data: ["ok"] }),
    );
    backgroundTaskScheduler.applyLocal<string[]>("t1", (prev) => [
      ...(prev ?? []),
      "local",
    ]);
    expect(listener).toHaveBeenLastCalledWith({
      state: "ready",
      data: ["ok", "local"],
    });
  });
});
