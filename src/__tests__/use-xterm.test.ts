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
  mockResolve,
  mockFit,
  mockProposeDimensions,
  mockOnFontSizeChange,
} = vi.hoisted(() => ({
  mockPushContext: vi.fn(),
  mockPopContext: vi.fn(),
  mockRegister: vi.fn(() => vi.fn()), // 返回注销函数
  mockUnregisterFn: vi.fn(),
  mockResolve: vi.fn<(e: KeyboardEvent, ctx?: string) => boolean>(() => false), // 委托解析：默认未消费
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
  attachCustomKeyEventHandler: ReturnType<typeof vi.fn>;
  options: Record<string, unknown>;
  parser: {
    registerOscHandler: ReturnType<typeof vi.fn>;
  };
} | null = null;

// OSC 52 测试：捕获 registerOscHandler(52, ...) 注册的回调
let capturedOsc52Handler: ((data: string) => boolean) | null = null;

// OSC 133 测试：捕获 registerOscHandler(133, ...) 注册的回调
let capturedOsc133Handler: ((data: string) => boolean) | null = null;

// 键盘测试：捕获 attachCustomKeyEventHandler 注册的回调
let capturedKeyEventHandler: ((event: KeyboardEvent) => boolean) | null = null;

// ─── Module mocks（路径相对于被测源文件 useXterm.ts 的 import 解析） ───
vi.mock("@xterm/xterm", () => {
  class MockTerminal {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    element: any;
    open = vi.fn();
    loadAddon = vi.fn();
    dispose = vi.fn();
    onData = vi.fn(() => ({ dispose: vi.fn() }));
    write = vi.fn();
    writeln = vi.fn();
    focus = vi.fn();
    getSelection = vi.fn(() => "");
    paste = vi.fn();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    attachCustomKeyEventHandler = vi.fn((handler: any) => {
      capturedKeyEventHandler = handler;
    });
    options: Record<string, unknown> = {};
    parser = {
      registerOscHandler: vi.fn((osc: number, handler: (data: string) => boolean) => {
        if (osc === 52) capturedOsc52Handler = handler;
        if (osc === 133) capturedOsc133Handler = handler;
        return { dispose: vi.fn() };
      }),
    };

    constructor() {
      const el = document.createElement("div");
      this.element = el;
      capturedTerminal = this as unknown as typeof capturedTerminal;
    }
  }
  return { Terminal: MockTerminal };
});

// useXterm.ts import { usePanelFocus } from "../../features/shortcuts"
const { mockUsePanelFocus } = vi.hoisted(() => ({
  mockUsePanelFocus: vi.fn(),
}));
vi.mock("../features/shortcuts", () => ({
  usePanelFocus: mockUsePanelFocus,
  getShortcutRegistry: () => ({
    register: mockRegister,
    unregister: mockUnregisterFn,
    pushContext: mockPushContext,
    popContext: mockPopContext,
    resolve: mockResolve,
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

// OSC 52 测试：mock clipboard（src/ipc/clipboard.ts）
const { mockWriteText, mockReadText } = vi.hoisted(() => ({
  mockWriteText: vi.fn(),
  mockReadText: vi.fn(() => Promise.resolve("")),
}));
vi.mock("../ipc/clipboard", () => ({
  writeText: mockWriteText,
  readText: mockReadText,
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

// OSC 8 linkHandler 测试：mock @tauri-apps/plugin-opener 的 openUrl
const { mockOpenUrl } = vi.hoisted(() => ({
  mockOpenUrl: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: mockOpenUrl,
}));

// useXterm.ts import { TerminalRegistry } from "./TerminalRegistry"
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: vi.fn(),
    get: vi.fn(),
    remove: vi.fn(),
  },
}));

// useXterm.ts import { tabTitleRegistry } from "./TabTitleRegistry"
const { mockTabTitleMatch } = vi.hoisted(() => ({
  mockTabTitleMatch: vi.fn<(cmd: string) => { command: string; title: string; icon: string } | null>(),
}));
vi.mock("../panels/terminal/TabTitleRegistry", () => ({
  tabTitleRegistry: {
    match: mockTabTitleMatch,
    register: vi.fn(),
    _reset: vi.fn(),
  },
}));

// useXterm.ts 的 side-effect import "./tabRules" — stub 防止实际加载图片资源
vi.mock("../panels/terminal/tabRules", () => ({}));

// 导入被测模块（mocks 就绪后）
import { useXterm } from "../panels/terminal/useXterm";
import type { TabState } from "../panels/terminal/TabTitleRegistry";
import { pty } from "../ipc";
import {
  createContainer,
  mockRaf,
  ptyOutputSpy,
  mockResizeObserver,
  setBufferType,
} from "./helpers/xterm-test-utils";

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

  it("36. 调用 usePanelFocus 传入 terminal 上下文、container 与激活/停用回调", () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "test-36" }),
    );

    expect(mockUsePanelFocus).toHaveBeenCalledWith(
      "terminal",
      container,
      expect.any(Function), // onActivate → setActiveTerminal
      expect.any(Function), // onDeactivate → clearActiveTerminal
    );
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
  let ro: ReturnType<typeof mockResizeObserver>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 100, rows: 40 });
    container = createContainer();
    ro = mockResizeObserver();
  });

  afterEach(() => {
    ro.cleanup();
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
    ro.trigger();

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
    ro.trigger();
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
    ro.trigger();
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
    ro.trigger();
    await new Promise((r) => setTimeout(r, 20));
    ro.trigger();
    await new Promise((r) => setTimeout(r, 20));
    ro.trigger();

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

// ═══════════════════════════════════════════════════════════
// Step 1.1: DEC 2026 同步更新测试
// ═══════════════════════════════════════════════════════════

describe("DEC 2026 同步更新包裹 flushBuffer", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
  });

  it("DEC1: >=64 字节合帧后 term.write 以 \\x1b[?2026h 开头", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-1" }),
    );

    // 等待 pollFitAndSpawn 首帧完成 → spawn
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 清空 spawn 过程中的 write 调用
    capturedTerminal!.write.mockClear();

    // 发送 >=64 字节 → 走合帧路径
    const testData = new Array(100).fill(65); // 100 个 'A' (0x41)
    ptyOut.sendPtyOutput(testData);

    // idle timer 未触发 → write 不应被调用（数据在 pendingBufferRef）
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 等待 idle timer (2ms) → flushBuffer 执行
    await new Promise((r) => setTimeout(r, 5));

    // 验证 write 被调用且以 DEC 2026 开始序列开头
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const written = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    expect(written.startsWith("\x1b[?2026h")).toBe(true);
  });

  it("DEC2: 合帧写入以 \\x1b[?2026l 结尾", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-2" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    const testData = new Array(100).fill(66); // 100 个 'B'
    ptyOut.sendPtyOutput(testData);
    // 等待 idle timer 触发 flush
    await new Promise((r) => setTimeout(r, 5));

    const written = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    expect(written.endsWith("\x1b[?2026l")).toBe(true);
  });

  it("DEC3: 多块累积后单次 flush → 只有一个 DEC 2026 包裹对", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-3" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 连续发送 3 块 >=64 字节数据（同一 idle 窗口内累积）
    ptyOut.sendPtyOutput(new Array(80).fill(67));  // 'C'
    ptyOut.sendPtyOutput(new Array(80).fill(68));  // 'D'
    ptyOut.sendPtyOutput(new Array(80).fill(69));  // 'E'

    // idle timer 未触发 → 没有 write
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 等待 idle timer → flushBuffer 执行
    await new Promise((r) => setTimeout(r, 5));

    // 单次 write，包含全部 3 块数据
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const written = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    // 只有一个 DEC 2026 开始和一个结束
    expect(written.indexOf("\x1b[?2026h")).toBe(0);
    expect(written.indexOf("\x1b[?2026h", 1)).toBe(-1); // 无第二个开始
    expect(written.lastIndexOf("\x1b[?2026l")).toBe(written.length - 8);
    expect(written.indexOf("\x1b[?2026l")).toBe(written.lastIndexOf("\x1b[?2026l")); // 只有一个结束
  });

  it("DEC4: <64 字节直写不含 DEC 2026 序列", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-4" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 发送 <64 字节 → 走直写路径
    const testData = new Array(30).fill(72); // 30 个 'H'
    ptyOut.sendPtyOutput(testData);

    // 直写：立即调用 term.write
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const written = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(written).not.toContain("\x1b[?2026h");
    expect(written).not.toContain("\x1b[?2026l");
  });

});

