// use-xterm-lifecycle.test.ts — useXterm 生命周期测试
//
// 覆盖 PTY spawn/exit、快捷键集成、rAF 轮询、ResizeObserver 尺寸变化、
// 字体大小调节、OSC 52/133 协议处理器、OSC 8 linkHandler、键盘委托、
// doSpawn 失败重试、setupRetry Enter 重连。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useFontSize } from "../stores";

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
  mockRegistryMap,
  mockRegistryRegister,
  mockRegistryGet,
  mockRegistryRemove,
  mockSetAgentSession,
  // Agent 事件测试：捕获 onAgentEvent 回调
  mockOnAgentEvent,
  mockUnsubscribeAgentEvent,
  capturedAgentEventCallbackRef,
  // MC-403: cliProfileRegistry.get 解析 profile（hook 事件按 cliId 取能力）
  mockCliProfileGet,
  // MC-403: profile.hooks.eventToStatus 真实调用断言（入参 mock）
  mockEventToStatus,
} = vi.hoisted(() => {
  const registry = new Map<string, { sessionId: string; agentSession?: { cliId?: string } | null }>();
  const agentEventCallbackRef: { current: ((_p: Record<string, unknown>) => void) | null } = { current: null };
  const mockUnsub = vi.fn();
  return {
    mockPushContext: vi.fn(),
    mockPopContext: vi.fn(),
    mockRegister: vi.fn(() => vi.fn()), // 返回注销函数
    mockUnregisterFn: vi.fn(),
    mockResolve: vi.fn<(e: KeyboardEvent, ctx?: string) => boolean>(() => false), // 委托解析：默认未消费
    mockFit: vi.fn(),
    mockProposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    mockOnFontSizeChange: vi.fn(),
    mockRegistryMap: registry,
    mockRegistryRegister: vi.fn((panelId: string, entry: { sessionId: string; agentSession?: { cliId?: string } | null }) => {
      // 幂等 merge（对齐生产 TerminalRegistry.register：agentSession 缺省时保留旧值，
      // 测试预置的 agentSession 不被注册覆盖——HUK12 反查分支依赖此语义）
      const old = registry.get(panelId);
      if (old && entry.agentSession === undefined) {
        entry = { ...entry, agentSession: old.agentSession };
      }
      registry.set(panelId, entry);
    }),
    mockRegistryGet: vi.fn((panelId: string) => registry.get(panelId)),
    mockRegistryRemove: vi.fn((panelId: string) => {
      registry.delete(panelId);
      return true;
    }),
    mockSetAgentSession: vi.fn(),
    // Agent 事件 mock
    mockOnAgentEvent: vi.fn((callback: (_p: Record<string, unknown>) => void) => {
      agentEventCallbackRef.current = callback;
      return mockUnsub;
    }),
    mockUnsubscribeAgentEvent: mockUnsub,
    capturedAgentEventCallbackRef: agentEventCallbackRef,
    mockCliProfileGet: vi.fn(),
    // 默认 stub 语义对齐 claude eventToStatus（10 事件映射）；单测可按需 mockReturnValue 覆盖
    mockEventToStatus: vi.fn(
      (event: string, notificationType?: string | null): string | null => {
        if (event === "SessionStart") return "attention";
        if (event === "UserPromptSubmit" || event === "PreToolUse" || event === "PostToolUse") return "working";
        if (event === "Stop") return "done";
        if (event === "StopFailure" || event === "PostToolUseFailure") return "error";
        if (event === "Notification") {
          return notificationType === "permission_prompt" ||
            notificationType === "idle_prompt" ||
            notificationType === "agent_needs_input"
            ? "attention"
            : null;
        }
        return null;
      },
    ),
  };
});

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

// useXterm.ts import { onAgentEvent } from "../../ipc/agentHooks"（MC-202，P1-F3-07）——
// 本地覆盖 setup.ts 全局 mock 以捕获回调供测试手动触发
vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: mockOnAgentEvent,
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
    register: mockRegistryRegister,
    get: mockRegistryGet,
    remove: mockRegistryRemove,
    setAgentSession: mockSetAgentSession,
  },
}));

