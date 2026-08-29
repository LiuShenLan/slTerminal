// settings-plan-balance.test.tsx — 设置中心「套餐余量」频率页 L2 测试（F11，SC-FE-04）
//
// 覆盖：缺失/越界显示 60 / 合法提交调专用命令且 refreshPlanBalance（生效反馈闭环）/
// 非法行内红字不提交不 toast / 命令 Err → toast + 保留用户输入。
//
// mock 策略：文件级 vi.mock 接管 ../ipc/planBalance（覆盖 setup.ts 全局 mock）与
// ../ipc/settings（loadSettings 数据源）；../lib 保留真实实现仅替换 toast（断言 show 调用）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  cleanup,
  fireEvent,
  screen,
  waitFor,
} from "@testing-library/react";
import PlanBalancePage from "../panels/settings/pages/PlanBalancePage";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => ({
  loadSettings: vi.fn(),
  setPlanBalanceInterval: vi.fn(),
  refreshPlanBalance: vi.fn(),
  toastShow: vi.fn(),
}));

// 文件级 mock 覆盖 setup.ts 全局 mock（同一模块路径，文件级优先）
vi.mock("../ipc/planBalance", () => ({
  getPlanBalance: vi.fn(),
  refreshPlanBalance: h.refreshPlanBalance,
  onPlanBalanceUpdated: vi.fn(),
  setPlanBalanceInterval: h.setPlanBalanceInterval,
}));

vi.mock("../ipc/settings", () => ({
  loadSettings: h.loadSettings,
  saveSettings: vi.fn(),
}));

// 保留 lib 真实实现（getErrorMessage 等），仅替换 toast（断言 show 调用）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: { show: h.toastShow, _reset: vi.fn() } };
});

/** 渲染频率页（loadSettings 挂载后异步读 → 输入框就绪） */
async function renderPage() {
  render(<PlanBalancePage />);
  const input = (await screen.findByLabelText("套餐余量查询频率（秒）")) as HTMLInputElement;
  return input;
}

/** loadSettings 返回 planBalance.intervalSec 值（data=null 模拟无文件） */
function mockSettingsWithInterval(intervalSec: number | null) {
  h.loadSettings.mockResolvedValue({
    data:
      intervalSec === null
        ? null
        : { planBalance: { intervalSec } },
    corrupted: false,
  });
}

beforeEach(() => {
  h.loadSettings.mockClear();
  h.setPlanBalanceInterval.mockClear();
  h.refreshPlanBalance.mockClear();
  h.toastShow.mockClear();
});

afterEach(() => {
  cleanup();
});

describe("挂载读取（缺失/越界显示 60，与后端 resolve_poll_interval 钳制语义对齐）", () => {
  it("无文件（data=null）→ 输入框显示 60", async () => {
    mockSettingsWithInterval(null);
    const input = await renderPage();
    expect(input.value).toBe("60");
  });

  it("intervalSec 缺失 → 显示 60", async () => {
    h.loadSettings.mockResolvedValue({ data: {}, corrupted: false });
    const input = await renderPage();
    expect(input.value).toBe("60");
  });

  it("低于下界（5）→ 显示 60", async () => {
    mockSettingsWithInterval(5);
    const input = await renderPage();
    expect(input.value).toBe("60");
  });

  it("高于上界（9999）→ 显示 60", async () => {
    mockSettingsWithInterval(9999);
    const input = await renderPage();
    expect(input.value).toBe("60");
  });

  it("合法值（120）→ 显示 120", async () => {
    mockSettingsWithInterval(120);
    const input = await renderPage();
    expect(input.value).toBe("120");
  });

  it("读设置失败 → 按默认值 60 显示，不阻塞页面", async () => {
    h.loadSettings.mockRejectedValue(new Error("read fail"));
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const input = await renderPage();
    expect(input.value).toBe("60");
    errSpy.mockRestore();
  });
});

