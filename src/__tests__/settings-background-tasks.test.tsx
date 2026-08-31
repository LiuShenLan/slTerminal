// settings-background-tasks.test.tsx — 设置中心「后台定时任务」页 L2 测试（F12，FE-07）
//
// 覆盖：挂载渲染两行（标题/勾选态/频率回显/范围提示）/ list 失败空态不崩 /
// 勾选 planBalance → set 命令 + 返回清单更新行 + refreshPlanBalance 反馈闭环 /
// 勾选 sessionRefresh → 调度器 applyConfig（参数为返回清单新值）/ 频率非法行内红字
// 不提交不 toast 输入保留 / 频率合法提交 + 规范化回显 / set reject → toast + 输入保留。
//
// mock 策略：文件级 vi.mock 接管 ../ipc/backgroundTasks（list/set hoisted 可控）与
// ../ipc/planBalance（refreshPlanBalance）；../lib 保留真实实现仅替换 toast；
// ../features/backgroundTasks barrel 替换为 applyConfig spy——避免真实调度器
// （页面另经 ./tasks side-effect import 会加载真实注册，但页面只消费本 mock 的
// applyConfig，真实调度器不被激活，无需处理）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  cleanup,
  fireEvent,
  screen,
  waitFor,
} from "@testing-library/react";
import BackgroundTasksPage from "../panels/settings/pages/BackgroundTasksPage";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => ({
  listBackgroundTasks: vi.fn(),
  setBackgroundTaskConfig: vi.fn(),
  refreshPlanBalance: vi.fn(),
  toastShow: vi.fn(),
  applyConfig: vi.fn(),
}));

// 文件级 mock 覆盖 setup.ts 全局 mock（同一模块路径，文件级优先）
vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: h.listBackgroundTasks,
  setBackgroundTaskConfig: h.setBackgroundTaskConfig,
  onBackgroundTasksUpdated: vi.fn(),
}));

vi.mock("../ipc/planBalance", () => ({
  getPlanBalance: vi.fn(),
  refreshPlanBalance: h.refreshPlanBalance,
  onPlanBalanceUpdated: vi.fn(),
}));

// 保留 lib 真实实现（getErrorMessage 等），仅替换 toast（断言 show 调用）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: { show: h.toastShow, _reset: vi.fn() } };
});

// barrel 替换为 applyConfig spy（硬约束 #13 调度器注册面不在本测试范围）
vi.mock("../features/backgroundTasks", () => ({
  backgroundTaskScheduler: { applyConfig: h.applyConfig },
}));

/** 两任务默认清单（与后端 registry TASKS 元数据一致） */
function defaultList() {
  return [
    { taskId: "planBalance", title: "套餐余量查询", enabled: true, intervalSec: 10, intervalMin: 10, intervalMax: 3600 },
    { taskId: "sessionRefresh", title: "会话历史刷新", enabled: true, intervalSec: 3, intervalMin: 2, intervalMax: 300 },
  ];
}

/** 渲染页面（listBackgroundTasks 挂载后异步读 → 两行就绪） */
async function renderPage() {
  render(<BackgroundTasksPage />);
  await screen.findByLabelText("套餐余量查询频率（秒）");
}

beforeEach(() => {
  h.listBackgroundTasks.mockClear();
  h.setBackgroundTaskConfig.mockClear();
  h.refreshPlanBalance.mockClear();
  h.toastShow.mockClear();
  h.applyConfig.mockClear();
});

afterEach(() => {
  cleanup();
});

describe("挂载渲染（两任务行齐备）", () => {
  it("渲染两行：任务标题 + 勾选态 + 频率回显 + 范围提示", async () => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    await renderPage();

    // 两行标题与频率输入回显 intervalSec
    const pbInput = screen.getByLabelText("套餐余量查询频率（秒）") as HTMLInputElement;
    const srInput = screen.getByLabelText("会话历史刷新频率（秒）") as HTMLInputElement;
    expect(pbInput.value).toBe("10");
    expect(srInput.value).toBe("3");
    // 勾选态默认 true（enabled 默认值）
    const pbCheck = document.querySelector(
      '[data-e2e="settings-background-tasks-enabled-planBalance"]',
    ) as HTMLInputElement;
    const srCheck = document.querySelector(
      '[data-e2e="settings-background-tasks-enabled-sessionRefresh"]',
    ) as HTMLInputElement;
    expect(pbCheck.checked).toBe(true);
    expect(srCheck.checked).toBe(true);
    // 范围提示文案（DTO 无 default 字段，只写范围不写默认值）
    expect(document.body.textContent).toContain("10–3600 秒");
    expect(document.body.textContent).toContain("2–300 秒");
  });

  it("list 失败 → 空态不崩 + console.error", async () => {
    h.listBackgroundTasks.mockRejectedValue(new Error("list fail"));
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<BackgroundTasksPage />);

    await waitFor(() => {
      expect(errSpy).toHaveBeenCalled();
    });
    // 空态：页面容器渲染、无任务行
    expect(
      document.querySelector('[data-e2e="settings-background-tasks-page"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-e2e="settings-background-tasks-row-planBalance"]'),
    ).toBeNull();
    errSpy.mockRestore();
  });
});

