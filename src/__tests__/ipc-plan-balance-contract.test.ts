// ipc-plan-balance-contract.test.ts — 套餐余量 IPC wrapper 合约测试（F10）
//
// 经共享工厂 describeIpcContract（helpers/ipc-contract.ts）声明式驱动
// 两命令（get_plan_balance / refresh_plan_balance）× 四维（命令名 / 无参 /
// 正常返回透传 / 异常传播）；onPlanBalanceUpdated 为 listen 事件封装，
// 属"wrapper 行为契约"（IHE-01②）——手写模拟驱动断言解包与 unsubscribe，
// 不走 invoke 工厂。文件尾部 DTO 键集合断言与后端 serde 测试互为双边锁
// （token 红线守卫：多键即红）。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";
import type { PlanBalanceInfo } from "../types/planBalance";

// 覆盖 setup.ts 全局 mock——导入原始 ../ipc/planBalance 模块以测试真实 IPC 合约
vi.mock("../ipc/planBalance", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/planBalance")>();
});

// mock @tauri-apps/api/event — onPlanBalanceUpdated 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
import * as planBalance from "../ipc/planBalance";

afterEach(() => {
  clearMocks();
});

// 两套餐全形态样例（deepseek 金额 / kimi 触顶）
const mockInfo: PlanBalanceInfo[] = [
  {
    sourceId: "claude",
    planId: "deepseek",
    frozen: false,
    amount: { value: "12.34", currency: "CNY" },
    windows: null,
    updatedAt: 1_752_500_000,
  },
  {
    sourceId: "claude",
    planId: "kimi",
    frozen: true,
    amount: null,
    windows: null,
    updatedAt: 1_752_500_100,
  },
];

// ═══════════════════════════════════════════════════════════════════
// get_plan_balance（挂载拉快照；后端尚未有快照 → 空数组）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("getPlanBalance 合约（get_plan_balance，无参）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 get_plan_balance 命令（非驼峰）",
    cmd: "get_plan_balance",
    call: () => planBalance.getPlanBalance(),
    respond: [],
  },
  // 维度 2：无参——payload 为空对象
  {
    name: "payload 为空对象（无参命令）",
    cmd: "get_plan_balance",
    call: () => planBalance.getPlanBalance(),
    respond: [],
    expectArgs: {},
  },
  // 维度 3：正常返回透传——PlanBalanceInfo[]（两套餐全形态；无快照 → 空数组）
  {
    name: "透传 PlanBalanceInfo[]（两套餐全形态）",
    cmd: "get_plan_balance",
    call: () => planBalance.getPlanBalance(),
    respond: mockInfo,
    expectResult: mockInfo,
  },
  {
    name: "后端尚无快照时透传空数组",
    cmd: "get_plan_balance",
    call: () => planBalance.getPlanBalance(),
    respond: [],
    expectResult: [],
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "get_plan_balance",
    call: () => planBalance.getPlanBalance(),
    mockThrow: "快照读取失败",
    expectReject: "快照读取失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// refresh_plan_balance（点击余量行立即刷新；恒 Ok 返回最新快照，D6）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("refreshPlanBalance 合约（refresh_plan_balance，无参）", [
  // 维度 1：命令名（snake_case 逐字）
  {
    name: "应调用 refresh_plan_balance 命令（非驼峰）",
    cmd: "refresh_plan_balance",
    call: () => planBalance.refreshPlanBalance(),
    respond: [],
  },
  // 维度 2：无参——payload 为空对象
  {
    name: "payload 为空对象（无参命令）",
    cmd: "refresh_plan_balance",
    call: () => planBalance.refreshPlanBalance(),
    respond: [],
    expectArgs: {},
  },
  // 维度 3：正常返回透传——最新快照（单来源失败保留旧值，不整体 Err）
  {
    name: "透传最新快照 PlanBalanceInfo[]",
    cmd: "refresh_plan_balance",
    call: () => planBalance.refreshPlanBalance(),
    respond: mockInfo,
    expectResult: mockInfo,
  },
  // 维度 4：异常传播（调用方 catch 仅防御）
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "refresh_plan_balance",
    call: () => planBalance.refreshPlanBalance(),
    mockThrow: "任务异常",
    expectReject: "任务异常",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// onPlanBalanceUpdated（事件订阅封装）——wrapper 行为契约（IHE-01②）
//
// listen 封装的回调解包（event.payload）不在 mockIPC 层验证——Tauri 的
// listen 运行时解包由 L4 E2E 守卫。此处用模拟驱动断言 wrapper 自身的
// 解包逻辑：构造 { payload } 事件对象 → 断言 callback 收到解包后数组。
// ═══════════════════════════════════════════════════════════════════

describe("onPlanBalanceUpdated 合约", () => {
  it("应调用 listen 监听 plan-balance-updated，回调收到解包数组", () => {
    let capturedHandler:
      | ((event: { payload: PlanBalanceInfo[] }) => void)
      | null = null;
    vi.mocked(listen).mockImplementation(
      (_event: string, handler: unknown) => {
        capturedHandler = handler as typeof capturedHandler;
        return Promise.resolve(vi.fn());
      },
    );

    const callback = vi.fn();
    planBalance.onPlanBalanceUpdated(callback);

    expect(listen).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledWith(
      "plan-balance-updated",
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

    const unsubscribe = planBalance.onPlanBalanceUpdated(vi.fn());

    // 等待 listen Promise resolve
    await new Promise((resolve) => setTimeout(resolve, 0));

    unsubscribe();
    // .then() 微任务需 flush 后才执行 mockUnlisten
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════════
// PlanBalanceInfo 键集合——与后端 serde camelCase 双边锁（token 红线守卫）
// ═══════════════════════════════════════════════════════════════════

describe("PlanBalanceInfo 键集合（与后端 serde 测试互为双边锁）", () => {
  it("六键精确匹配（无 token 字段）", () => {
    const info: PlanBalanceInfo = {
      sourceId: "claude",
      planId: "kimi",
      frozen: false,
      amount: { value: "12.34", currency: "CNY" },
      windows: {
        fiveHour: { usedPercent: 62, resetsAt: "2026-09-02T06:00:00Z" },
        sevenDay: { usedPercent: 45, resetsAt: null },
      },
      updatedAt: 1_752_500_000,
    };
    expect(Object.keys(info).sort()).toEqual([
      "amount",
      "frozen",
      "planId",
      "sourceId",
      "updatedAt",
      "windows",
    ]);
  });
});