// useCommandDetection.ts import { cliProfileRegistry } from "../../features/cliProfiles"
const { mockMatchByCommand } = vi.hoisted(() => ({
  mockMatchByCommand: vi.fn<
    (cmd: string) => {
      id: string;
      displayName: string;
      commands: string[];
      iconSrc: string;
      tabTitle: string;
      capabilities: Record<string, never>;
    } | null
  >(),
}));
vi.mock("../features/cliProfiles", () => ({
  cliProfileRegistry: {
    matchByCommand: mockMatchByCommand,
    register: vi.fn(),
    get: mockCliProfileGet,
    getAll: vi.fn(),
    _reset: vi.fn(),
  },
}));

// 导入被测模块（mocks 就绪后）
import { useXterm } from "../panels/terminal/useXterm";
import type { TabState } from "../panels/terminal/useCommandDetection";
import { pty } from "../ipc";
import {
  createContainer,
  mockResizeObserver,
} from "./helpers/xterm-test-utils";

/** 构造 matchByCommand 返回的 profile（CodingCliProfile 最小合法形态，跨边界契约） */
function makeCliProfile(id: string, iconSrc: string) {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc,
    tabTitle: id,
    capabilities: {},
  };
}

/** 构造含 hooks 能力的 profile（MC-403：eventToStatus 经 profile.capabilities.hooks 真实调用） */
function makeHooksProfile(id: string) {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {
      hooks: {
        eventToStatus: mockEventToStatus,
        classifyNotification: vi.fn(),
        contextLimit: 200_000,
        restartHint: "hooks 改动需重启会话生效",
        hasConfigEditor: true,
      },
    },
  };
}