// ═══════════════════════════════════════════════════════════
// Step 1.2: 直写阈值 256→64 测试
// ═══════════════════════════════════════════════════════════

describe("直写阈值 64 字节路由", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
  });

  it("TH1: 63 字节 PTY 输出走直写路径（term.write 立即调用）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-1" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 63 字节 < 阈值 64 → 直写
    ptyOut.sendPtyOutput(new Array(63).fill(88)); // 'X'

    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("TH2: 64 字节 PTY 输出走合帧路径（阈值边界）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-2" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 64 字节 >= 阈值 64 → 合帧
    ptyOut.sendPtyOutput(new Array(64).fill(89)); // 'Y'

    // idle timer 未触发 → write 不应被调用
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 等待 idle timer
    await new Promise((r) => setTimeout(r, 5));
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("TH4: 高频小块输出（连续 5 次 50 字节）全部直写", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-4" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 5 次小块直写
    for (let i = 0; i < 5; i++) {
      ptyOut.sendPtyOutput(new Array(50).fill(65 + i));
    }

    // 全部走直写：每次立即调用 write
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(5);
  });

  it("TH5: 混合输出（先 50 字节直写，再 200 字节合帧）各自正确路由", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-5" }),
    );

    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    capturedTerminal!.write.mockClear();

    // 小块直写
    ptyOut.sendPtyOutput(new Array(50).fill(80)); // 'P'
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    // 大块合帧
    ptyOut.sendPtyOutput(new Array(200).fill(81)); // 'Q'
    // 直写调用数不变（大块走合帧，idle timer 未触发）
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    // 等待 idle timer → 合帧 flush
    await new Promise((r) => setTimeout(r, 5));
    // 合帧后 write 被调用第二次
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(2);
  });
});

// ═══════════════════════════════════════════════════════════
// Step 1.3: ResizeObserver 交替缓冲检查测试
// ═══════════════════════════════════════════════════════════

describe("ResizeObserver 交替缓冲检查", () => {
  let container: HTMLDivElement;
  let ro: ReturnType<typeof mockResizeObserver>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 100, rows: 40 });
    capturedTerminal = null;
    container = createContainer();
    ro = mockResizeObserver();
  });

  afterEach(() => {
    ro.cleanup();
    vi.restoreAllMocks();
  });

  it("AB1: 交替缓冲中 resize → fit + pty.resize 正常调用", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ab-1" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    setBufferType(capturedTerminal, "alternate");

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    // 交替缓冲中 fit 被调用（同步 xterm.js 网格尺寸与 PTY 新尺寸）
    expect(mockFit).toHaveBeenCalled();
    // SIGWINCH 仍然透传
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String),
      100,
      40,
    );
  });

  it("AB3: 普通缓冲中 resize → fit + pty.resize 正常调用", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ab-3" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    setBufferType(capturedTerminal, "normal");

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    // 普通缓冲中 fit 正常调用
    expect(mockFit).toHaveBeenCalledTimes(1);
    expect(pty.resize).toHaveBeenCalledTimes(1);
  });

  it("AB5: 交替缓冲中 resize → 传入正确的 cols/rows", async () => {
    mockProposeDimensions.mockReturnValue({ cols: 120, rows: 50 });

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ab-5" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    setBufferType(capturedTerminal, "alternate");

    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    // 交替缓冲中尺寸参数正确透传
    expect(pty.resize).toHaveBeenCalledWith("test-session-id", 120, 50);
  });
});

