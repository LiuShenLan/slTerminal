// useXterm.test.ts — useXterm hook 焦点驱动测试
//
// 验证 useXterm 通过 DOM focusin/focusout 事件跟踪焦点终端，
// 确保多终端场景下 Ctrl+Shift+C/V 操作当前聚焦的终端。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";

// ─── Hoisted mocks ───
const {
  mockSetActiveTerminal,
  mockClearActiveTerminalIfMine,
  mockInstallKeyboardHandler,
} = vi.hoisted(() => ({
  mockSetActiveTerminal: vi.fn(),
  mockClearActiveTerminalIfMine: vi.fn(),
  mockInstallKeyboardHandler: vi.fn(),
}));

// ─── 捕获 mock Terminal 实例（供测试验证 addEventListener 调用） ───
let capturedTerminal: {
  element: HTMLDivElement;
  open: ReturnType<typeof vi.fn>;
  loadAddon: ReturnType<typeof vi.fn>;
  dispose: ReturnType<typeof vi.fn>;
  onData: ReturnType<typeof vi.fn>;
  write: ReturnType<typeof vi.fn>;
  writeln: ReturnType<typeof vi.fn>;
  focus: ReturnType<typeof vi.fn>;
  getSelection: ReturnType<typeof vi.fn>;
  paste: ReturnType<typeof vi.fn>;
  options: Record<string, unknown>;
} | null = null;

// ─── Module mocks（路径相对于被测源文件 useXterm.ts 的 import 解析） ───
vi.mock("@xterm/xterm", () => {
  class MockTerminal {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    element: any;
    open = vi.fn();
    loadAddon = vi.fn();
    dispose = vi.fn();
    onData = vi.fn();
    write = vi.fn();
    writeln = vi.fn();
    focus = vi.fn();
    getSelection = vi.fn(() => "");
    paste = vi.fn();
    options: Record<string, unknown> = {};

    constructor() {
      const el = document.createElement("div");
      this.element = el;
      capturedTerminal = this as unknown as typeof capturedTerminal;
    }
  }
  return { Terminal: MockTerminal };
});

// useXterm.ts import { ... } from "./keyboard" → src/panels/terminal/keyboard
vi.mock("../panels/terminal/keyboard", () => ({
  setActiveTerminal: mockSetActiveTerminal,
  clearActiveTerminalIfMine: mockClearActiveTerminalIfMine,
  installKeyboardHandler: mockInstallKeyboardHandler,
}));

// useXterm.ts import { pty } from "../../ipc" → src/ipc
vi.mock("../ipc", () => ({
  pty: {
    spawn: vi.fn().mockResolvedValue("test-session-id"),
    write: vi.fn(),
    resize: vi.fn(),
    kill: vi.fn(),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  },
}));

vi.mock("@xterm/addon-fit", () => {
  class MockFitAddon {
    fit = vi.fn();
    proposeDimensions = vi.fn(() => ({ cols: 80, rows: 24 }));
    dispose = vi.fn();
  }
  return { FitAddon: MockFitAddon };
});

vi.mock("@xterm/addon-webgl", () => {
  class MockWebglAddon {
    dispose = vi.fn();
    onContextLoss = vi.fn();
  }
  return { WebglAddon: MockWebglAddon };
});

// useXterm.ts import { TerminalRegistry } from "./TerminalRegistry"
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn(),
    get: vi.fn(),
    remove: vi.fn(),
  },
}));

// 导入被测模块（mocks 就绪后）
import { useXterm } from "../panels/terminal/useXterm";

// ─── 辅助函数 ───

/** 创建模拟容器（非零尺寸避免 rAF 轮询超时循环） */
function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

// ─── 测试套件 ───

