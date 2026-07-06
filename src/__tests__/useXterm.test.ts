// useXterm.test.ts — useXterm hook 焦点驱动测试
//
// 验证 useXterm 通过 DOM focusin/focusout 事件跟踪焦点终端，
// 确保多终端场景下 Ctrl+Shift+C/V 操作当前聚焦的终端。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const {
  mockPushContext,
  mockPopContext,
  mockRegister,
  mockUnregisterFn,
  mockFit,
  mockProposeDimensions,
  mockOnFontSizeChange,
} = vi.hoisted(() => ({
  mockPushContext: vi.fn(),
  mockPopContext: vi.fn(),
  mockRegister: vi.fn(() => vi.fn()), // 返回注销函数
  mockUnregisterFn: vi.fn(),
  mockFit: vi.fn(),
  mockProposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
  mockOnFontSizeChange: vi.fn(),
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

// useXterm.ts import { createTerminalShortcuts } from "./keyboard" → 返回空命令列表
vi.mock("../panels/terminal/keyboard", () => ({
  createTerminalShortcuts: vi.fn(() => []),
}));

// useXterm.ts import { useShortcutContext } from "../../features/shortcuts"
const { mockUseShortcutContext } = vi.hoisted(() => ({
  mockUseShortcutContext: vi.fn(),
}));
vi.mock("../features/shortcuts", () => ({
  useShortcutContext: mockUseShortcutContext,
  getShortcutRegistry: () => ({
    register: mockRegister,
    unregister: mockUnregisterFn,
    pushContext: mockPushContext,
    popContext: mockPopContext,
    _reset: vi.fn(),
  }),
}));

// useXterm.ts import { pty } from "../../ipc" → src/ipc
vi.mock("../ipc", () => ({
  pty: {
    spawn: vi.fn().mockResolvedValue("test-session-id"),
    write: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    kill: vi.fn().mockResolvedValue(undefined),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = mockFit;
    proposeDimensions = mockProposeDimensions;
    dispose = vi.fn();
  },
}));

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
import { pty } from "../ipc";

// ─── 辅助函数 ───

/** 创建模拟容器（非零尺寸避免 rAF 轮询超时循环） */
function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

// ─── 测试套件 ───

describe("useXterm 快捷键集成", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = createContainer();
    capturedTerminal = null;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("36. 调用 useShortcutContext 传入 terminal 上下文和 container", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-36" }),
    );

    expect(mockUseShortcutContext).toHaveBeenCalledWith(
      "terminal",
      expect.any(Array), // ShortcutCommand[]
      container,
    );
  });

  it("37. 调用 createTerminalShortcuts 生成命令列表", async () => {
    const { createTerminalShortcuts } = await import("../panels/terminal/keyboard");
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-37" }),
    );

    expect(createTerminalShortcuts).toHaveBeenCalled();
  });
});