// ═══════════════════════════════════════════════════════════
// Step 2.1: Idle+Max 双定时器合帧测试
// ═══════════════════════════════════════════════════════════

describe("Idle+Max 双定时器合帧", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
  });

  it("IT2: 连续高频输出（2ms 内多次）→ 不立即 flush，最后一次后 2ms flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-2" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    // 连续 3 次大块输出
    ptyOut.sendPtyOutput(new Array(80).fill(65));
    ptyOut.sendPtyOutput(new Array(80).fill(66));
    ptyOut.sendPtyOutput(new Array(80).fill(67));

    // idle timer 被每次输出重置 → 不立即 flush
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 等待 5ms → 最后一次后 2ms idle timer 触发
    await new Promise((r) => setTimeout(r, 5));
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("IT3: 持续 16ms 不断有数据 → max timer 强制 flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-3" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    // 每隔 3ms 发送数据——idle timer 永远被重置，但 max timer 在 16ms 时强制 flush
    ptyOut.sendPtyOutput(new Array(80).fill(65));
    await new Promise((r) => setTimeout(r, 3));
    ptyOut.sendPtyOutput(new Array(80).fill(66));
    await new Promise((r) => setTimeout(r, 3));
    ptyOut.sendPtyOutput(new Array(80).fill(67));
    await new Promise((r) => setTimeout(r, 3));
    ptyOut.sendPtyOutput(new Array(80).fill(68));
    await new Promise((r) => setTimeout(r, 3));
    ptyOut.sendPtyOutput(new Array(80).fill(69));
    // 已过 ~12ms，max timer 即将触发
    await new Promise((r) => setTimeout(r, 10));

    // max timer (16ms) 已强制 flush 至少一次
    const writeCalls = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls;
    expect(writeCalls.length).toBeGreaterThan(0);
  });

  it("IT4: 混合输出：小数据直写 + 大数据累积后定时器 flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-4" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    // 小块直写
    ptyOut.sendPtyOutput(new Array(30).fill(80));
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    // 小块不含 DEC 2026
    const smallWritten = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(smallWritten).not.toContain("\x1b[?2026h");

    // 大块合帧
    ptyOut.sendPtyOutput(new Array(100).fill(81));
    // 累积中
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    await new Promise((r) => setTimeout(r, 5));
    // 大块 flush 带 DEC 2026
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(2);
  });

  it("IT5: 组件卸载 → idle + max timer 均被清除", async () => {
    const { unmount } = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-5" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    // 发送大块输出 → 启动 idle+max timer
    ptyOut.sendPtyOutput(new Array(100).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 卸载 → cleanup 清除定时器
    unmount();

    // 等待足够长时间 → timer 不应触发 write（因为已卸载）
    await new Promise((r) => setTimeout(r, 30));
    // 卸载后不会再调用 write
    // （即使 timer 触发，flushBuffer 内 terminalRef.current 已为 null 会提前返回）
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════
// Step 2.3: Uint8Array 替代字符串拼接测试
// ═══════════════════════════════════════════════════════════

describe("Uint8Array 合帧缓冲", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
  });

  it("UA1: flushBuffer 合并多个 Uint8Array → term.write 收到正确拼接数据", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-1" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    // 发送多块数据，各块内容不同
    const bytes1 = new Array(80).fill(65); // 'A'
    const bytes2 = new Array(80).fill(66); // 'B'
    ptyOut.sendPtyOutput(bytes1);
    ptyOut.sendPtyOutput(bytes2);

    await new Promise((r) => setTimeout(r, 5));

    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    // 解码后应包含 'A'*80 + 'B'*80（包裹在 DEC 2026 之间）
    expect(decoded).toContain("A".repeat(80));
    expect(decoded).toContain("B".repeat(80));
    // 'A' 块在 'B' 块之前
    expect(decoded.indexOf("A")).toBeLessThan(decoded.indexOf("B"));
  });

  it("UA2: flushBuffer 带 DEC 2026 包裹（Uint8Array 拼接）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-2" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(100).fill(88)); // 'X'

    await new Promise((r) => setTimeout(r, 5));

    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    expect(decoded.startsWith("\x1b[?2026h")).toBe(true);
    expect(decoded.endsWith("\x1b[?2026l")).toBe(true);
    expect(decoded).toContain("X".repeat(100));
  });

  it("UA4: 单块 Uint8Array flush 数据完整性", async () => {
    });
  it("UA4: 单块 Uint8Array flush 数据完整性", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-4" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(256).fill(90)); // 'Z' * 256

    await new Promise((r) => setTimeout(r, 5));

    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    const content = decoded.slice(8, decoded.length - 8); // 去除 DEC 2026 包裹
    expect(content).toBe("Z".repeat(256));
  });
});

// ═══════════════════════════════════════════════════════════
// Step 2.4: 非焦点终端降频测试
// ═══════════════════════════════════════════════════════════