// ─── 全局 beforeEach：清空 mock Registry 状态（约束 #8：仅 register 后 get 才返回 entry） ───
beforeEach(() => {
  mockRegistryMap.clear();
});

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
    }, { timeout: 3000 });

    // 验证 spawn 使用了 proposeDimensions 返回的尺寸（非默认 80x24）
    expect(pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 100, rows: 40, panelId: "resize-1" }),
      expect.any(Function),
    );

    // 重置 mocks，隔离 ResizeObserver 触发的调用
    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // TE-11: spawn 后用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });

    // 模拟 ResizeObserver 检测到容器尺寸变化
    ro.trigger();

    // TE-11: 推进假定时器 150ms（等待 100ms debounce + 缓冲）
    vi.advanceTimersByTime(150);

    // 验证链路：fit → proposeDimensions → pty.resize（含 panelId）
    expect(mockFit).toHaveBeenCalled();
    expect(mockProposeDimensions).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String), // sessionId（"test-session-id"）
      expect.any(String), // panelId（"resize-1"）
      100, // cols
      40,  // rows
    );

    vi.useRealTimers();
  });

  it("T7: 终端已卸载后 ResizeObserver 触发 → canFit 返回 false → 不调用 fit/resize", async () => {
    const { unmount } = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-2" }),
    );

    // 等待 spawn 完成
    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 卸载终端 → isDisposedRef.current = true
    unmount();

    // 清除之前 spawn/渲染过程中的 fit 调用
    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // TE-11: 切换到假定时器
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });

    // 触发 ResizeObserver 回调
    ro.trigger();
    vi.advanceTimersByTime(150);

    // canFit 条件 5（isDisposedRef.current === true）把门，fit 不被调用
    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();

    vi.useRealTimers();
  });

  it("T8: 容器尺寸变化为 0 时不触发 fit（canFit 条件 3 把门）", async () => {
    // 设置容器尺寸为 0→0
    Object.defineProperty(container, "offsetWidth", { value: 0, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 0, configurable: true });

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-3" }),
    );

    // TE-11: 切换到假定时器
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });

    // 此时 rAF 轮询因为尺寸为 0 持续，spawn 未发生
    // 触发 ResizeObserver
    ro.trigger();
    vi.advanceTimersByTime(150);

    // canFit 条件 3（offsetWidth===0||offsetHeight===0）把门，fit 不被调用
    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();

    vi.useRealTimers();
  });

  it("T9: ResizeObserver 高频触发 → 100ms debounce 合并为单次 resize", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "resize-4" }),
    );

    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    }, { timeout: 3000 });

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // TE-11: 切换到假定时器
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });

    // 在 50ms 内触发 3 次 resize
    ro.trigger();
    vi.advanceTimersByTime(20);
    ro.trigger();
    vi.advanceTimersByTime(20);
    ro.trigger();

    // TE-11: 推进假定时器 130ms（距离最后一次触发 100ms + 缓冲）
    vi.advanceTimersByTime(130);

    // 验证只执行了一次 fit + resize（debounce 生效）
    expect(mockFit).toHaveBeenCalledTimes(1);
    expect(pty.resize).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
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
    // 重置字体大小 store 防止跨测试状态泄露
    useFontSize.setState({ terminalFontSize: 14 });
  });

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
    }, { timeout: 3000 });

    mockFit.mockClear();
    mockProposeDimensions.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    rerender({ fontSize: 18 });

    await waitFor(() => {
      expect(mockFit).toHaveBeenCalled();
    }, { timeout: 3000 });

    expect(mockProposeDimensions).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String),
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
    }, { timeout: 3000 });

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
    // 设置 store 字体大小为下限 8（useFontSizeWheel 从 store 读取基准值）
    useFontSize.setState({ terminalFontSize: 8 });

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

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("46. 到达上限 32 → 放大不触发 onFontSizeChange", () => {
    // 设置 store 字体大小为上限 32（useFontSizeWheel 从 store 读取基准值）
    useFontSize.setState({ terminalFontSize: 32 });

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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
    expect(capturedOsc133Handler).not.toBeNull();
    return capturedOsc133Handler!;
  }

    it("OSC133-1: OSC 133 C 序列匹配注册命令 → onTabStateChange 含 title 和 attention icon + setAgentSession", async () => {
    mockMatchByCommand.mockReturnValue(makeCliProfile("claude", "/cli-icons/claude.png"));

    const handler = await mountAndWaitForOsc133();
    // spawn 成功回调中调用了 onTabStateChange({ active: false })，需清除
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    const result = handler("C;claude");

    expect(result).toBe(false); // 返回 false 不消费序列（xterm.js 仍渲染提示符）
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      title: "claude",
      icon: "🟡",
      // CLI 品牌 logo：cliProfileRegistry.matchByCommand("claude") 命中默认注册 → claude.png
      logo: "/cli-icons/claude.png",
    });
      // P1-F3-01: OSC 133 C 固定使用 🟡 (attention)，不使用 rule.icon
    // MC-107: OSC 133 C 命中注册命令 → 写入会话状态（cliId 取匹配 profile 的 id）
    expect(mockSetAgentSession).toHaveBeenCalledWith("osc133-test", {
      cliId: "claude",
      matchedCommand: "claude",
    });
  });

  it("OSC133-2: OSC 133 D 序列 → onTabStateChange({ active: false }) + setAgentSession(null)", async () => {
    mockMatchByCommand.mockReturnValue(makeCliProfile("claude", "/cli-icons/claude.png"));

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();

    // 先发送 C 序列使 isCommandRunningRef = true
    handler("C;claude");
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    // 发送 D 序列 → 重置运行状态
    const result = handler("D;0");

    expect(result).toBe(false);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
    // MC-107: OSC 133 D → 清除会话行
    expect(mockSetAgentSession).toHaveBeenCalledWith("osc133-test", null);
  });

  it("OSC133-2b: OSC 133 C 匹配 profile → title/logo 均取自 profile（tabTitle / iconSrc）", async () => {
    mockMatchByCommand.mockReturnValue(makeCliProfile("codex", "/cli-icons/codex.png"));

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    handler("C;codex");

    // profile 原子匹配：命中即 title=tabTitle、logo=iconSrc（不再存在"标题命中但 logo 缺失"组合）
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      title: "codex",
      icon: "🟡",
      logo: "/cli-icons/codex.png",
    });
  });

  it("OSC133-3: 空命令名不触发 onTabStateChange", async () => {
    mockMatchByCommand.mockReturnValue(null); // 显式重置（hoisted mock 不受 clearAllMocks 影响）

    const handler = await mountAndWaitForOsc133();
    mockOnTabStateChange.mockClear();

    const result = handler("C;"); // 空命令名，trim() 后为 ""

    expect(result).toBe(false);
    // cliProfileRegistry.matchByCommand("") 返回 null → 不触发 onTabStateChange（未命中零副作用）
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
  });

  it("OSC133-4: 未注册命令不触发 onTabStateChange", async () => {
    mockMatchByCommand.mockReturnValue(null);

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
    }, { timeout: 3000 });
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
    mockMatchByCommand.mockReturnValue(makeCliProfile("claude", "/cli-icons/claude.png"));

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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

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

