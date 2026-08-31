// plan-balance-footer.test.tsx — F10 PlanBalanceFooter L2 测试（PB-FE-06）
//
// 文件级 vi.mock 自定义 ../ipc/planBalance 实现，接管 setup.ts 全局 mock：
// hoisted 工厂暴露 getPlanBalance/refreshPlanBalance/onPlanBalanceUpdated 可控 mock
// + triggerUpdate 测试辅助（直接推送订阅回调，模拟后端 plan-balance-updated 事件）。
// 覆盖：四场景渲染 / 隐藏态 / 初始拉取 / 事件订阅 / 点击节流（vi.spyOn Date.now 推进）/
// logo onError 与 src / tooltip 组合。
// F12：文件级 vi.mock 追加 ../ipc/backgroundTasks——listBackgroundTasks 默认 resolve 含
// planBalance enabled=true 清单（既有用例 enabled 恒放行），onBackgroundTasksUpdated 捕获
// 回调供 triggerConfigUpdate 辅助（模拟后端 set_config 后推送清单，照 triggerUpdate 先例）；
// 覆盖：enabled=false 整块不渲染 / 事件推送隐藏与重显最后快照 / list 失败回退启用。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { PlanBalanceFooter } from "../features/navTree/PlanBalanceFooter";
import type { PlanBalanceInfo } from "../types/planBalance";
import type { BackgroundTaskInfo } from "../types/backgroundTasks";

/** planBalance 任务默认清单（enabled=true——既有用例默认放行渲染） */
const planBalanceTaskList: BackgroundTaskInfo[] = [
  { taskId: "planBalance", title: "套餐余量查询", enabled: true, intervalSec: 10, intervalMin: 10, intervalMax: 3600 },
  { taskId: "sessionRefresh", title: "会话历史刷新", enabled: true, intervalSec: 3, intervalMin: 2, intervalMax: 300 },
];

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => {
  let push: ((v: PlanBalanceInfo[]) => void) | null = null;
  let configPush: ((list: BackgroundTaskInfo[]) => void) | null = null;
  return {
    getPlanBalance: vi.fn(),
    refreshPlanBalance: vi.fn(),
    onPlanBalanceUpdated: vi.fn((cb: (v: PlanBalanceInfo[]) => void) => {
      push = cb;
      return () => { push = null; };
    }),
    /** 测试辅助：模拟后端推送新快照（照 setup.ts createNotifyMocks.triggerFsEvent 先例） */
    triggerUpdate(v: PlanBalanceInfo[]) { push?.(v); },
    listBackgroundTasks: vi.fn(),
    onBackgroundTasksUpdated: vi.fn((cb: (list: BackgroundTaskInfo[]) => void) => {
      configPush = cb;
      return () => { configPush = null; };
    }),
    /** 测试辅助：模拟后端 set_config 成功后推送完整清单（F12 enabled 感知通道） */
    triggerConfigUpdate(list: BackgroundTaskInfo[]) { configPush?.(list); },
  };
});

// 文件级 mock 覆盖 setup.ts 全局 mock（同一模块路径，文件级优先）
vi.mock("../ipc/planBalance", () => ({
  getPlanBalance: h.getPlanBalance,
  refreshPlanBalance: h.refreshPlanBalance,
  onPlanBalanceUpdated: h.onPlanBalanceUpdated,
}));

// F12：文件级 mock 覆盖 setup.ts 全局 mock（usePlanBalance 挂载即读 list + 订阅事件）
vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: h.listBackgroundTasks,
  onBackgroundTasksUpdated: h.onBackgroundTasksUpdated,
}));

/** 最小 PlanBalanceInfo 工厂（F10 DTO 六键全字段） */
function makeInfo(overrides: Partial<PlanBalanceInfo> = {}): PlanBalanceInfo {
  return {
    sourceId: "claude",
    planId: "deepseek",
    frozen: false,
    amount: null,
    windows: null,
    updatedAt: 0,
    ...overrides,
  };
}

/** deepseek 金额行夹具（§8.2 示例） */
function deepseekInfo(): PlanBalanceInfo {
  return makeInfo({
    planId: "deepseek",
    amount: { value: "12.34", currency: "CNY" },
    updatedAt: Math.floor(Date.now() / 1000),
  });
}

/** kimi 双窗行夹具（§8.2 示例） */
function kimiInfo(): PlanBalanceInfo {
  return makeInfo({
    planId: "kimi",
    windows: {
      fiveHour: { remainingPercent: 62, resetsAt: null },
      sevenDay: { remainingPercent: 45, resetsAt: null },
    },
    updatedAt: Math.floor(Date.now() / 1000),
  });
}