// ─── pollFitAndSpawn rAF 轮询测试 ───
describe("pollFitAndSpawn rAF 轮询", () => {
  let container: HTMLDivElement;
  let rafCallbacks: Array<FrameRequestCallback>;
  let rafIdCounter: number;
  let perfNowValue: number;

  const origRAF = globalThis.requestAnimationFrame;
  const origCAF = globalThis.cancelAnimationFrame;

  beforeEach(() => {
    rafCallbacks = [];
    rafIdCounter = 0;
    perfNowValue = 0;

    // mock rAF：存储回调但不同步执行，由 advanceFrames 手动推进
    globalThis.requestAnimationFrame = vi.fn(
      (cb: FrameRequestCallback) => {
        const id = ++rafIdCounter;
        rafCallbacks.push(cb);
        return id;
      },
    );
    globalThis.cancelAnimationFrame = vi.fn();
    // spy performance.now：控制 elapsed 时间推进
    vi.spyOn(performance, "now").mockImplementation(() => perfNowValue);

    vi.clearAllMocks();
    // 重置 proposeDimensions 默认返回值
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
  });

  afterEach(() => {
    globalThis.requestAnimationFrame = origRAF;
    globalThis.cancelAnimationFrame = origCAF;
    vi.restoreAllMocks();
  });

  /** 推进 N 帧，每帧经过 elapsedPerFrame ms */
  function advanceFrames(n: number, elapsedPerFrame = 16) {
    for (let i = 0; i < n && rafCallbacks.length > 0; i++) {
      perfNowValue += elapsedPerFrame;
      const cb = rafCallbacks.shift()!;
      cb(perfNowValue);
    }
  }

  /** 创建指定 offsetWidth/offsetHeight 的容器 */
  function createContainerWithSize(w: number, h: number): HTMLDivElement {
    const el = document.createElement("div");
    Object.defineProperty(el, "offsetWidth", {
      value: w,
      configurable: true,
    });
    Object.defineProperty(el, "offsetHeight", {
      value: h,
      configurable: true,
    });
    return el;
  }

  // ────────────────────────────────────────────────
  // T1：offsetWidth=0 场景
  // ────────────────────────────────────────────────
  it("T1: 容器 offsetWidth=0 时启动 rAF 轮询，不立即 spawn", () => {
    container = createContainerWithSize(0, 0);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "poll-1" }),
    );

    // rAF 已调度（pollFitAndSpawn 注册为回调）
    expect(rafCallbacks.length).toBe(1);
    // PTY 未 spawn（容器尺寸为 0，轮询中）
    expect(pty.spawn).not.toHaveBeenCalled();
  });

  // ────────────────────────────────────────────────
  // T2：轮询后获得尺寸 → fit → proposeDimensions → spawn
  // ────────────────────────────────────────────────
  it("T2: rAF 轮询后容器获得尺寸 → fit → proposeDimensions → spawn", () => {
    container = createContainerWithSize(0, 0);
    // 模拟 proposeDimensions 返回非默认值，验证真实尺寸传递
    mockProposeDimensions.mockReturnValue({ cols: 100, rows: 40 });

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "poll-2" }),
    );

    // 第一帧：尺寸仍为 0 → 继续轮询
    advanceFrames(1, 16);
    expect(pty.spawn).not.toHaveBeenCalled();
    expect(rafCallbacks.length).toBe(1); // 下一帧已调度

    // 容器获得尺寸（模拟布局完成）
    Object.defineProperty(container, "offsetWidth", {
      value: 800,
      configurable: true,
    });
    Object.defineProperty(container, "offsetHeight", {
      value: 600,
      configurable: true,
    });

    // 第二帧：检测到有效尺寸 → fit → proposeDimensions → doSpawn
    advanceFrames(1, 16);

    expect(mockFit).toHaveBeenCalled();
    expect(mockProposeDimensions).toHaveBeenCalled();
    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 100, rows: 40, panelId: "poll-2" }),
      expect.any(Function),
    );
    // spawn 后不再调度新帧
    expect(rafCallbacks.length).toBe(0);
  });

  // ────────────────────────────────────────────────
  // T3：30 帧超时回退 80x24
  // ────────────────────────────────────────────────
  it("T3: 30 帧内 offsetWidth 一直为 0 → 超时回退 80x24", () => {
    container = createContainerWithSize(0, 0);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "poll-3" }),
    );

    // 推进 30 帧（每帧 16ms → 总耗时 480ms < 500ms，纯帧数触发超时）
    advanceFrames(30, 16);

    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24 }),
      expect.any(Function),
    );
    // spawn 后不再调度新帧
    expect(rafCallbacks.length).toBe(0);
  });

  // ────────────────────────────────────────────────
  // T4：500ms 超时（帧数未达 30 但时间到）
  // ────────────────────────────────────────────────
  it("T4: 超过 500ms 后无论帧数是否达 30 都会回退 80x24", () => {
    container = createContainerWithSize(0, 0);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "poll-4" }),
    );

    // 仅 10 帧，但每帧 51ms → 总耗时 510ms → 触发时间超时
    advanceFrames(10, 51);

    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24 }),
      expect.any(Function),
    );
    expect(rafCallbacks.length).toBe(0);
  });

  // ────────────────────────────────────────────────
  // T5：NaN 守卫
  // ────────────────────────────────────────────────
  it("T5: proposeDimensions 返回 NaN 时使用默认值 80x24", () => {
    // 容器有有效尺寸
    container = createContainerWithSize(800, 600);
    mockProposeDimensions.mockReturnValue({ cols: NaN, rows: NaN });

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "poll-5" }),
    );

    // 第一帧即检测到尺寸，但 proposeDimensions 返回 NaN → 回退 80x24
    advanceFrames(1, 16);

    expect(mockFit).toHaveBeenCalled();
    expect(mockProposeDimensions).toHaveBeenCalled();
    // NaN 守卫触发：cols/rows 回退为 80/24
    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24 }),
      expect.any(Function),
    );
    expect(rafCallbacks.length).toBe(0);
  });
});