describe("勾选切换（立即提交 + 生效闭环）", () => {
  it("勾选 planBalance → set('planBalance', { enabled: false }) + 返回清单更新行 + refreshPlanBalance", async () => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    const newList = defaultList().map((t) =>
      t.taskId === "planBalance" ? { ...t, enabled: false } : t,
    );
    h.setBackgroundTaskConfig.mockResolvedValue(newList);
    h.refreshPlanBalance.mockResolvedValue([]);
    await renderPage();

    const pbCheck = document.querySelector(
      '[data-e2e="settings-background-tasks-enabled-planBalance"]',
    ) as HTMLInputElement;
    fireEvent.click(pbCheck);

    await waitFor(() => {
      expect(h.setBackgroundTaskConfig).toHaveBeenCalledWith("planBalance", {
        enabled: false,
      });
    });
    // 成功后行勾选态更新（用返回清单）
    await waitFor(() => {
      const updated = document.querySelector(
        '[data-e2e="settings-background-tasks-enabled-planBalance"]',
      ) as HTMLInputElement;
      expect(updated.checked).toBe(false);
    });
    // 反馈闭环：返回清单含 planBalance → refreshPlanBalance 被调
    await waitFor(() => {
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);
    });
    // 成功路径不弹 toast
    expect(h.toastShow).not.toHaveBeenCalled();
  });

  it("勾选 sessionRefresh → 调度器 applyConfig（参数为返回清单新值）", async () => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    const newList = defaultList().map((t) =>
      t.taskId === "sessionRefresh" ? { ...t, enabled: false } : t,
    );
    h.setBackgroundTaskConfig.mockResolvedValue(newList);
    h.refreshPlanBalance.mockResolvedValue([]);
    await renderPage();

    const srCheck = document.querySelector(
      '[data-e2e="settings-background-tasks-enabled-sessionRefresh"]',
    ) as HTMLInputElement;
    fireEvent.click(srCheck);

    await waitFor(() => {
      expect(h.setBackgroundTaskConfig).toHaveBeenCalledWith("sessionRefresh", {
        enabled: false,
      });
    });
    // applyConfig 参数 = 返回清单新值（enabled 新值 + intervalSec 回显）
    await waitFor(() => {
      expect(h.applyConfig).toHaveBeenCalledWith("sessionRefresh", {
        enabled: false,
        intervalSec: 3,
      });
    });
    // 公共收尾：返回清单含 planBalance → refreshPlanBalance 同样被调
    await waitFor(() => {
      expect(h.refreshPlanBalance).toHaveBeenCalledTimes(1);
    });
  });
});

describe("非法输入（行内红字，不提交不 toast）", () => {
  it.each([
    ["非数字", "abc"],
    ["空串", ""],
    ["小数", "30.5"],
    ["低于下界", "1"],
    ["高于上界", "9999"],
  ])("%s → 行内红字 = 范围文案 + 不提交不 toast + 输入保留", async (_name, badValue) => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    await renderPage();

    const srInput = screen.getByLabelText("会话历史刷新频率（秒）") as HTMLInputElement;
    fireEvent.change(srInput, { target: { value: badValue } });
    fireEvent.blur(srInput);

    // 行内红字出现（文案 = 范围提示，无默认值）
    await waitFor(() => {
      expect(
        document.querySelector('[data-e2e="settings-background-tasks-error-sessionRefresh"]'),
      ).not.toBeNull();
    });
    const errorEl = document.querySelector(
      '[data-e2e="settings-background-tasks-error-sessionRefresh"]',
    );
    expect(errorEl?.textContent).toBe("2–300 秒");
    // 不提交不 toast
    expect(h.setBackgroundTaskConfig).not.toHaveBeenCalled();
    expect(h.toastShow).not.toHaveBeenCalled();
    // 输入保留（不重置）
    expect(srInput.value).toBe(badValue);
  });
});

describe("合法提交（失焦 → 命令 + 规范化回显）", () => {
  it("失焦提交合法值 → set('sessionRefresh', { intervalSec: 15 }) + 规范化回显", async () => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    const newList = defaultList().map((t) =>
      t.taskId === "sessionRefresh" ? { ...t, intervalSec: 15 } : t,
    );
    h.setBackgroundTaskConfig.mockResolvedValue(newList);
    h.refreshPlanBalance.mockResolvedValue([]);
    await renderPage();

    const srInput = screen.getByLabelText("会话历史刷新频率（秒）") as HTMLInputElement;
    fireEvent.change(srInput, { target: { value: " 15 " } });
    fireEvent.blur(srInput);

    await waitFor(() => {
      expect(h.setBackgroundTaskConfig).toHaveBeenCalledWith("sessionRefresh", {
        intervalSec: 15,
      });
    });
    // 规范化回显（trim 去残留空白）
    await waitFor(() => {
      expect(srInput.value).toBe("15");
    });
    // 无红字 + 成功不弹 toast
    expect(
      document.querySelector('[data-e2e="settings-background-tasks-error-sessionRefresh"]'),
    ).toBeNull();
    expect(h.toastShow).not.toHaveBeenCalled();
  });
});

describe("命令 Err（后端拒绝）→ toast + 保留用户输入", () => {
  it("set reject → toast.show(warning) + 输入保留 + 不触发反馈闭环", async () => {
    h.listBackgroundTasks.mockResolvedValue(defaultList());
    h.setBackgroundTaskConfig.mockRejectedValue(new Error("设置后台任务失败: 未知任务"));
    h.refreshPlanBalance.mockResolvedValue([]);
    await renderPage();

    const srInput = screen.getByLabelText("会话历史刷新频率（秒）") as HTMLInputElement;
    fireEvent.change(srInput, { target: { value: "15" } });
    fireEvent.blur(srInput);

    await waitFor(() => {
      expect(h.toastShow).toHaveBeenCalledTimes(1);
    });
    expect(h.toastShow).toHaveBeenCalledWith(
      "warning",
      expect.stringContaining("设置失败"),
    );
    // 保留用户输入（不重置回原值）
    expect(srInput.value).toBe("15");
    // 失败路径不触发反馈闭环
    expect(h.refreshPlanBalance).not.toHaveBeenCalled();
    expect(h.applyConfig).not.toHaveBeenCalled();
  });
});
