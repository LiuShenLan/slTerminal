// usePanelFocus.test.ts — 面板焦点上下文/聚焦实例跟踪 hook 单元测试
//
// 覆盖：focusin → pushContext + onActivate；focusout(离开子树) → popContext + onDeactivate；
//       内部焦点转移不触发；卸载 → popContext + onDeactivate。用真实 ShortcutRegistry + 真实 DOM 事件。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { usePanelFocus } from "../features/shortcuts/usePanelFocus";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";

const registry = getShortcutRegistry();

describe("usePanelFocus", () => {
  let element: HTMLDivElement;
  let child: HTMLElement;

  beforeEach(() => {
    registry._reset();
    element = document.createElement("div");
    child = document.createElement("input");
    element.appendChild(child);
    document.body.appendChild(element);
  });

  afterEach(() => {
    document.body.removeChild(element);
    registry._reset();
  });

  it("element 为 null 时不操作", () => {
    const onActivate = vi.fn();
    renderHook(() => usePanelFocus("editor", null, onActivate));
    expect(onActivate).not.toHaveBeenCalled();
    expect(registry._contextStack()).toEqual([]);
  });

  it("focusin → pushContext + onActivate", () => {
    const onActivate = vi.fn();
    renderHook(() => usePanelFocus("editor", element, onActivate));

    element.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));

    expect(registry._contextStack()).toEqual(["editor"]);
    expect(onActivate).toHaveBeenCalledTimes(1);
  });

  it("focusout 离开子树 → popContext + onDeactivate", () => {
    const onDeactivate = vi.fn();
    renderHook(() => usePanelFocus("editor", element, undefined, onDeactivate));

    element.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    // relatedTarget 在子树外 → 视为离开
    const outside = document.createElement("div");
    element.dispatchEvent(new FocusEvent("focusout", { bubbles: true, relatedTarget: outside }));

    expect(registry._contextStack()).toEqual([]);
    expect(onDeactivate).toHaveBeenCalledTimes(1);
  });

  it("focusout 到子树内（内部焦点转移）→ 不触发", () => {
    const onDeactivate = vi.fn();
    renderHook(() => usePanelFocus("editor", element, undefined, onDeactivate));

    element.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    // relatedTarget 在 element 子树内 → 不算 blur
    element.dispatchEvent(new FocusEvent("focusout", { bubbles: true, relatedTarget: child }));

    expect(registry._contextStack()).toEqual(["editor"]);
    expect(onDeactivate).not.toHaveBeenCalled();
  });

  it("卸载 → popContext + onDeactivate", () => {
    const onDeactivate = vi.fn();
    const { unmount } = renderHook(() => usePanelFocus("terminal", element, undefined, onDeactivate));

    element.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    expect(registry._contextStack()).toEqual(["terminal"]);

    unmount();
    expect(registry._contextStack()).toEqual([]);
    expect(onDeactivate).toHaveBeenCalled();
  });
});