/** 渲染 footer 并等待首行出现（getPlanBalance 兑现后行才存在；text = 行文案） */
async function renderWithRow(info: PlanBalanceInfo, text: string) {
  h.getPlanBalance.mockResolvedValue([info]);
  render(<PlanBalanceFooter />);
  const el = await screen.findByText(text);
  return el.closest('[data-e2e="plan-balance-row"]') as HTMLElement;
}

beforeEach(() => {
  h.getPlanBalance.mockClear().mockResolvedValue([]);
  h.refreshPlanBalance.mockClear().mockResolvedValue([]);
  h.onPlanBalanceUpdated.mockClear();
  h.listBackgroundTasks.mockClear().mockResolvedValue(planBalanceTaskList);
  h.onBackgroundTasksUpdated.mockClear();
});

afterEach(() => {
  cleanup();
});

describe("四场景渲染（§8.2）", () => {
  it("deepseek 金额：¥12.34", async () => {
    h.getPlanBalance.mockResolvedValue([deepseekInfo()]);
    render(<PlanBalanceFooter />);
    expect(await screen.findByText("¥12.34")).toBeTruthy();
  });

  it("kimi 双窗：5h 62% · 7d 45%", async () => {
    h.getPlanBalance.mockResolvedValue([kimiInfo()]);
    render(<PlanBalanceFooter />);
    expect(await screen.findByText("5h 62% · 7d 45%")).toBeTruthy();
  });

  it("frozen 触顶：已冻结", async () => {
    h.getPlanBalance.mockResolvedValue([makeInfo({ frozen: true, planId: "kimi" })]);
    render(<PlanBalanceFooter />);
    expect(await screen.findByText("已冻结")).toBeTruthy();
  });

  it("占位（无成功值）：--", async () => {
    h.getPlanBalance.mockResolvedValue([makeInfo()]);
    render(<PlanBalanceFooter />);
    expect(await screen.findByText("--")).toBeTruthy();
  });
});

describe("隐藏态与数据流", () => {
  it("items=[]（无命中来源）→ 整块不渲染（含发丝线容器）", () => {
    h.getPlanBalance.mockResolvedValue([]);
    render(<PlanBalanceFooter />);
    expect(
      document.querySelector('[data-e2e="plan-balance-footer"]'),
    ).toBeNull();
  });

  it("初始拉取：挂载即调 getPlanBalance 并渲染返回值", async () => {
    const info = deepseekInfo();
    h.getPlanBalance.mockResolvedValue([info]);
    render(<PlanBalanceFooter />);
    expect(h.getPlanBalance).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("¥12.34")).toBeTruthy();
    // 挂载同时订阅事件（返回取消函数）
    expect(h.onPlanBalanceUpdated).toHaveBeenCalledTimes(1);
  });

  it("事件订阅：推送新数组 → 行文案更新", async () => {
    // 初始拉取挂起（恒 pending）——防 resolve([]) 微任务在推送后覆盖新值
    h.getPlanBalance.mockReturnValue(new Promise<PlanBalanceInfo[]>(() => {}));
    render(<PlanBalanceFooter />); // 初始空 → footer 隐藏
    h.triggerUpdate([deepseekInfo()]);
    expect(await screen.findByText("¥12.34")).toBeTruthy();
  });
});