describe("合法提交（失焦/回车 → 调专用命令 + 生效反馈闭环）", () => {
  it("失焦提交合法值 → setPlanBalanceInterval(120) + refreshPlanBalance", async () => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockResolvedValue(undefined);
    h.refreshPlanBalance.mockResolvedValue([]);
    const input = await renderPage();

    fireEvent.change(input, { target: { value: "120" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(h.setPlanBalanceInterval).toHaveBeenCalledTimes(1);
    });
    expect(h.setPlanBalanceInterval).toHaveBeenCalledWith(120);
    // 生效反馈闭环（规格 §4.4）：成功后调一次 refreshPlanBalance 拉取最新余量
    await waitFor(() => {
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);
    });
    // 不弹 toast（成功路径）
    expect(h.toastShow).not.toHaveBeenCalled();
    // 成功规范化显示
    expect(input.value).toBe("120");
  });

  it("回车提交合法值 → 同样走提交通道（含 trim）", async () => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockResolvedValue(undefined);
    h.refreshPlanBalance.mockResolvedValue([]);
    const input = await renderPage();

    fireEvent.change(input, { target: { value: "  120  " } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(h.setPlanBalanceInterval).toHaveBeenCalledWith(120);
    });
    await waitFor(() => {
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);
    });
  });

  it("提交成功但 refreshPlanBalance 失败 → 仅 console.error 防御，不 toast", async () => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockResolvedValue(undefined);
    h.refreshPlanBalance.mockRejectedValue(new Error("refresh fail"));
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const input = await renderPage();

    fireEvent.change(input, { target: { value: "120" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(h.setPlanBalanceInterval).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(errSpy).toHaveBeenCalled();
    });
    expect(h.toastShow).not.toHaveBeenCalled();
    errSpy.mockRestore();
  });
});

describe("非法输入（行内红字，不提交不 toast）", () => {
  it.each([
    ["非数字", "abc"],
    ["空串", ""],
    ["小数", "30.5"],
    ["低于下界", "5"],
    ["高于上界", "9999"],
  ])("%s → 行内红字提示且不调命令", async (_name, badValue) => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockResolvedValue(undefined);
    h.refreshPlanBalance.mockResolvedValue([]);
    const input = await renderPage();

    fireEvent.change(input, { target: { value: badValue } });
    fireEvent.blur(input);

    // 行内红字出现（文案 = 范围提示）
    await waitFor(() => {
      expect(
        document.querySelector('[data-e2e="settings-plan-balance-error"]'),
      ).not.toBeNull();
    });
    const errorEl = document.querySelector('[data-e2e="settings-plan-balance-error"]');
    expect(errorEl?.textContent).toContain("10–3600 秒，默认 60");
    // 不提交不 toast
    expect(h.setPlanBalanceInterval).not.toHaveBeenCalled();
    expect(h.refreshPlanBalance).not.toHaveBeenCalled();
    expect(h.toastShow).not.toHaveBeenCalled();
    // 用户输入保留（不重置）
    expect(input.value).toBe(badValue);
  });

  it("非法后改回合法 → 红字消失并成功提交", async () => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockResolvedValue(undefined);
    h.refreshPlanBalance.mockResolvedValue([]);
    const input = await renderPage();

    fireEvent.change(input, { target: { value: "abc" } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(
        document.querySelector('[data-e2e="settings-plan-balance-error"]'),
      ).not.toBeNull();
    });

    fireEvent.change(input, { target: { value: "120" } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(h.setPlanBalanceInterval).toHaveBeenCalledWith(120);
    });
    expect(
      document.querySelector('[data-e2e="settings-plan-balance-error"]'),
    ).toBeNull();
  });
});

describe("命令 Err（后端拒绝）→ toast + 保留用户输入", () => {
  it("setPlanBalanceInterval reject → toast.show(warning) + 输入框保留用户值", async () => {
    mockSettingsWithInterval(60);
    h.setPlanBalanceInterval.mockRejectedValue(new Error("设置轮询间隔失败: 须为 10–3600 秒"));
    h.refreshPlanBalance.mockResolvedValue([]);
    const input = await renderPage();

    fireEvent.change(input, { target: { value: "120" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(h.toastShow).toHaveBeenCalledTimes(1);
    });
    expect(h.toastShow).toHaveBeenCalledWith(
      "warning",
      expect.stringContaining("设置失败"),
    );
    // 保留用户输入（不重置回原值）
    expect(input.value).toBe("120");
    // 失败路径不触发反馈闭环
    expect(h.refreshPlanBalance).not.toHaveBeenCalled();
  });
});