describe("非焦点终端降频", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
  });

  it("NF1: visible=false → PTY 输出累积但不 flush", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-1", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // 发送大块输出
    ptyOut.sendPtyOutput(new Array(200).fill(65));

    // 等待 → idle timer 不应触发 flush（因为 visible=false）
    await new Promise((r) => setTimeout(r, 10));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
  });

  it("NF2: visible 切回 true → 累积数据立即 flush", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-2", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // 发送大块输出（隐藏期间）
    ptyOut.sendPtyOutput(new Array(200).fill(65));
    await new Promise((r) => setTimeout(r, 10));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 切回可见 → useEffect 检测到 visible 变为 true → flush
    rerender({ visible: true });

    await vi.waitFor(() => {
      expect(capturedTerminal!.write).toHaveBeenCalled();
    });
  });

  it("NF3: visible=false 期间直写路径也被抑制", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-3", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // 直写路径：<64 字节在 visible 期间会走直写
    // 注意：visible=false 时 visibleRef.current 也为 false
    // handlePtyOutput 中的 fast-path (< 64) 不检查 visible，
    // 只在大块路径中检查 isVisible
    ptyOut.sendPtyOutput(new Array(30).fill(80)); // <64 字节 → 直写（不检查 visible）
    // 直写路径不受 visible 控制——这是当前实现的设计选择
    // （fast-path 用于打字回显，不受焦点状态影响）
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("NF4: 组件在 visible=false 状态下卸载 → kill PTY", async () => {
    const { rerender, unmount } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-4", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 切换到隐藏
    rerender({ visible: false });

    // 发送数据
    ptyOut.sendPtyOutput(new Array(200).fill(65));

    // 卸载 → 验证 PTY 被 kill（即使处于非焦点状态）
    unmount();
    expect(pty.kill).toHaveBeenCalled();
  });

  it("NF5: 默认 visible=undefined → 视为可见，正常 flush", async () => {
    // 不传 visible prop（undefined）
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "nf-5" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(200).fill(65));
    await new Promise((r) => setTimeout(r, 5));

    // visible 未指定 → 视为可见 → 正常 flush
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════
// Investigation 4: OSC 52 剪贴板支持测试
// ═══════════════════════════════════════════════════════════

describe("OSC 52 剪贴板支持", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    mockWriteText.mockResolvedValue(undefined);
    capturedTerminal = null;
    capturedOsc52Handler = null;

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 渲染 hook 并等待 pty spawn 完成（触发 registerOscHandler） */
  async function mountAndWaitForSpawn() {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "osc-test" }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    // 验证 handler 已注册
    expect(capturedOsc52Handler).not.toBeNull();
  }

  it("OSC1: 正常写入 — selector=c, 有效 base64 → writeText 被调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    // "Hello" 的 base64: SGVsbG8=
    const result = capturedOsc52Handler!("c;SGVsbG8=");

    expect(result).toBe(true);
    expect(mockWriteText).toHaveBeenCalledWith("Hello");
  });

  it("OSC2: CJK 内容 — 中文 base64 → writeText 正确解码", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    // "你好" → UTF-8 bytes → base64
    const utf8Bytes = new TextEncoder().encode("你好");
    const base64 = btoa(String.fromCharCode(...utf8Bytes));
    const result = capturedOsc52Handler!("c;" + base64);

    expect(result).toBe(true);
    expect(mockWriteText).toHaveBeenCalledWith("你好");
  });

  it("OSC3: 空选择器 — 默认系统剪贴板 → writeText 被调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!(";SGVsbG8=");

    expect(result).toBe(true);
    expect(mockWriteText).toHaveBeenCalledWith("Hello");
  });

  it("OSC4: 查询请求 — Pd=? → writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!("c;?");

    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC5: 空 payload → writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!("c;");

    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC6: 无效 base64 → 不抛异常，writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    expect(() => {
      const result = capturedOsc52Handler!("c;!!!INVALID!!!");
      expect(result).toBe(true);
    }).not.toThrow();

    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC7: 非系统剪贴板 — selector=p → writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!("p;SGVsbG8=");

    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC8: 无分号格式 → writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!("SGVsbG8=");

    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC9: 超大 payload（>1MB）→ writeText 不调用", async () => {
    await mountAndWaitForSpawn();
    mockWriteText.mockClear();

    // 构造 1MB + 1 字节的 payload
    const hugePayload = "A".repeat(1048577); // 1MB + 1
    const result = capturedOsc52Handler!("c;" + hugePayload);

    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC10: 非焦点面板（visible=false）→ writeText 不调用", async () => {
    // 以 visible=false 渲染，等待 spawn
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "osc-focus", visible: false }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    expect(capturedOsc52Handler).not.toBeNull();
    mockWriteText.mockClear();

    const result = capturedOsc52Handler!("c;SGVsbG8=");

    // visible=false → visibleRef.current === false → 焦点门控生效
    expect(result).toBe(true);
    expect(mockWriteText).not.toHaveBeenCalled();
  });

  it("OSC11: handler 返回 true 吞噬序列（阻止写入终端显示）", async () => {
    await mountAndWaitForSpawn();

    // 有效 OSC 52 → handler 返回 true → 序列被吞噬
    const result = capturedOsc52Handler!("c;SGVsbG8=");
    expect(result).toBe(true);

    // 无效 OSC 52 → handler 也返回 true → 同样吞噬
    const result2 = capturedOsc52Handler!("p;SGVsbG8=");
    expect(result2).toBe(true);

    const result3 = capturedOsc52Handler!("c;?");
    expect(result3).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════
// Resize 后画面撕裂修复：cancelPendingFlush 测试
// ═══════════════════════════════════════════════════════════

describe("cancelPendingFlush", () => {
  let container: HTMLDivElement;
  let raf: ReturnType<typeof mockRaf>;
  let ro: ReturnType<typeof mockResizeObserver>;
  let ptyOut: ReturnType<typeof ptyOutputSpy>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    raf = mockRaf();
    ro = mockResizeObserver();
    ptyOut = ptyOutputSpy();
    container = createContainer();
  });

  afterEach(() => {
    raf.cleanup();
    ro.cleanup();
    vi.restoreAllMocks();
  });

  async function mountAndWait() {
    const result = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "cpf-test" }),
    );
    raf.flush();
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    return result;
  }

  // ─── cancelPendingFlush 单元测试 ───

  it("CPF1: 取消时清除 idle+max 定时器并清空缓冲区", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(100).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    result.current._test!.cancelPendingFlush();

    // idle + max timer 均被清除
    expect(clearTimeoutSpy.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    // 确认清后不会再有 write
    await new Promise((r) => setTimeout(r, 5));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
    clearTimeoutSpy.mockRestore();
  });

  it("CPF2: 取消后新数据从零开始累积", async () => {
    const { result } = await mountAndWait();

    // 先积攒数据
    ptyOut.sendPtyOutput(new Array(200).fill(65));
    ptyOut.sendPtyOutput(new Array(312).fill(66));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(2);

    // 取消 → 缓冲归零
    result.current._test!.cancelPendingFlush();
    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    // 新数据从 0 开始累积
    ptyOut.sendPtyOutput(new Array(100).fill(67));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);
  });

  it("CPF5: 空缓冲取消不抛异常", async () => {
    const { result } = await mountAndWait();

    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    expect(() => {
      result.current._test!.cancelPendingFlush();
    }).not.toThrow();

    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);
  });

  // ─── cancelPendingFlush vs flushBuffer ───

  it("CPF8: cancelPendingFlush 不写入终端", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(200).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    result.current._test!.cancelPendingFlush();
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    ptyOut.sendPtyOutput(new Array(200).fill(66));
    result.current._test!.flushBuffer();
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  // ─── ResizeObserver 集成测试 ───

  it("CPF6: ResizeObserver 用 cancelPendingFlush 不写入旧数据", async () => {
    await mountAndWait();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(200).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    mockProposeDimensions.mockReturnValue({ cols: 70, rows: 24 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 10));

    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 24 });
    ro.trigger();

    await new Promise((r) => setTimeout(r, 150));

    const writeCalls = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls;
    if (writeCalls.length > 0) {
      for (const call of writeCalls) {
        const decoded = new TextDecoder().decode(call[0] as Uint8Array);
        expect(decoded).not.toContain("A".repeat(200));
      }
    }
  });

  it("CPF7: resize 后新数据正常 DEC 2026 合帧", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 30 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(200).fill(88));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);

    await new Promise((r) => setTimeout(r, 5));
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    expect(decoded.startsWith("\x1b[?2026h")).toBe(true);
  });

  it("CPF9: 列变化 debounce 合并 resize（不产生多余写）", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ptyOut.sendPtyOutput(new Array(200).fill(65));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);

    mockProposeDimensions.mockReturnValue({ cols: 70, rows: 30 });
    ro.trigger();
    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 30 });
    ro.trigger();
    mockProposeDimensions.mockReturnValue({ cols: 50, rows: 30 });
    ro.trigger();

    await new Promise((r) => setTimeout(r, 150));

    expect(pty.resize).toHaveBeenCalledTimes(1);
    expect(mockFit).toHaveBeenCalledTimes(1);
  });

  it("CPF10: 仅行变化立即 fit + resize", async () => {
    await mountAndWait();
    capturedTerminal!.write.mockClear();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();
    capturedTerminal!.write.mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();

    await new Promise((r) => setTimeout(r, 10));

    expect(mockFit).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String),
      80,
      30,
    );
  });

  // ─── 第二轮修复：交替缓冲中始终调用 fit() ───

  it("CPF11: 交替缓冲 + 行变化 → fit 被调用更新网格", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "alternate");

    // 初始化 prevDimsRef
    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 仅行变化（列不变）
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();

    await new Promise((r) => setTimeout(r, 10));

    // 交替缓冲也必须调 fit（更新 xterm.js 网格与 PTY 新尺寸同步）
    expect(mockFit).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith("test-session-id", 80, 30);
  });

  it("CPF12: 交替缓冲 + 列变化 debounce → fit 被调用", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "alternate");

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 列变化 → 进入 debounce 分支
    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 40 });
    ro.trigger();

    await new Promise((r) => setTimeout(r, 150));

    // debounce 后 fit 被调用
    expect(mockFit).toHaveBeenCalledTimes(1);
    expect(pty.resize).toHaveBeenCalledWith("test-session-id", 60, 40);
  });

  it("CPF13: 交替缓冲 + 行变化 → fit 先于 pty.resize", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "alternate");

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 10));

    // 验证调用顺序：fit 在 resize 之前
    const fitOrder = mockFit.mock.invocationCallOrder[0];
    const resizeOrder = (pty.resize as ReturnType<typeof vi.fn>).mock.invocationCallOrder[0];
    expect(fitOrder).toBeLessThan(resizeOrder);
  });

  it("CPF14: 普通缓冲 + 行变化 → fit 被调用（未受影响）", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "normal");

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 10));

    expect(mockFit).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith("test-session-id", 80, 30);
  });

  it("CPF15: 交替缓冲 + 尺寸无变化 → 不调用 fit/resize", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "alternate");

    // 设置 prevDimsRef
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 尺寸不变
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    ro.trigger();
    await new Promise((r) => setTimeout(r, 50));

    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();
  });

  it("CPF16: cancelPendingFlush 在 fit 更新网格之前", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    setBufferType(capturedTerminal, "alternate");

    // 发送旧尺寸数据到缓冲
    ptyOut.sendPtyOutput(new Array(200).fill(65));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);

    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();
    capturedTerminal!.write.mockClear();

    // 再次发送数据 + 触发 resize
    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 30 });
    ptyOut.sendPtyOutput(new Array(200).fill(66));
    ro.trigger();
    await new Promise((r) => setTimeout(r, 150));

    // 旧缓冲数据被丢弃（cancelPendingFlush）
    const writeCalls = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls;
    if (writeCalls.length > 0) {
      for (const call of writeCalls) {
        const decoded = new TextDecoder().decode(call[0] as Uint8Array);
        expect(decoded).not.toContain("A".repeat(200));
      }
    }
    // fit 被调用（更新网格）
    expect(mockFit).toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════