describe("F12 enabled 感知（backgroundTasks 配置）", () => {
  it("enabled=false → 整块不渲染（有快照也隐藏）", async () => {
    // list 返回 planBalance enabled=false + getPlanBalance 有行数据——行数据就绪仍隐藏
    h.listBackgroundTasks.mockResolvedValue([
      { ...planBalanceTaskList[0], enabled: false },
      planBalanceTaskList[1],
    ]);
    h.getPlanBalance.mockResolvedValue([deepseekInfo()]);
    render(<PlanBalanceFooter />);
    await waitFor(() => {
      expect(document.querySelector('[data-e2e="plan-balance-footer"]')).toBeNull();
    });
    // 行文案也不得出现（有快照也隐藏）
    expect(screen.queryByText("¥12.34")).toBeNull();
  });

  it("事件推送 enabled=false → 已渲染 footer 隐藏；再推 enabled=true → 重显最后快照", async () => {
    h.getPlanBalance.mockResolvedValue([deepseekInfo()]);
    render(<PlanBalanceFooter />);
    // 初始 list enabled=true → 渲染出行
    expect(await screen.findByText("¥12.34")).toBeTruthy();
    // 事件推 enabled=false → 隐藏
    h.triggerConfigUpdate([{ ...planBalanceTaskList[0], enabled: false }]);
    await waitFor(() => {
      expect(document.querySelector('[data-e2e="plan-balance-footer"]')).toBeNull();
    });
    // 再推 enabled=true → 重显最后快照（不重拉）
    h.triggerConfigUpdate([planBalanceTaskList[0]]);
    expect(await screen.findByText("¥12.34")).toBeTruthy();
    expect(h.getPlanBalance).toHaveBeenCalledTimes(1); // 重显未重新拉取
  });

  it("list 失败 → 按启用处理（footer 正常渲染）", async () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      h.listBackgroundTasks.mockRejectedValue(new Error("mock list 失败"));
      h.getPlanBalance.mockResolvedValue([deepseekInfo()]);
      render(<PlanBalanceFooter />);
      expect(await screen.findByText("¥12.34")).toBeTruthy();
      expect(errSpy).toHaveBeenCalled();
    } finally {
      errSpy.mockRestore();
    }
  });
});

describe("行点击刷新（节流 5s，D7）", () => {
  it("连点两次 → refreshPlanBalance 仅调 1 次", async () => {
    const row = await renderWithRow(deepseekInfo(), "¥12.34");
    fireEvent.click(row);
    fireEvent.click(row);
    await waitFor(() => {
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);
    });
  });

  it("推进 5s+ 后再点 → 第 2 次调用；返回值更新行", async () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_000_000);
    try {
      h.refreshPlanBalance.mockResolvedValue([deepseekInfo()]);
      const row = await renderWithRow(deepseekInfo(), "¥12.34");
      fireEvent.click(row); // 第 1 次（lastRefreshRef 初始 0，必然放行）
      fireEvent.click(row); // 节流窗口内 → 忽略
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);

      // 推进 6s（> REFRESH_THROTTLE_MS=5000）→ 放行第 2 次，返回新快照更新行
      nowSpy.mockReturnValue(1_000_000 + 6_000);
      h.refreshPlanBalance.mockResolvedValue([kimiInfo()]);
      fireEvent.click(row);
      await waitFor(() => {
        expect(h.refreshPlanBalance).toHaveBeenCalledTimes(2);
      });
      expect(await screen.findByText("5h 62% · 7d 45%")).toBeTruthy();
    } finally {
      nowSpy.mockRestore();
    }
  });
});

describe("logo 与 tooltip", () => {
  it("logo src = /plan-icons/<planId>.png；onError → img 隐藏文本保留", async () => {
    h.getPlanBalance.mockResolvedValue([kimiInfo()]);
    render(<PlanBalanceFooter />);
    const row = (await screen.findByText("5h 62% · 7d 45%")).closest(
      '[data-e2e="plan-balance-row"]',
    ) as HTMLElement;
    const img = row.querySelector("img") as HTMLImageElement;
    expect(img.src).toContain("/plan-icons/kimi.png");
    expect(img.width).toBe(14);

    fireEvent.error(img);
    expect(row.querySelector("img")).toBeNull();
    expect(screen.getByText("5h 62% · 7d 45%")).toBeTruthy();
  });

  it("kimi 行 title tooltip：5h 重置 · 7d 重置 · 上次更新", async () => {
    const now = Date.now();
    const info = makeInfo({
      planId: "kimi",
      windows: {
        fiveHour: {
          remainingPercent: 62,
          resetsAt: new Date(now + 42 * 60_000).toISOString(),
        },
        // 20h30m（远离整小时边界——Date.now 微秒差会把 21h0m 抖成 20h 60m）
        sevenDay: {
          remainingPercent: 45,
          resetsAt: new Date(now + 20 * 3_600_000 + 30 * 60_000).toISOString(),
        },
      },
      updatedAt: new Date(2026, 8, 2, 14, 3, 5).getTime() / 1000,
    });
    h.getPlanBalance.mockResolvedValue([info]);
    render(<PlanBalanceFooter />);
    const row = (await screen.findByText("5h 62% · 7d 45%")).closest(
      '[data-e2e="plan-balance-row"]',
    ) as HTMLElement;
    expect(row.title).toContain("5h 窗（42m 后重置）");
    expect(row.title).toContain("7d 窗（20h 30m 后重置）");
    expect(row.title).toContain("上次更新 14:03:05");
  });
});
