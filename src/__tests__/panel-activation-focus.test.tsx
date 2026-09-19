// panel-activation-focus.test.tsx — usePanelActivationFocus hook 直测（C4）
//
// 覆盖两条焦点驱动路径：
// ① 挂载期意图消费（ready && isActive && isGroupActive && 意图 take）；
// ② 激活事件联动（不需意图——页签点击/去重 focus/switchToPageAndFocus）。
// 恢复豁免锚 = 「无意图挂载即激活不 focus」（fromJSON 从不写意图）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
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

describe("usePanelActivationFocus 挂载期意图消费", () => {
  it("挂载即激活 + 有意图 → focus 一次（新建打开路径）", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:terminal-0");

    renderHook(() => usePanelActivationFocus(api, "page-1:terminal-0", focus));

    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("挂载即激活 + 无意图 → 不 focus（fromJSON 恢复豁免锚）", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();

    renderHook(() => usePanelActivationFocus(api, "page-1:terminal-0", focus));

    expect(focus).not.toHaveBeenCalled();
  });

  it("挂载未激活 + 有意图 → 不消费不 focus；其后激活事件 → focus（事件路径接管）", () => {
    const f = fakePanelApi({ isActive: false, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:editor-0");

    renderHook(() => usePanelActivationFocus(f.api, "page-1:editor-0", focus));
    expect(focus).not.toHaveBeenCalled();

    // 用户点页签激活 → 事件路径（无需意图）
    act(() => f.fireActive(true));
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("ready false 挂载（有意图+激活）→ 不 focus；ready 翻 true 补消费 → focus", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:editor-1");

    const { rerender } = renderHook(
      ({ ready }) => usePanelActivationFocus(api, "page-1:editor-1", focus, ready),
      { initialProps: { ready: false } },
    );
    expect(focus).not.toHaveBeenCalled();

    rerender({ ready: true });
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("意图 take 语义：消费一次后 ready 二次翻转不重复 focus", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    const focus = vi.fn();
    markPanelFocusIntent("page-1:terminal-1");

    const { rerender } = renderHook(
      ({ ready }) => usePanelActivationFocus(api, "page-1:terminal-1", focus, ready),
      { initialProps: { ready: false } },
    );
    rerender({ ready: true });
    rerender({ ready: false });
    rerender({ ready: true });

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