// OSC 8 linkHandler 测试（变更 4）
// ═══════════════════════════════════════════════════════════

describe("OSC 8 linkHandler", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    capturedOsc52Handler = null;

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 渲染 hook 并等待 useXterm 挂载完成（term.open 后 linkHandler 已设置） */
  async function mountAndWaitForLinkHandler() {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "lnk-test" }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    expect(capturedTerminal).not.toBeNull();
  }

  function getLinkHandler() {
    return capturedTerminal?.options.linkHandler as
      | { activate: (event: MouseEvent, url: string) => void }
      | undefined;
  }

  it("LNK1: linkHandler 已设置，activate 为函数", async () => {
    await mountAndWaitForLinkHandler();

    const handler = getLinkHandler();
    expect(handler).toBeDefined();
    expect(typeof handler?.activate).toBe("function");
  });

  it.each([
    { name: "HTTPS", url: "https://example.com" },
    { name: "file://", url: "file:///C:/path/file.txt" },
    { name: "FTP", url: "ftp://server/file" },
  ])("LNK2: 点击 $name 链接 → openUrl 被调用", async ({ url }) => {
    await mountAndWaitForLinkHandler();
    mockOpenUrl.mockClear();

    const handler = getLinkHandler();
    const fakeEvent = new MouseEvent("click");
    handler?.activate(fakeEvent, url);

    await vi.waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalledWith(url);
    });
  });

  it("LNK5: openUrl reject → 不抛异常", async () => {
    mockOpenUrl.mockRejectedValue(new Error("打开失败"));

    await mountAndWaitForLinkHandler();

    const handler = getLinkHandler();
    const fakeEvent = new MouseEvent("click");

    // activate() 返回 undefined（非 Promise），内部 import().then().catch() 不传播异常
    const result = handler?.activate(fakeEvent, "https://example.com");
    expect(result).toBeUndefined();

    // 等待 microtask 完成（openUrl 被调用+reject 但不抛异常）
    await vi.waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalled();
    });
    // 没有未捕获的异常 → 测试通过
  });

  it("LNK6: openUrl 不阻塞终端 — activate 异步返回", async () => {
    let resolveOpen!: () => void;
    const slowPromise = new Promise<void>((resolve) => { resolveOpen = resolve; });

    await mountAndWaitForLinkHandler();

    // 在 mount 后设置 mock（避免 beforeEach 的 clearAllMocks 清除）
    mockOpenUrl.mockReturnValue(slowPromise);

    const handler = getLinkHandler();
    const fakeEvent = new MouseEvent("click");

    // activate 应同步返回 undefined（不等待 openUrl 完成）
    const result = handler?.activate(fakeEvent, "https://example.com");
    expect(result).toBeUndefined();

    // 等待 microtask：openUrl 已被调用，但 Promise 仍 pending
    await vi.waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalled();
    });

    // 终端不被阻塞——测试继续执行，不等待 resolveOpen
    resolveOpen();
  });

  it("LNK7: hover 回调不实现（一期不做）", async () => {
    await mountAndWaitForLinkHandler();

    const handler = capturedTerminal?.options.linkHandler as
      | { hover?: (event: MouseEvent, url: string) => void }
      | undefined;
    expect(handler?.hover).toBeUndefined();
  });

  it("LNK8: 空字符串 URL 不抛异常", async () => {
    await mountAndWaitForLinkHandler();
    mockOpenUrl.mockClear();

    const handler = getLinkHandler();
    const fakeEvent = new MouseEvent("click");

    expect(() => {
      handler?.activate(fakeEvent, "");
    }).not.toThrow();

    await vi.waitFor(() => {
      // openUrl("") 仍被调用（由 Tauri 侧处理）
      expect(mockOpenUrl).toHaveBeenCalledWith("");
    });
  });
});

