// forwardGlobalShortcuts.test.ts — iframe 全局键转发单元测试
//
// 覆盖：命中全局键 → preventDefault + 重放到 window（props 正确）；非全局键不转发；
//       多全局绑定各自命中；空绑定全不转发；detach 后失效；修饰键指纹区分。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// mock 注册表的 exportContextBindings（formatKeystroke 用真实实现）
const { mockExportContextBindings } = vi.hoisted(() => ({
  mockExportContextBindings: vi.fn<() => { id: string; keystroke: string }[]>(() => []),
}));
vi.mock("../features/shortcuts/ShortcutRegistry", () => ({
  getShortcutRegistry: () => ({ exportContextBindings: mockExportContextBindings }),
}));

import { attachGlobalShortcutForwarder } from "../features/shortcuts/forwardGlobalShortcuts";

function keydown(opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean; code: string; key?: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    metaKey: opts.metaKey ?? false,
    code: opts.code,
    key: opts.key ?? "",
    bubbles: true,
    cancelable: true,
  });
}

describe("attachGlobalShortcutForwarder", () => {
  let detach: (() => void) | null = null;
  let dispatchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mockExportContextBindings.mockReturnValue([]);
    // spy window.dispatchEvent（默认调用原始实现——重放到 window 无害，无 registry 监听）
    dispatchSpy = vi.spyOn(window, "dispatchEvent");
  });

  afterEach(() => {
    detach?.();
    detach = null;
    dispatchSpy.mockRestore();
  });

  it("命中全局键（Ctrl+W）→ preventDefault + 重放到 window（props 正确）", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, code: "KeyW", key: "w" });
    document.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(true);
    // 重放的合成事件
    const calls = dispatchSpy.mock.calls as unknown as Array<[Event]>;
    const replayed = calls
      .map((c) => c[0])
      .find((ev): ev is KeyboardEvent => ev instanceof KeyboardEvent && ev !== e);
    expect(replayed).toBeDefined();
    expect(replayed!.type).toBe("keydown");
    expect(replayed!.ctrlKey).toBe(true);
    expect(replayed!.shiftKey).toBe(false);
    expect(replayed!.code).toBe("KeyW");
    expect(replayed!.key).toBe("w");
  });

  it("非全局键（Ctrl+A）→ 不 preventDefault、不重放", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const before = dispatchSpy.mock.calls.length;
    const e = keydown({ ctrlKey: true, code: "KeyA" });
    document.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(false);
    // 无新增的 window.dispatchEvent 调用（除 document.dispatchEvent 本身，不经 window）
    expect(dispatchSpy.mock.calls.length).toBe(before);
  });

  it("多全局绑定动态 → 各自命中", () => {
    mockExportContextBindings.mockReturnValue([
      { id: "global.closeTab", keystroke: "Ctrl+KeyW" },
      { id: "global.other", keystroke: "Ctrl+Shift+KeyP" },
    ]);
    detach = attachGlobalShortcutForwarder(document);

    const e1 = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e1);
    expect(e1.defaultPrevented).toBe(true);

    const e2 = keydown({ ctrlKey: true, shiftKey: true, code: "KeyP" });
    document.dispatchEvent(e2);
    expect(e2.defaultPrevented).toBe(true);
  });

  it("空全局绑定 → 全不转发", () => {
    mockExportContextBindings.mockReturnValue([]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it("detach 后再派发 → 无反应", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    const d = attachGlobalShortcutForwarder(document);
    d(); // detach
    detach = null;

    const e = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it("修饰键指纹区分：绑定 Ctrl+W 不匹配 Ctrl+Shift+W", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, shiftKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });
});
