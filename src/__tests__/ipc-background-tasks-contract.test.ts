// ipc-background-tasks-contract.test.ts — 后台定时任务 IPC wrapper 合约测试（F12）
//
// 经共享工厂 describeIpcContract（helpers/ipc-contract.ts）声明式驱动
// 两命令（background_tasks_list / background_tasks_set_config）× 四维（命令名 /
// 无参或键集合 / 正常返回透传 / 异常传播）；onBackgroundTasksUpdated 为 listen
// 事件封装，属"wrapper 行为契约"（IHE-01②）——手写模拟驱动断言解包与 unsubscribe，
// 不走 invoke 工厂。文件尾部 DTO 键集合与 taskId 值集断言和后端注册表测试互为
// 双边锁（token 红线守卫：多键即红）。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";
import { BACKGROUND_TASK_IDS } from "../types/backgroundTasks";
import type { BackgroundTaskInfo } from "../types/backgroundTasks";

// 覆盖 setup.ts 全局 mock——导入原始 ../ipc/backgroundTasks 模块以测试真实 IPC 合约
vi.mock("../ipc/backgroundTasks", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/backgroundTasks")>();
});

// mock @tauri-apps/api/event — onBackgroundTasksUpdated 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
import * as backgroundTasks from "../ipc/backgroundTasks";

afterEach(() => {
  clearMocks();
});

// 两任务全形态样例（注册表默认配置）
const mockInfo: BackgroundTaskInfo[] = [
  {
    taskId: "planBalance",
    title: "套餐余量查询",
    enabled: true,
    intervalSec: 10,
    intervalMin: 10,
    intervalMax: 3600,
  },
  {
    taskId: "sessionRefresh",
    title: "会话历史刷新",
    enabled: true,
    intervalSec: 3,
    intervalMin: 2,
    intervalMax: 300,
  },
];

// ═══════════════════════════════════════════════════════════════════
// background_tasks_list（挂载读清单；无参）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("listBackgroundTasks 合约（background_tasks_list，无参）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 background_tasks_list 命令（非驼峰）",
    cmd: "background_tasks_list",
    call: () => backgroundTasks.listBackgroundTasks(),
    respond: [],
  },
  // 维度 2：无参——payload 为空对象
  {
    name: "payload 为空对象（无参命令）",
    cmd: "background_tasks_list",
    call: () => backgroundTasks.listBackgroundTasks(),
    respond: [],
    expectArgs: {},
  },
  // 维度 3：正常返回透传——BackgroundTaskInfo[]（两任务清单）
  {
    name: "透传 BackgroundTaskInfo[]（两任务清单）",
    cmd: "background_tasks_list",
    call: () => backgroundTasks.listBackgroundTasks(),
    respond: mockInfo,
    expectResult: mockInfo,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "background_tasks_list",
    call: () => backgroundTasks.listBackgroundTasks(),
    mockThrow: "清单读取失败",
    expectReject: "清单读取失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// background_tasks_set_config（配置写通道：taskId + enabled?/intervalSec? 缺省键不发送）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("setBackgroundTaskConfig 合约（background_tasks_set_config，带参）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 background_tasks_set_config 命令（非驼峰）",
    cmd: "background_tasks_set_config",
    call: () => backgroundTasks.setBackgroundTaskConfig("planBalance", { intervalSec: 120 }),
    respond: [],
  },
  // 维度 2：payload 键集合精确——{taskId, intervalSec} 形态（防单边字段漂移）
  {
    name: "payload 键集合精确 { taskId, intervalSec }",
    cmd: "background_tasks_set_config",
    call: () => backgroundTasks.setBackgroundTaskConfig("planBalance", { intervalSec: 120 }),
    respond: [],
    expectArgs: { taskId: "planBalance", intervalSec: 120 },
    expectExactKeys: ["taskId", "intervalSec"],
  },
  // 维度 2：payload 键集合精确——{taskId, enabled} 形态（undefined 键不入 payload）
  {
    name: "payload 键集合精确 { taskId, enabled }（undefined 键不入 payload）",
    cmd: "background_tasks_set_config",
    call: () => backgroundTasks.setBackgroundTaskConfig("sessionRefresh", { enabled: false }),
    respond: [],
    expectArgs: { taskId: "sessionRefresh", enabled: false },
    expectExactKeys: ["taskId", "enabled"],
  },
  // 维度 3：正常返回透传——完整清单（设置成功后后端推送/返回）
  {
    name: "透传完整清单 BackgroundTaskInfo[]",
    cmd: "background_tasks_set_config",
    call: () => backgroundTasks.setBackgroundTaskConfig("planBalance", { intervalSec: 120 }),
    respond: mockInfo,
    expectResult: mockInfo,
  },
  // 维度 4：异常传播（越界 → 后端 Validation Err → reject）
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "background_tasks_set_config",
    call: () => backgroundTasks.setBackgroundTaskConfig("planBalance", { intervalSec: 5 }),
    mockThrow: "设置后台任务失败: 套餐余量查询 频率须为 10–3600 秒，实际 5",
    expectReject: "设置后台任务失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// onBackgroundTasksUpdated（事件订阅封装）——wrapper 行为契约（IHE-01②）