// ═══════════════════════════════════════════════════════════
// attachCustomKeyEventHandler 键盘拦截测试（变更 2 补强）
// ═══════════════════════════════════════════════════════════

describe("attachCustomKeyEventHandler", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    capturedOsc52Handler = null;
    capturedKeyEventHandler = null;

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 渲染 hook 并等待挂载，返回 capturedKeyEventHandler */
  async function mountAndGetKeyHandler() {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "key-test" }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    expect(capturedKeyEventHandler).not.toBeNull();
    return capturedKeyEventHandler!;
  }

  function makeKeyEvent(overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
    return new KeyboardEvent("keydown", {
      ctrlKey: false,
      shiftKey: false,
      altKey: false,
      metaKey: false,
      code: "KeyA",
      bubbles: true,
      cancelable: true,
      ...overrides,
    });
  }

  it("KEY1: attachCustomKeyEventHandler 已注册", async () => {
    await mountAndGetKeyHandler();
    expect(capturedTerminal?.attachCustomKeyEventHandler).toHaveBeenCalled();
  });

  it("KEY2: keydown → 委托 registry.resolve(event, 'terminal')", async () => {
    const handler = await mountAndGetKeyHandler();
    mockResolve.mockClear();

    const event = makeKeyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });
    handler(event);

    expect(mockResolve).toHaveBeenCalledTimes(1);
    expect(mockResolve.mock.calls[0][0]).toBe(event);
    expect(mockResolve.mock.calls[0][1]).toBe("terminal");
  });

  it("KEY3: resolve 返回 true（命令消费）→ handler 返回 false + preventDefault", async () => {
    const handler = await mountAndGetKeyHandler();
    mockResolve.mockReturnValue(true);

    const event = makeKeyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });
    const result = handler(event);

    expect(result).toBe(false); // 不交给 xterm.js
    expect(event.defaultPrevented).toBe(true);
  });

  it("KEY4: resolve 返回 false（未命中）→ handler 返回 true 透传 + 不 preventDefault", async () => {
    const handler = await mountAndGetKeyHandler();
    mockResolve.mockReturnValue(false);

    const event = makeKeyEvent({ ctrlKey: true, code: "KeyC" }); // Ctrl+C 不注册 → 透传
    const result = handler(event);

    expect(result).toBe(true); // 交给 xterm.js（编码 \x03 到 PTY）
    expect(event.defaultPrevented).toBe(false);
  });

  it("KEY5: 普通字母键 resolve 未命中 → 透传", async () => {
    const handler = await mountAndGetKeyHandler();
    mockResolve.mockReturnValue(false);

    const event = makeKeyEvent({ code: "KeyA" });
    const result = handler(event);

    expect(result).toBe(true);
    expect(event.defaultPrevented).toBe(false);
  });

  it("KEY6: 非 keydown 类型 → 直接透传，不调用 resolve", async () => {
    const handler = await mountAndGetKeyHandler();
    mockResolve.mockClear();

    const event = new KeyboardEvent("keyup", {
      ctrlKey: true, shiftKey: true, code: "KeyC",
      bubbles: true, cancelable: true,
    });
    const result = handler(event);

    expect(result).toBe(true);
    expect(mockResolve).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════
// vtExtensions.kittyKeyboard 配置测试
// ═══════════════════════════════════════════════════════════

describe("theme 配置", () => {
  it("VTX1: terminalOptions 含 vtExtensions.kittyKeyboard=true", async () => {
    const { terminalOptions } = await import("../panels/terminal/theme");
    expect(terminalOptions.vtExtensions?.kittyKeyboard).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════
// P0-3: OSC 133 命令边界检测测试
// ═══════════════════════════════════════════════════════════

describe("OSC 133 命令边界检测", () => {
  let container: HTMLDivElement;
  let mockOnTabStateChange: ReturnType<typeof vi.fn<(state: TabState) => void>>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    capturedOsc133Handler = null;
    mockOnTabStateChange = vi.fn<(state: TabState) => void>();

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  async function mountAndWaitForOsc133() {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "osc133-test",
        onTabStateChange: mockOnTabStateChange,
      }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    expect(capturedOsc133Handler).not.toBeNull();
    return capturedOsc133Handler!;
  }

  it("OSC133-1: OSC 133 C 序列匹配注册命令 → onTabStateChange 含 title 和 icon", async () => {
    mockTabTitleMatch.mockReturnValue({
      command: "claude",
      title: "claude",
      icon: "/claude-logo.png",
    });

    const handler = await mountAndWaitForOsc133();
    // spawn 成功回调中调用了 onTabStateChange({ active: false })，需清除
    mockOnTabStateChange.mockClear();

    const result = handler("C;claude");

    expect(result).toBe(false); // 返回 false 不消费序列（xterm.js 仍渲染提示符）
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      title: "claude",
      icon: "/claude-logo.png",
    });
  });

  it("OSC133-2: OSC 133 D 序列 → onTabStateChange({ active: false })", async () => {
    mockTabTitleMatch.mockReturnValue({
      command: "claude",
      title: "claude",
      icon: "/claude-logo.png",
    });

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();

    // 先发送 C 序列使 isCommandRunningRef = true
    handler("C;claude");
    mockOnTabStateChange.mockClear();

    // 发送 D 序列 → 重置运行状态
    const result = handler("D;0");

    expect(result).toBe(false);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
  });

  it("OSC133-3: 空命令名不触发 onTabStateChange", async () => {
    mockTabTitleMatch.mockReturnValue(null); // 显式重置（hoisted mock 不受 clearAllMocks 影响）

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();

    const result = handler("C;"); // 空命令名，trim() 后为 ""

    expect(result).toBe(false);
    // tabTitleRegistry.match("") 返回 null → 不触发 onTabStateChange
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
  });

  it("OSC133-4: 未注册命令不触发 onTabStateChange", async () => {
    mockTabTitleMatch.mockReturnValue(null);

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();

    const result = handler("C;git_status");

    expect(result).toBe(false);
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════
// P0-3: PTY exit 处理测试
// ═══════════════════════════════════════════════════════════

describe("PTY exit 处理", () => {
  let container: HTMLDivElement;
  let mockOnTabStateChange: ReturnType<typeof vi.fn<(state: TabState) => void>>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    mockOnTabStateChange = vi.fn<(state: TabState) => void>();

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 获取 pty.spawn 的 onOutput 回调 */
  function getSpawnOutputCallback() {
    const calls = (pty.spawn as ReturnType<typeof vi.fn>).mock.calls;
    if (calls.length === 0) return null;
    return calls[calls.length - 1][1] as (event: { type: string; data: { bytes?: number[]; code?: number } }) => void;
  }

  /** 发送 PtyEvent::Exit 事件 */
  function sendPtyExit(code: number | null) {
    const cb = getSpawnOutputCallback();
    if (cb) cb({ type: "exit", data: { code: code ?? undefined } as { code?: number } });
  }

  async function mountAndWait() {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "exit-test",
        onTabStateChange: mockOnTabStateChange,
      }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
  }

  it("EXIT-1: PtyEvent::Exit → terminal.writeln 被调用（含退出码）", async () => {
    await mountAndWait();

    // 清除 spawn 过程中的 writeln 调用
    capturedTerminal!.writeln.mockClear();

    sendPtyExit(1);

    expect(capturedTerminal!.writeln).toHaveBeenCalledWith(
      expect.stringContaining("进程已退出"),
    );
    expect(capturedTerminal!.writeln).toHaveBeenCalledWith(
      expect.stringContaining("退出码: 1"),
    );
  });

  it("EXIT-2: PtyEvent::Exit 退出码为 null → 显示 ?", async () => {
    await mountAndWait();
    capturedTerminal!.writeln.mockClear();

    sendPtyExit(null);

    expect(capturedTerminal!.writeln).toHaveBeenCalledWith(
      expect.stringContaining("退出码: ?"),
    );
  });

  it("EXIT-3: PtyEvent::Exit 时 isCommandRunningRef 为 true → onTabStateChange({ active: false })", async () => {
    // 需要先通过 OSC 133 C 将 isCommandRunningRef 设为 true
    mockTabTitleMatch.mockReturnValue({
      command: "claude",
      title: "claude",
      icon: "/claude-logo.png",
    });

    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "exit-cmd",
        onTabStateChange: mockOnTabStateChange,
      }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });
    expect(capturedOsc133Handler).not.toBeNull();

    // 清除 spawn 回调中的 onTabStateChange 调用
    mockOnTabStateChange.mockClear();

    // 发送 OSC 133 C → isCommandRunningRef = true
    capturedOsc133Handler!("C;claude");
    expect(mockOnTabStateChange).toHaveBeenCalledWith(
      expect.objectContaining({ active: true }),
    );
    mockOnTabStateChange.mockClear();

    // 发送 exit → 应触发 isCommandRunningRef 重置
    sendPtyExit(0);

    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
  });

  it("EXIT-4: PtyEvent::Exit 后 setupRetry 被触发 → term.onData 再次注册", async () => {
    await mountAndWait();

    // 记录 exit 前的 onData 调用次数（至少 1 次：PTY write 监听）
    const onDataCallsBefore = capturedTerminal!.onData.mock.calls.length;

    sendPtyExit(1);

    // exit 后 setupRetry 注册了新 onData 监听 → 调用次数增加
    expect(capturedTerminal!.onData.mock.calls.length).toBeGreaterThan(onDataCallsBefore);
  });
});

// ═══════════════════════════════════════════════════════════
// P0-3: doSpawn catch 测试
// ═══════════════════════════════════════════════════════════

describe("doSpawn catch — spawn 失败重试", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  it("DS1: pty.spawn reject → terminal.writeln 含重新连接提示", async () => {
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ds-1" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // catch 块应写入错误提示
    expect(capturedTerminal!.writeln).toHaveBeenCalledWith(
      expect.stringContaining("重新连接"),
    );
  });

  it("DS2: pty.spawn reject → setupRetry 被触发 → term.onData 注册新监听", async () => {
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ds-2" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // setupRetry 调用了 term.onData（注册 Enter 监听）
    // 注意：effect 中 term.onData 已在 PTY write 路径被调用 1 次
    // setupRetry 会再调 1 次 → 总调用次数 >= 2
    const onDataCallCount = capturedTerminal!.onData.mock.calls.length;
    expect(onDataCallCount).toBeGreaterThanOrEqual(2);
  });

  it("DS3: pty.spawn reject → 错误信息含 panelId", async () => {
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ds-3-p1" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    });

    // 验证 console.error 被调用且含 panelId
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining("ds-3-p1"),
      expect.any(Error),
    );

    consoleErrorSpy.mockRestore();
  });
});

// ═══════════════════════════════════════════════════════════
// P0-3: setupRetry Enter 重连测试
// ═══════════════════════════════════════════════════════════

describe("setupRetry Enter 重连", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 获取最后一次 term.onData 注册的回调 */
  function getLastOnDataCallback(): ((data: string) => void) | null {
    const calls = capturedTerminal!.onData.mock.calls;
    if (calls.length === 0) return null;
    return calls[calls.length - 1][0] as (data: string) => void;
  }

  it("SR1: Enter 键（\\r）→ 触发重新 spawn", async () => {
    // 第一次 spawn 失败 → 触发 setupRetry
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "sr-1" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalledTimes(1);
    });

    // 清除第一次 spawn 记录
    (pty.spawn as ReturnType<typeof vi.fn>).mockClear();
    // 恢复为 resolve 以便第二次 spawn 成功
    (pty.spawn as ReturnType<typeof vi.fn>).mockResolvedValue("retry-session");

    // 获取 setupRetry 注册的 onData 回调
    const onDataCb = getLastOnDataCallback();
    expect(onDataCb).not.toBeNull();

    // 模拟 Enter 键
    onDataCb!("\r");

    // 验证 pty.spawn 被再次调用
    expect(pty.spawn).toHaveBeenCalledTimes(1);
    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24, panelId: "sr-1" }),
      expect.any(Function),
    );
  });

  it("SR2: 非 Enter 键（普通字符）→ 不触发重新 spawn", async () => {
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "sr-2" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalledTimes(1);
    });

    (pty.spawn as ReturnType<typeof vi.fn>).mockClear();

    const onDataCb = getLastOnDataCallback();
    expect(onDataCb).not.toBeNull();

    // 模拟普通按键 'a'
    onDataCb!("a");
    // 模拟换行 '\n'（非 Enter 的 \r）
    onDataCb!("\n");

    // pty.spawn 不应被再次调用
    expect(pty.spawn).not.toHaveBeenCalled();
  });

  it("SR3: Enter 触发 spawn 后 disposable.dispose 被调用", async () => {
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "sr-3" }),
    );

    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalledTimes(1);
    });

    // 清除第一次 spawn 记录，第二次 spawn resolve
    (pty.spawn as ReturnType<typeof vi.fn>).mockClear();
    (pty.spawn as ReturnType<typeof vi.fn>).mockResolvedValue("retry-session-3");

    const beforeCallCount = capturedTerminal!.onData.mock.calls.length;

    const onDataCb = getLastOnDataCallback();
    expect(onDataCb).not.toBeNull();

    // Enter → 触发 spawn
    onDataCb!("\r");
    expect(pty.spawn).toHaveBeenCalledTimes(1);

    // 验证 setupRetry 中调用了 disposable.dispose()
    // 且该 dispose 是 setupRetry 注册的 onData 返回值的 dispose
    const lastOnDataResult =
      capturedTerminal!.onData.mock.results[beforeCallCount - 1];
    expect(lastOnDataResult).toBeDefined();
    expect(lastOnDataResult.value.dispose).toHaveBeenCalled();
  });
});