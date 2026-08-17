// toast.test.tsx — toast L2 测试（OV-01，UI-804）
//
// 覆盖：右上容器渲染、单条语义色规格（success/warning/error 的 12% 底 + 语义描边
// + fg-1 文字 + 圆角 8）、堆叠顺序、自动消失时长（fake timers：
// success 3s / warning 4s / error 5s）、到期仅移除自身条目。
// 纯前端组件，零 IPC mock。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, act, type RenderResult } from "@testing-library/react";
import { toast, ToastHost, type ToastType } from "../lib";
import {
  SIDEBAR_FG,
  ERROR_FG,
  AGENT_STATUS_USAGE_COLORS,
  GIT_FILE_COLORS,
} from "../theme";

afterEach(() => {
  cleanup();
  toast._reset();
  vi.useRealTimers();
});

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"） */
function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

/** 语义色 12% rgba 的 jsdom 归一化形态（与实现 withAlpha 同源换算） */
function expectedBg(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, 0.12)`;
}

/** 语义色单点映射（与实现 TOAST_COLORS 同源） */
const TYPE_COLORS: Record<ToastType, string> = {
  success: AGENT_STATUS_USAGE_COLORS.low, // #86bb7a
  warning: GIT_FILE_COLORS.modified, // #d6b25e
  error: ERROR_FG, // #d9706b
};

/** 挂载 Host 并弹出通知，返回渲染工具与容器元素（有通知时才存在） */
function renderWithToast(type: ToastType, message: string) {
  const utils = render(<ToastHost />);
  act(() => {
    toast.show(type, message);
  });
  const container = utils.container.querySelector(
    '[data-e2e="toast-container"]',
  ) as HTMLElement | null;
  return { ...utils, container };
}

describe("toast 渲染规格", () => {
  it("show 后渲染消息，容器固定于右上角", () => {
    const { container, getByText } = renderWithToast("success", "保存成功");

    expect(getByText("保存成功")).toBeTruthy();
    expect(container!.style.position).toBe("fixed");
    expect(container!.style.top).toBe("16px");
    expect(container!.style.right).toBe("16px");
    // 无障碍：容器声明 live region，屏幕阅读器播报新通知（FE-29）
    expect(container!.getAttribute("role")).toBe("status");
    expect(container!.getAttribute("aria-live")).toBe("polite");
  });

  it("三型语义色规格：12% 底 + 1px 语义描边 + fg-1 文字 + 圆角 8", () => {
    const utils = render(<ToastHost />);
    act(() => {
      toast.show("success", "成功");
      toast.show("warning", "警告");
      toast.show("error", "错误");
    });
    const container = utils.container.querySelector(
      '[data-e2e="toast-container"]',
    ) as HTMLElement;

    const items = container.querySelectorAll(
      '[data-e2e^="toast-"]',
    ) as NodeListOf<HTMLElement>;
    expect(items).toHaveLength(3);

    items.forEach((item, i) => {
      const type = (["success", "warning", "error"] as ToastType[])[i];
      expect(item.dataset.e2e).toBe(`toast-${type}`);
      expect(item.style.background).toBe(expectedBg(TYPE_COLORS[type]));
      expect(item.style.border).toBe(`1px solid ${hexToRgb(TYPE_COLORS[type])}`);
      expect(item.style.color).toBe(hexToRgb(SIDEBAR_FG));
      expect(item.style.borderRadius).toBe("8px");
    });
  });

  it("多条堆叠，按 show 顺序渲染", () => {
    const { container, getByText } = renderWithToast("error", "第一条");
    act(() => {
      toast.show("success", "第二条");
    });

    const items = container!.querySelectorAll('[data-e2e^="toast-"]');
    expect(items).toHaveLength(2);
    // 首条在上（flex 列序）
    expect(items[0].textContent).toBe("第一条");
    expect(getByText("第二条")).toBeTruthy();
  });

  it("无通知时容器不渲染", () => {
    const utils: RenderResult = render(<ToastHost />);
    expect(
      utils.container.querySelector('[data-e2e="toast-container"]'),
    ).toBeNull();
  });
});

describe("toast 自动消失（fake timers）", () => {
  it("success 3s / warning 4s / error 5s 到期移除，提前保留", () => {
    vi.useFakeTimers();
    const utils = render(<ToastHost />);
    act(() => {
      toast.show("success", "s");
      toast.show("warning", "w");
      toast.show("error", "e");
    });
    // 实时查询：全部条目消失后容器会被卸载，持有旧引用会读到残留文本
    const texts = () =>
      utils.container.querySelector('[data-e2e="toast-container"]')
        ?.textContent ?? "";

    // 3s 前全部保留
    act(() => {
      vi.advanceTimersByTime(2999);
    });
    expect(texts()).toContain("s");
    expect(texts()).toContain("w");
    expect(texts()).toContain("e");

    // 3s 整 → success 移除，warning/error 保留
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(texts()).not.toContain("s");
    expect(texts()).toContain("w");
    expect(texts()).toContain("e");

    // 4s 整 → warning 移除，error 保留
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(texts()).not.toContain("w");
    expect(texts()).toContain("e");

    // 5s 整 → error 移除，容器清空
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(texts()).not.toContain("e");
    // 全部到期 → 容器整体卸载
    expect(
      utils.container.querySelector('[data-e2e="toast-container"]'),
    ).toBeNull();
  });
});