//
// listen 封装的回调解包（event.payload）不在 mockIPC 层验证——Tauri 的
// listen 运行时解包由 L4 E2E 守卫。此处用模拟驱动断言 wrapper 自身的
// 解包逻辑：构造 { payload } 事件对象 → 断言 callback 收到解包后数组。
// ═══════════════════════════════════════════════════════════════════

describe("onBackgroundTasksUpdated 合约", () => {
  it("应调用 listen 监听 background-tasks-updated，回调收到解包数组", () => {
    let capturedHandler:
      | ((event: { payload: BackgroundTaskInfo[] }) => void)
      | null = null;
    vi.mocked(listen).mockImplementation(
      (_event: string, handler: unknown) => {
        capturedHandler = handler as typeof capturedHandler;
        return Promise.resolve(vi.fn());
      },
    );

    const callback = vi.fn();
    backgroundTasks.onBackgroundTasksUpdated(callback);

    expect(listen).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledWith(
      "background-tasks-updated",
      expect.any(Function),
    );

    // 模拟 Tauri 事件推送
    expect(capturedHandler).not.toBeNull();
    capturedHandler!({ payload: mockInfo });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith(mockInfo);
  });

  it("返回的 unsubscribe 应调用 listen 返回的清理函数", async () => {
    const mockUnlisten = vi.fn();
    vi.mocked(listen).mockReturnValue(Promise.resolve(mockUnlisten));

    const unsubscribe = backgroundTasks.onBackgroundTasksUpdated(vi.fn());

    // 等待 listen Promise resolve
    await new Promise((resolve) => setTimeout(resolve, 0));

    unsubscribe();
    // .then() 微任务需 flush 后才执行 mockUnlisten
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════════
// BackgroundTaskInfo 键集合——与后端 serde camelCase 双边锁（token 红线守卫）
// ═══════════════════════════════════════════════════════════════════

describe("BackgroundTaskInfo 键集合（与后端 serde 测试互为双边锁）", () => {
  it("六键精确匹配（无 default 字段）", () => {
    const info: BackgroundTaskInfo = {
      taskId: "planBalance",
      title: "套餐余量查询",
      enabled: true,
      intervalSec: 10,
      intervalMin: 10,
      intervalMax: 3600,
    };
    expect(Object.keys(info).sort()).toEqual([
      "enabled",
      "intervalMax",
      "intervalMin",
      "intervalSec",
      "taskId",
      "title",
    ]);
  });
});

// ═══════════════════════════════════════════════════════════════════
// BACKGROUND_TASK_IDS 值集——与后端 registry TASKS 键集双边锁（硬约束 #4）
// ═══════════════════════════════════════════════════════════════════

describe("BACKGROUND_TASK_IDS 值集（与后端 TASKS 键集互为双边锁）", () => {
  it("值集精确 == [planBalance, sessionRefresh]", () => {
    expect([...BACKGROUND_TASK_IDS]).toEqual(["planBalance", "sessionRefresh"]);
  });
});
