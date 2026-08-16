// confirm-dialog.test.tsx — ConfirmDialog L2 测试（OV-01，UI-801/803）
//
// 覆盖：渲染规格（遮罩/卡片/按钮色值）、按钮回调（确认 true / 取消 false）、
// ESC / 遮罩点击 = false、danger 变体、默认文案与可省略 title、kind 契约字段接受。
// 纯前端组件，零 IPC mock。

import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, fireEvent, act } from "@testing-library/react";
import {
  confirmDialog,
  ConfirmDialogHost,
  _resetConfirmDialog,
  type ConfirmDialogOptions,
} from "../lib";
import {
  SHADOW_MENU,
  SIDEBAR_BG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  FOCUS_BORDER,
  ON_ACCENT_FG,
  ERROR_FG,
  SIDEBAR_FG,
  SECONDARY_BG,
} from "../theme";

afterEach(() => {
  cleanup();
  _resetConfirmDialog();
});

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 输入补空格） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  const m = hex.match(/^rgba\((\d+),(\d+),(\d+),([0-9.]+)\)$/);
  if (m) return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${m[4]})`;
  return hex;
}

/** 挂载 Host + 弹出一个弹窗，返回渲染工具、Promise 与关键元素 */
async function openDialog(opts: ConfirmDialogOptions) {
  const utils = render(<ConfirmDialogHost />);
  // confirmDialog 触发的状态更新需包在 act 内
  let promise!: Promise<boolean>;
  await act(async () => {
    promise = confirmDialog(opts);
  });
  return {
    ...utils,
    promise,
    mask: utils.container.querySelector(
      '[data-e2e="confirm-dialog-mask"]',
    ) as HTMLElement,
    card: utils.container.querySelector(
      '[data-e2e="confirm-dialog"]',
    ) as HTMLElement,
    ok: utils.container.querySelector(
      '[data-e2e="confirm-ok"]',
    ) as HTMLButtonElement,
    cancel: utils.container.querySelector(
      '[data-e2e="confirm-cancel"]',
    ) as HTMLButtonElement,
  };
}

describe("ConfirmDialog 渲染规格", () => {
  it("标题 + 消息 + 确认/取消按钮均渲染", async () => {
    const { getByText, card } = await openDialog({
      title: "确认删除",
      message: "删除后将无法恢复。",
      confirmText: "删除",
      cancelText: "保留",
    });

    expect(card).toBeTruthy();
    expect(getByText("确认删除")).toBeTruthy();
    expect(getByText("删除后将无法恢复。")).toBeTruthy();
    expect(getByText("删除")).toBeTruthy();
    expect(getByText("保留")).toBeTruthy();
  });

  it("遮罩/卡片/按钮色值符合 UI-801 规格", async () => {
    const { mask, card, ok, cancel } = await openDialog({ message: "规格" });

    // 遮罩 rgba(0,0,0,0.55)（SHADOW_MENU）
    expect(mask.style.background).toBe(hexToRgb(SHADOW_MENU));
    expect(mask.style.position).toBe("fixed");

    // 卡片 #1a1a1e 底 + 0.09 白描边 + 圆角 8 + contextMenuShadow 阴影
    expect(card.style.background).toBe(hexToRgb(SIDEBAR_BG));
    expect(card.style.border).toBe(`1px solid ${hexToRgb(CONTEXT_MENU_BORDER)}`);
    expect(card.style.borderRadius).toBe("8px");
    expect(card.style.boxShadow).toBe(SIDEBAR_COLORS.contextMenuShadow);

    // 主按钮 #6e9ff2 底 + #0c1220 字；次按钮 #222227 底 + #ece9e4 字
    expect(ok.style.background).toBe(hexToRgb(FOCUS_BORDER));
    expect(ok.style.color).toBe(hexToRgb(ON_ACCENT_FG));
    expect(cancel.style.background).toBe(hexToRgb(SECONDARY_BG));
    expect(cancel.style.color).toBe(hexToRgb(SIDEBAR_FG));
  });

  it("danger → 主按钮 ERROR_FG 底 + SIDEBAR_FG 字", async () => {
    const { ok } = await openDialog({ message: "危险操作", danger: true });
    expect(ok.style.background).toBe(hexToRgb(ERROR_FG));
    expect(ok.style.color).toBe(hexToRgb(SIDEBAR_FG));
  });

  it("title/confirmText/cancelText 省略 → 默认文案 + 无标题行", async () => {
    const { queryByText, getByText } = await openDialog({ message: "只有消息" });
    expect(getByText("确认")).toBeTruthy();
    expect(getByText("取消")).toBeTruthy();
    expect(queryByText("确认删除")).toBeNull();
  });

  it("kind 契约字段三值均可接受，不抛错", async () => {
    for (const kind of ["warning", "error", "info"] as const) {
      const { getByText } = await openDialog({ message: `kind=${kind}`, kind });
      expect(getByText(`kind=${kind}`)).toBeTruthy();
      cleanup();
    }
  });

  it("无挂起弹窗时 Host 渲染 null", () => {
    const { container } = render(<ConfirmDialogHost />);
    expect(container.querySelector('[data-e2e="confirm-dialog"]')).toBeNull();
  });
});

describe("ConfirmDialog 交互", () => {
  it("点击确认 → Promise 解析 true，弹窗关闭", async () => {
    const { ok, promise, container } = await openDialog({ message: "确认？" });

    await act(async () => {
      fireEvent.click(ok);
    });
    expect(await promise).toBe(true);
    expect(container.querySelector('[data-e2e="confirm-dialog"]')).toBeNull();
  });

  it("点击取消 → Promise 解析 false，弹窗关闭", async () => {
    const { cancel, promise, container } = await openDialog({ message: "取消？" });

    await act(async () => {
      fireEvent.click(cancel);
    });
    expect(await promise).toBe(false);
    expect(container.querySelector('[data-e2e="confirm-dialog"]')).toBeNull();
  });

  it("ESC 键 → Promise 解析 false", async () => {
    const { promise, container } = await openDialog({ message: "ESC 取消" });

    await act(async () => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(await promise).toBe(false);
    expect(container.querySelector('[data-e2e="confirm-dialog"]')).toBeNull();
  });

  it("非 Escape 键不关闭", async () => {
    const { promise, card } = await openDialog({ message: "其他键" });

    await act(async () => {
      fireEvent.keyDown(window, { key: "Enter" });
    });
    expect(card).toBeTruthy();
    // 等待微任务后仍未解析（promise 仍挂起）
    await act(async () => {});
    await expect(
      Promise.race([
        promise.then(() => "resolved"),
        new Promise<string>((r) => setTimeout(() => r("pending"), 20)),
      ]),
    ).resolves.toBe("pending");
  });

  it("遮罩点击 → Promise 解析 false；点卡片内部不关闭", async () => {
    const { mask, promise } = await openDialog({ message: "遮罩" });

    await act(async () => {
      fireEvent.mouseDown(mask);
    });
    expect(await promise).toBe(false);

    // 再次打开，点击卡片内部不触发关闭
    const second = await openDialog({ message: "卡片内部" });
    await act(async () => {
      fireEvent.mouseDown(second.card);
    });
    expect(second.card).toBeTruthy();
    await act(async () => {
      fireEvent.click(second.cancel); // 按钮走 onClick
    });
    expect(await second.promise).toBe(false);
  });
});