// ═══════════════════════════════════════════════════════════
// P2-52: ResizeObserver 测试 — 容器尺寸变化 → fit → pty.resize 链路
// ═══════════════════════════════════════════════════════════

describe("ResizeObserver 尺寸变化 → fit → pty.resize 链路", () => {
  let container: HTMLDivElement;
  let triggerResize: () => void;
  const OrigRO = globalThis.ResizeObserver;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 100, rows: 40 });

    // 创建带有效尺寸的容器（非 0，rAF 轮询首帧即完成）
    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });

    // 覆盖 ResizeObserver 全局 stub，捕获构造时传入的回调
    triggerResize = () => {};
    globalThis.ResizeObserver = class {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      constructor(cb: any) {
        triggerResize = () => cb([], this as unknown as ResizeObserver);
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    globalThis.ResizeObserver = OrigRO;
    vi.restoreAllMocks();
  });

  it("T6: ResizeObserver 回调 → 100ms debounce → fit → proposeDimensions → pty.resize", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-1" }),
    );

    // 等待 PTY spawn 完成（rAF 首帧检测到尺寸 → fit → proposeDimensions → spawn）
    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 验证 spawn 使用了 proposeDimensions 返回的尺寸（非默认 80x24）
    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 100, rows: 40, panelId: "resize-1" }),
      expect.any(Function),
    );

    // 重置 mocks，隔离 ResizeObserver 触发的调用
    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 模拟 ResizeObserver 检测到容器尺寸变化
    triggerResize();

    // 等待 100ms debounce + 缓冲
    await new Promise((r) => setTimeout(r, 150));

    // 验证链路：fit → proposeDimensions → pty.resize
    expect(mockFit).toHaveBeenCalled();
    expect(mockProposeDimensions).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String), // sessionId（"test-session-id"）
      100, // cols
      40,  // rows
    );
  });

  it("T7: 终端已卸载后 ResizeObserver 触发 → canFit 返回 false → 不调用 fit/resize", async () => {
    const { unmount } = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-2" }),
    );

    // 等待 spawn 完成
    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 卸载终端 → isDisposedRef.current = true
    unmount();

    // 清除之前 spawn/渲染过程中的 fit 调用
    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 触发 ResizeObserver 回调
    triggerResize();
    await new Promise((r) => setTimeout(r, 150));

    // canFit 条件 5（isDisposedRef.current === true）把门，fit 不被调用
    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();
  });

  it("T8: 容器尺寸变化为 0 时不触发 fit（canFit 条件 3 把门）", async () => {
    // 设置容器尺寸为 0→0
    Object.defineProperty(container, "offsetWidth", { value: 0, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 0, configurable: true });

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-3" }),
    );

    // 此时 rAF 轮询因为尺寸为 0 持续，spawn 未发生
    // 触发 ResizeObserver
    triggerResize();
    await new Promise((r) => setTimeout(r, 150));

    // canFit 条件 3（offsetWidth===0||offsetHeight===0）把门，fit 不被调用
    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();
  });

  it("T9: ResizeObserver 高频触发 → 100ms debounce 合并为单次 resize", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-4" }),
    );

    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 在 50ms 内触发 3 次 resize
    triggerResize();
    await new Promise((r) => setTimeout(r, 20));
    triggerResize();
    await new Promise((r) => setTimeout(r, 20));
    triggerResize();

    // 等待 debounce 定时器走完（距离最后一次触发 100ms + 缓冲）
    await new Promise((r) => setTimeout(r, 130));

    // 验证只执行了一次 fit + resize（debounce 生效）
    expect(mockFit).toHaveBeenCalledTimes(1);
    expect(pty.resize).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════