// ═══════════════════════════════════════════════════════════
// P1-F3-07: Hooks 事件过滤测试
// ═══════════════════════════════════════════════════════════

describe("Hooks 事件过滤 (panelId + profile 解析 + eventToStatus)", () => {
  let container: HTMLDivElement;
  let mockOnTabStateChange: ReturnType<typeof vi.fn<(state: TabState) => void>>;

  /** 构造最小合法 AgentEventPayload 的模拟对象 */
  function makeHookPayload(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
    return {
      panelId: "hooks-test",
      event: "UserPromptSubmit",
      timestamp: Date.now(),
      sessionId: "s1",
      usageSourcePath: "/t.json",
      cwd: "/proj",
      toolName: null,
      notificationType: null,
      ...overrides,
    };
  }

  beforeEach(() => {
    vi.clearAllMocks();
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    capturedTerminal = null;
    mockOnTabStateChange = vi.fn<(state: TabState) => void>();
    // 缺省：claude cliId → claude hooks profile（eventToStatus 经 mockEventToStatus 真实调用）
    mockCliProfileGet.mockReturnValue(makeHooksProfile("claude"));

    container = document.createElement("div");
    Object.defineProperty(container, "offsetWidth", { value: 800, configurable: true });
    Object.defineProperty(container, "offsetHeight", { value: 600, configurable: true });
  });

  /** 渲染 useXterm 并等待 PTY spawn + onAgentEvent 注册完成 */
  async function mountAndWaitForHooks() {
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "hooks-test",
        onTabStateChange: mockOnTabStateChange,
      }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    }, { timeout: 3000 });
    // onAgentEvent 在 effect 中同步调用，spawn 后应已注册
    expect(mockOnAgentEvent).toHaveBeenCalled();
    expect(capturedAgentEventCallbackRef.current).not.toBeNull();
  }

  it("HUK1: 匹配 panelId + UserPromptSubmit → eventToStatus 真实调用（入参断言）+ setAgentSession 携 usageSourcePath", async () => {
    await mountAndWaitForHooks();
    // 清除 spawn 成功时 resetCommandState 产生的 onTabStateChange 调用
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
    }));

    // MC-403: 四态映射委托 profile.hooks.eventToStatus（入参 = event + notificationType）
    expect(mockEventToStatus).toHaveBeenCalledWith("UserPromptSubmit", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      icon: "⚡",
    });
    // PF2-FE-04: 非 SessionEnd 事件 → setAgentSession 携 sessionId/usageSourcePath/status（问题 2 四态同源）
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: "working",
    });
  });

  it("HUK2: 匹配 panelId + SessionEnd → onTabStateChange({ active: false }) + setAgentSession(null)", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "SessionEnd",
    }));

    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
    // PF2-FE-04: SessionEnd → 清除会话行
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", null);
  });

  it("HUK3: 不匹配 panelId → onTabStateChange + setAgentSession 均不触发", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      panelId: "other-panel-id",
      event: "UserPromptSubmit",
    }));

    expect(mockOnTabStateChange).not.toHaveBeenCalled();
    expect(mockSetAgentSession).not.toHaveBeenCalled();
  });

  it("HUK4: PreToolUse → working → ⚡ + setAgentSession", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "PreToolUse",
    }));

    expect(mockEventToStatus).toHaveBeenCalledWith("PreToolUse", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      icon: "⚡",
    });
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: "working",
    });
  });

  it("HUK5: Stop → done → ✅ + setAgentSession", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "Stop",
    }));

    expect(mockEventToStatus).toHaveBeenCalledWith("Stop", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      icon: "✅",
    });
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: "done",
    });
  });

  it("HUK6: StopFailure → error → ❌ + setAgentSession", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "StopFailure",
    }));

    expect(mockEventToStatus).toHaveBeenCalledWith("StopFailure", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      icon: "❌",
    });
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: "error",
    });
  });

  it("HUK7: SessionStart → attention → 🟡 + setAgentSession", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "SessionStart",
    }));

    expect(mockEventToStatus).toHaveBeenCalledWith("SessionStart", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({
      active: true,
      icon: "🟡",
    });
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: "attention",
    });
  });

  it("HUK9: Notification 非 attention 子类型 → setAgentSession 携 status: undefined（不覆盖旧值）", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "Notification",
      notificationType: "general",
    }));

    // notificationType 透传给 profile.eventToStatus（子类型判定归 profile 实现）
    expect(mockEventToStatus).toHaveBeenCalledWith("Notification", "general");
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: "s1",
      usageSourcePath: "/t.json",
      status: undefined,
    });
    // 页签状态不改变（eventToStatus 返回 null）
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
  });

  it("HUK10: payload sessionId/usageSourcePath 空串 → setAgentSession 携 undefined（空串防御归一）", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
      sessionId: "",
      usageSourcePath: "",
    }));

    // 空串必须归一为 undefined——否则 derive 定位/标题覆盖/usage 拉取全部静默失效
    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", {
      sessionId: undefined,
      usageSourcePath: undefined,
      status: "working",
    });
  });

  it("HUK8: 卸载时 unsubscribe 被调用", async () => {
    const { unmount } = renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId: "hooks-unsub",
        onTabStateChange: mockOnTabStateChange,
      }),
    );
    await vi.waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    }, { timeout: 3000 });

    expect(mockOnAgentEvent).toHaveBeenCalled();
    mockUnsubscribeAgentEvent.mockClear();
    unmount();

    // 验证 unsubscribe 被调用（清理函数在 effect cleanup 中执行）
    expect(mockUnsubscribeAgentEvent).toHaveBeenCalled();
  });

  // ═══════════════════════════════════════════════════════════
  // MC-205 三级解析（payload.cliId → 反查 agentSession.cliId → CLAUDE_CLI_ID 缺省）
  // ═══════════════════════════════════════════════════════════

  it("HUK11: 三级解析分支一——payload.cliId 显式（可选字段注入）→ 按该 cliId 解析 profile", async () => {
    mockCliProfileGet.mockReturnValue(makeHooksProfile("codex"));
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();
    mockCliProfileGet.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
      cliId: "codex",
    }));

    expect(mockCliProfileGet).toHaveBeenCalledWith("codex");
    expect(mockEventToStatus).toHaveBeenCalledWith("UserPromptSubmit", null);
    expect(mockSetAgentSession).toHaveBeenCalled();
  });

  it("HUK12: 三级解析分支二——payload.cliId 缺省 → 反查注册表 agentSession.cliId", async () => {
    mockCliProfileGet.mockReturnValue(makeHooksProfile("codex"));
    // 反查键：TerminalRegistry.get(panelId)?.agentSession?.cliId（OSC 133 C 已写入）
    mockRegistryMap.set("hooks-test", {
      sessionId: "s1",
      agentSession: { cliId: "codex" },
    });
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();
    mockCliProfileGet.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
      // 不传 cliId——走反查分支
    }));

    expect(mockCliProfileGet).toHaveBeenCalledWith("codex");
    expect(mockSetAgentSession).toHaveBeenCalled();
  });

  it("HUK13: 三级解析分支三——payload.cliId 缺省且无反查值 → 缺省 CLAUDE_CLI_ID", async () => {
    mockCliProfileGet.mockReturnValue(makeHooksProfile("claude"));
    // 注册表无 entry / entry 无 agentSession——反查为空
    mockRegistryMap.delete("hooks-test");
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();
    mockCliProfileGet.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
    }));

    expect(mockCliProfileGet).toHaveBeenCalledWith("claude");
    expect(mockSetAgentSession).toHaveBeenCalled();
  });

  it("HUK14: 反查 agentSession 存在但 cliId 未设置 → 缺省 CLAUDE_CLI_ID", async () => {
    mockCliProfileGet.mockReturnValue(makeHooksProfile("claude"));
    mockRegistryMap.set("hooks-test", {
      sessionId: "s1",
      agentSession: null, // 已退出残留（OSC 133 D 清空）——cliId 反查为空
    });
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();
    mockCliProfileGet.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
    }));

    expect(mockCliProfileGet).toHaveBeenCalledWith("claude");
  });

  it("HUK14b: 空串/仅空白 cliId 同等回退缺省（ZQ-2——不按空串解析 profile）", async () => {
    mockCliProfileGet.mockReturnValue(makeHooksProfile("claude"));
    mockRegistryMap.delete("hooks-test");
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();
    mockCliProfileGet.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "UserPromptSubmit",
      cliId: "   ", // 仅空白——trim 后为空，必须回退缺省
    }));

    // 空串 cliId 不短路（原 ?? 链会以空串查 profile → 未注册跳过）；
    // resolvePayloadCliId（契约 4）trim 后回退缺省 claude
    expect(mockCliProfileGet).toHaveBeenCalledWith("claude");
    expect(mockSetAgentSession).toHaveBeenCalled();
  });

  // ═══════════════════════════════════════════════════════════
  // MC-206/403: 未知 cliId / 无 hooks 能力 → console.warn + 跳过
  // ═══════════════════════════════════════════════════════════

  it("HUK15: 未知 cliId（未注册）→ console.warn + 跳过（不建行/不置图标/不通知），不抛异常", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    mockCliProfileGet.mockReturnValue(undefined); // 未注册
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    expect(() => {
      capturedAgentEventCallbackRef.current!(makeHookPayload({
        event: "UserPromptSubmit",
        cliId: "unknown-cli",
      }));
    }).not.toThrow();

    expect(mockCliProfileGet).toHaveBeenCalledWith("unknown-cli");
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining("unknown-cli"),
    );
    expect(mockSetAgentSession).not.toHaveBeenCalled();
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("HUK16: 无 hooks 能力 profile → console.warn + 跳过", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // 身份域 profile：capabilities 为空（无 hooks 能力）
    mockCliProfileGet.mockReturnValue(makeCliProfile("codex", "/cli-icons/codex.png"));
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    expect(() => {
      capturedAgentEventCallbackRef.current!(makeHookPayload({
        event: "UserPromptSubmit",
        cliId: "codex",
      }));
    }).not.toThrow();

    expect(mockCliProfileGet).toHaveBeenCalledWith("codex");
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining("codex"),
    );
    expect(mockEventToStatus).not.toHaveBeenCalled();
    expect(mockSetAgentSession).not.toHaveBeenCalled();
    expect(mockOnTabStateChange).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("HUK17: Exit 事件 → setAgentSession(null) 清会话 + 清图标（ZQ-6——双事件判定对齐 SessionEnd）", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    // ZQ-6: Exit 事件清图标条件扩为 SessionEnd ∨ Exit——与删 agentSession
    // 的双事件判定对齐（原仅 SessionEnd 清图标，Exit 事件漏清页签图标）
    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "Exit",
    }));

    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
  });

  it("HUK18: SessionEnd 清图标语义保留——eventToStatus 返回 null 时仍 onTabStateChange({ active: false })", async () => {
    await mountAndWaitForHooks();
    mockOnTabStateChange.mockClear();
    mockSetAgentSession.mockClear();

    capturedAgentEventCallbackRef.current!(makeHookPayload({
      event: "SessionEnd",
    }));

    expect(mockSetAgentSession).toHaveBeenCalledWith("hooks-test", null);
    expect(mockOnTabStateChange).toHaveBeenCalledWith({ active: false });
  });
});
