// panel-activation-focus.test.tsx — usePanelActivationFocus hook 直测（C4）
//
// 覆盖两条焦点驱动路径：
// ① 挂载期意图消费（ready && isActive && isGroupActive && 意图 take）；
// ② 激活事件联动（不需意图——页签点击/去重 focus/switchToPageAndFocus）。
// 恢复豁免锚 = 「无意图挂载即激活不 focus」（fromJSON 从不写意图）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import type { DockviewPanelApi } from "dockview-react";
import { usePanelActivationFocus } from "../panels/usePanelActivationFocus";
import {
  markPanelFocusIntent,
  _resetPanelFocusIntent,
} from "../workspace/panelFocusIntent";

/** fake 面板 api：可控 isActive/isGroupActive + 事件派发捕获 */
function fakePanelApi(init: { isActive: boolean; isGroupActive: boolean }) {
  const activeHandlers = new Set<() => void>();
  const groupHandlers = new Set<() => void>();
  const api = {
    isActive: init.isActive,
    isGroupActive: init.isGroupActive,
    onDidActiveChange: (cb: () => void) => {
      activeHandlers.add(cb);
      return { dispose: () => activeHandlers.delete(cb) };
    },
    onDidActiveGroupChange: (cb: () => void) => {
      groupHandlers.add(cb);
      return { dispose: () => groupHandlers.delete(cb) };
    },
  } as unknown as DockviewPanelApi;
  return {
    api,
    /** 触发激活变化（先改状态再派发——照 dockview 真实序） */
    fireActive(isActive: boolean) {
      (api as { isActive: boolean }).isActive = isActive;
      activeHandlers.forEach((h) => h());
    },
    fireGroupActive(isGroupActive: boolean) {
      (api as { isGroupActive: boolean }).isGroupActive = isGroupActive;
      groupHandlers.forEach((h) => h());
    },
  };
}

beforeEach(() => {
  _resetPanelFocusIntent();
});

/** 冲刷一帧 rAF（setup.ts 的 rAF mock = setTimeout(0)——路径① focus 延迟一帧
 *  等 dockview overlay 可见性翻开，断言前须过一宏任务）。
 *  分工：负向断言（不应 focus）用本函数单轮冲刷即可（rAF 未注册时无延迟注册风险）；
 *  正向断言一律用 waitFor 轮询——passive effect 的 flush 时机受 act 边界影响，
 *  rAF 可能晚于本轮定时器注册，单轮冲刷会间歇漏判（2026-09-19 实证）。 */
const flushRaf = async () => {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
};

describe("usePanelActivationFocus 挂载期意图消费", () => {
  it("挂载即激活 + 有意图 → focus 一次（新建打开路径）", async () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:terminal-0");

    renderHook(() => usePanelActivationFocus(api, "page-1:terminal-0", focus));

    await waitFor(() => expect(focus).toHaveBeenCalledTimes(1));
  });

  it("挂载即激活 + 无意图 → 不 focus（fromJSON 恢复豁免锚）", async () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();

    renderHook(() => usePanelActivationFocus(api, "page-1:terminal-0", focus));

    await flushRaf();
    expect(focus).not.toHaveBeenCalled();
  });

  it("挂载未激活 + 有意图 → 不消费不 focus；其后激活事件 → focus（事件路径接管）", async () => {
    const f = fakePanelApi({ isActive: false, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:editor-0");

    renderHook(() => usePanelActivationFocus(f.api, "page-1:editor-0", focus));
    await flushRaf();
    expect(focus).not.toHaveBeenCalled();

    // 用户点页签激活 → 事件路径（无需意图；同步 focus——激活时面板已可见）
    act(() => f.fireActive(true));
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("ready false 挂载（有意图+激活）→ 不 focus；ready 翻 true 补消费 → focus", async () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:editor-1");

    const { rerender } = renderHook(
      ({ ready }) => usePanelActivationFocus(api, "page-1:editor-1", focus, ready),
      { initialProps: { ready: false } },
    );
    await flushRaf();
    expect(focus).not.toHaveBeenCalled();

    rerender({ ready: true });
    await waitFor(() => expect(focus).toHaveBeenCalledTimes(1));
  });

  it("意图 take 语义：消费一次后 ready 二次翻转不重复 focus", async () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:terminal-1");

    const { rerender } = renderHook(
      ({ ready }) => usePanelActivationFocus(api, "page-1:terminal-1", focus, ready),
      { initialProps: { ready: false } },
    );
    rerender({ ready: true });
    await waitFor(() => expect(focus).toHaveBeenCalledTimes(1));
    rerender({ ready: false });
    rerender({ ready: true });
    // 二次翻转后再过一帧——take 语义锁死二次消费
    await flushRaf();
    expect(focus).toHaveBeenCalledTimes(1);
  });
});

describe("usePanelActivationFocus 激活事件联动", () => {
  it("激活事件双条件满足 → focus（不需意图——去重命中/页签点击路径）", () => {
    const f = fakePanelApi({ isActive: false, isGroupActive: false });
    const focus = vi.fn();
    renderHook(() => usePanelActivationFocus(f.api, "page-1:editor-2", focus));

    // 组先激活（面板未激活）→ 双条件缺一不 focus
    act(() => f.fireGroupActive(true));
    expect(focus).not.toHaveBeenCalled();
    // 面板再激活 → 双条件齐 → focus
    act(() => f.fireActive(true));
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("激活事件时 ready false → 不 focus（异步资源未就绪）", () => {
    const f = fakePanelApi({ isActive: false, isGroupActive: true });
    const focus = vi.fn();
    renderHook(() =>
      usePanelActivationFocus(f.api, "page-1:editor-3", focus, false),
    );

    act(() => f.fireActive(true));
    expect(focus).not.toHaveBeenCalled();
  });
});