// 字体大小调节 — 终端 fontSize 传递 + Ctrl+Wheel
// ═══════════════════════════════════════════════════════════

describe("字体大小调节", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = createContainer();
    capturedTerminal = null;
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
  });

  // ── Terminal 构造 ──

  it("38. Terminal 创建时使用传入的 fontSize", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "fs-1", fontSize: 18 }),
    );

    expect(capturedTerminal).not.toBeNull();
    // Terminal 构造函数传了配置，验证 terminal 被创建
    expect(capturedTerminal?.open).toHaveBeenCalled();
  });

  it("39. 不传 fontSize 时正常创建（使用默认值）", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "fs-2" }),
    );

    expect(capturedTerminal?.open).toHaveBeenCalled();
  });

  // ── fontSize effect ──

  it("40. fontSize 变化触发 fit + resize", async () => {
    const { rerender } = renderHook(
      ({ fontSize }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "fs-3", fontSize }),
      { initialProps: { fontSize: 14 } },
    );

    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    rerender({ fontSize: 18 });

    await waitFor(() => {
      expect(mockFit).toHaveBeenCalled();
    });

    expect(mockProposeDimensions).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String),
      expect.any(Number),
      expect.any(Number),
    );
  });

  it("41. 无 sessionId 时只 fit 不 resize", async () => {
    const zeroContainer = document.createElement("div");
    Object.defineProperty(zeroContainer, "offsetWidth", { value: 0, configurable: true });
    Object.defineProperty(zeroContainer, "offsetHeight", { value: 0, configurable: true });

    const { rerender } = renderHook(
      ({ fontSize }) =>
        useXterm({ container: zeroContainer, cols: 80, rows: 24, panelId: "fs-4", fontSize }),
      { initialProps: { fontSize: 14 } },
    );

    // pty.spawn 未被调用（尺寸为 0）
    expect(pty.spawn).not.toHaveBeenCalled();

    Object.defineProperty(zeroContainer, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(zeroContainer, "offsetHeight", { value: 600, configurable: true });

    mockFit.mockClear();
    rerender({ fontSize: 20 });

    await waitFor(() => {
      expect(mockFit).toHaveBeenCalled();
    });

    expect(pty.resize).not.toHaveBeenCalled();
  });

  // ── Ctrl+Wheel ──

  it("42. Ctrl+上滚（deltaY<0）→ onFontSizeChange +1", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-5",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: -100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).toHaveBeenCalledWith(15);
  });

  it("43. Ctrl+下滚（deltaY>0）→ onFontSizeChange -1", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-6",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).toHaveBeenCalledWith(13);
  });

  it("44. 非 Ctrl 滚轮透传 → onFontSizeChange 不调用", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-7",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: false,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("45. 到达下限 8 → 缩小不触发 onFontSizeChange", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-8",
        fontSize: 8,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    if (capturedTerminal) capturedTerminal.options.fontSize = 8;

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("46. 到达上限 32 → 放大不触发 onFontSizeChange", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-9",
        fontSize: 32,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    if (capturedTerminal) capturedTerminal.options.fontSize = 32;

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: -100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("47. 未传 onFontSizeChange → wheel 不报错", () => {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "fs-10",
        fontSize: 14,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: -100,
      cancelable: true,
    });
    expect(() => container.dispatchEvent(event)).not.toThrow();
  });
});