describe("useXterm 焦点驱动", () => {
  let container: HTMLDivElement;
  // 用普通变量记录，避免 vi.spyOn prototype 带来的 "not function/class" 警告
  let focusinHandlers: Array<EventListenerOrEventListenerObject> = [];
  let focusoutHandlers: Array<EventListenerOrEventListenerObject> = [];
  let removedFocusinCount = 0;
  let removedFocusoutCount = 0;
  const origAddEventListener = HTMLDivElement.prototype.addEventListener;
  const origRemoveEventListener = HTMLDivElement.prototype.removeEventListener;

  beforeEach(() => {
    container = createContainer();
    capturedTerminal = null;
    focusinHandlers = [];
    focusoutHandlers = [];
    removedFocusinCount = 0;
    removedFocusoutCount = 0;

    // 拦截 addEventListener：记录 focusin/focusout handler
    HTMLDivElement.prototype.addEventListener = function (
      this: HTMLDivElement,
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ) {
      if (type === "focusin") {
        focusinHandlers.push(listener);
      } else if (type === "focusout") {
        focusoutHandlers.push(listener);
      }
      return origAddEventListener.call(this, type, listener, options);
    };

    // 拦截 removeEventListener：计数 focusin/focusout 移除
    HTMLDivElement.prototype.removeEventListener = function (
      this: HTMLDivElement,
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ) {
      if (type === "focusin") {
        removedFocusinCount++;
      } else if (type === "focusout") {
        removedFocusoutCount++;
      }
      return origRemoveEventListener.call(this, type, listener, options);
    };
  });

  afterEach(() => {
    HTMLDivElement.prototype.addEventListener = origAddEventListener;
    HTMLDivElement.prototype.removeEventListener = origRemoveEventListener;
    vi.clearAllMocks();
  });

  // ─── 36–37: 事件注册 ───

  it("36. 挂载时注册 focusin 事件监听", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-36" }),
    );
    expect(focusinHandlers.length).toBeGreaterThanOrEqual(1);
    expect(typeof focusinHandlers[0]).toBe("function");
  });

  it("37. 挂载时注册 focusout 事件监听", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-37" }),
    );
    expect(focusoutHandlers.length).toBeGreaterThanOrEqual(1);
    expect(typeof focusoutHandlers[0]).toBe("function");
  });

  // ─── 38–39: 回调链路 ───

  it("38. focusin 触发 → 调用 setActiveTerminal(term)", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-38" }),
    );

    // 通过 DOM 事件分发触发 focusin handler（走完整 DOM → handler → keyboard 链路）
    capturedTerminal!.element.dispatchEvent(
      new FocusEvent("focusin", { bubbles: true }),
    );

    expect(mockSetActiveTerminal).toHaveBeenCalledWith(capturedTerminal);
  });

  it("39. focusout 触发 → clearActiveTerminalIfMine(term)，内部焦点转移不触发", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-39" }),
    );

    // 场景 1：焦点移到终端外（relatedTarget = null）→ 清空
    capturedTerminal!.element.dispatchEvent(
      new FocusEvent("focusout", {
        bubbles: true,
        relatedTarget: null,
      }),
    );
    expect(mockClearActiveTerminalIfMine).toHaveBeenCalledWith(
      capturedTerminal,
    );

    // 场景 2：焦点移到终端内部子元素 → 不清空
    mockClearActiveTerminalIfMine.mockClear();
    const innerEl = document.createElement("span");
    capturedTerminal!.element.appendChild(innerEl);
    capturedTerminal!.element.dispatchEvent(
      new FocusEvent("focusout", {
        bubbles: true,
        relatedTarget: innerEl,
      }),
    );
    expect(mockClearActiveTerminalIfMine).not.toHaveBeenCalled();

    innerEl.remove();
  });

  // ─── 40: cleanup ───

  it("40. 组件卸载时移除焦点监听并调用 clearActiveTerminalIfMine(term)", () => {
    const { unmount } = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-40" }),
    );

    unmount();

    // 验证移除 focusin/focusout 监听
    expect(removedFocusinCount).toBeGreaterThanOrEqual(1);
    expect(removedFocusoutCount).toBeGreaterThanOrEqual(1);

    // cleanup 中调用 clearActiveTerminalIfMine
    expect(mockClearActiveTerminalIfMine).toHaveBeenCalledWith(
      capturedTerminal,
    );
  });
});
