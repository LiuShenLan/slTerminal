// use-xterm-output.test.ts — useXterm 输出合帧测试
//
// 覆盖 PTY 输出处理全链路：DEC 2026 同步更新、直写阈值路由、
// Idle+Max 双定时器合帧、Uint8Array 缓冲、非焦点终端降频、cancelPendingFlush、
// 缓冲上限淘汰 / 退出码透传 / E2E 缓冲截断（TRM-04，usePtyOutput 直接驱动）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";

// ─── Hoisted mocks ───
const {
  mockPushContext,
  mockPopContext,
  mockRegister,
  mockUnregisterFn,
  mockResolve,
  mockFit,
  mockProposeDimensions,
  mockRegistryMap,
  mockRegistryRegister,
  mockRegistryGet,
  mockRegistryRemove,
} = vi.hoisted(() => {
  const registry = new Map<string, { sessionId: string }>();
  return {
    mockPushContext: vi.fn(),
    mockPopContext: vi.fn(),
    mockRegister: vi.fn(() => vi.fn()), // 返回注销函数
    mockUnregisterFn: vi.fn(),
    mockResolve: vi.fn<(e: KeyboardEvent, ctx?: string) => boolean>(() => false), // 委托解析：默认未消费
    mockFit: vi.fn(),
    mockProposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    mockRegistryMap: registry,
    mockRegistryRegister: vi.fn((panelId: string, entry: { sessionId: string }) => {
      registry.set(panelId, entry);
    }),
    mockRegistryGet: vi.fn((panelId: string) => registry.get(panelId)),
    mockRegistryRemove: vi.fn((panelId: string) => {
      registry.delete(panelId);
      return true;
    }),
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
    attachCustomKeyEventHandler = vi.fn();
    options: Record<string, unknown> = {};
    parser = {
      registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })),
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
    register: mockRegistryRegister,
    get: mockRegistryGet,
    remove: mockRegistryRemove,
  },
}));

// 导入被测模块（mocks 就绪后）
import { useXterm } from "../panels/terminal/useXterm";
import { usePtyOutput } from "../panels/terminal/usePtyOutput";
// 生产阈值常量（TQ-A-06：测试魔数改 import 生产常量，杜绝双维护）
import {
  DIRECT_WRITE_THRESHOLD,
  IDLE_FLUSH_MS,
  MAX_PENDING_BYTES,
  E2E_BUFFER_MAX_LINES,
} from "../panels/terminal/usePtyOutput";
import { pty } from "../ipc";
import {
  createContainer,
  mockRaf,
  ptyOutputSpy,
  mockResizeObserver,
  flushMicrotasks,
} from "./helpers/xterm-test-utils";

// ─── 全局 beforeEach：清空 mock Registry 状态（约束 #8：仅 register 后 get 才返回 entry） ───
beforeEach(() => {
  mockRegistryMap.clear();
});

// ─── 测试套件 ───

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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    vi.useRealTimers();
  });

  it("DEC1: >=256 字节合帧后 term.write 以 \\x1b[?2026h 开头", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-1" }),
    );

    // 等待 pollFitAndSpawn 首帧完成 → spawn
    raf.flush();
    // 等待 microtask 清空（TerminalRegistry.register 在 spawn .then() 中执行）
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    // 清空 spawn 过程中的 write 调用
    capturedTerminal!.write.mockClear();

    // 发送 >256 字节 → 走合帧路径
    const testData = new Array(300).fill(65); // 300 个 'A' (0x41)
    ptyOut.sendPtyOutput(testData);

    // idle timer 未触发 → write 不应被调用（数据在 pendingBufferRef）
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // TE-11: 推进假定时器 5ms → idle timer (2ms) 触发 flushBuffer
    vi.advanceTimersByTime(5);

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
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    const testData = new Array(300).fill(66); // 300 个 'B'
    ptyOut.sendPtyOutput(testData);
    // TE-11: 推进假定时器 → idle timer 触发 flush
    vi.advanceTimersByTime(5);

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
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // 连续发送 3 块 >256 字节数据（同一 idle 窗口内累积）
    ptyOut.sendPtyOutput(new Array(300).fill(67));  // 'C'
    ptyOut.sendPtyOutput(new Array(300).fill(68));  // 'D'
    ptyOut.sendPtyOutput(new Array(300).fill(69));  // 'E'

    // idle timer 未触发 → 没有 write
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // TE-11: 推进假定时器 → idle timer 触发 flushBuffer
    vi.advanceTimersByTime(5);

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

  it("DEC4: <=256 字节直写不含 DEC 2026 序列", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "dec-4" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // 发送 <=256 字节 → 走直写路径
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
// Step 1.2: 直写阈值 64→256 测试（FE-18）
// ═══════════════════════════════════════════════════════════

describe("直写阈值 256 字节路由", () => {
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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    vi.useRealTimers();
  });

  it("TH1: 255 字节 PTY 输出走直写路径（term.write 立即调用）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-1" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // DIRECT_WRITE_THRESHOLD - 1 字节 <= 阈值 → 直写
    ptyOut.sendPtyOutput(new Array(DIRECT_WRITE_THRESHOLD - 1).fill(88)); // 'X'

    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("TH2: 256 字节 PTY 输出走直写路径（阈值边界，FE-18）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-2" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // DIRECT_WRITE_THRESHOLD 字节 = 阈值 → 直写（契约：<=256B 直接写）
    ptyOut.sendPtyOutput(new Array(DIRECT_WRITE_THRESHOLD).fill(89)); // 'Y'

    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    // 直写不含 DEC 2026 包裹
    const written = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(written).not.toContain("\x1b[?2026h");
  });

  it("TH3: 257 字节 PTY 输出走合帧路径（>256 走合并，FE-18）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-3" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // DIRECT_WRITE_THRESHOLD + 1 字节 > 阈值 → 合帧
    ptyOut.sendPtyOutput(new Array(DIRECT_WRITE_THRESHOLD + 1).fill(90)); // 'Z'

    // idle timer 未触发 → write 不应被调用
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // TE-11: 推进假定时器 → idle timer 触发 flush
    vi.advanceTimersByTime(5);
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("TH4: 高频小块输出（连续 5 次 50 字节，均 <=256）全部直写", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-4" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // 5 次小块直写
    for (let i = 0; i < 5; i++) {
      ptyOut.sendPtyOutput(new Array(50).fill(65 + i));
    }

    // 全部走直写：每次立即调用 write
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(5);
  });

  it("TH5: 混合输出（先 50 字节直写，再 300 字节合帧）各自正确路由", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "th-5" }),
    );

    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    capturedTerminal!.write.mockClear();

    // 小块直写
    ptyOut.sendPtyOutput(new Array(50).fill(80)); // 'P'
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    // 大块合帧
    ptyOut.sendPtyOutput(new Array(300).fill(81)); // 'Q'
    // 直写调用数不变（大块走合帧，idle timer 未触发）
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);
    // 合帧后 write 被调用第二次
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(2);
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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    vi.useRealTimers();
  });

  it("IT2: 连续高频输出（2ms 内多次）→ 不立即 flush，最后一次后 2ms flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-2" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    // 连续 3 次大块输出
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    ptyOut.sendPtyOutput(new Array(300).fill(66));
    ptyOut.sendPtyOutput(new Array(300).fill(67));

    // idle timer 被每次输出重置 → 不立即 flush
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // TE-11: 推进假定时器 IDLE_FLUSH_MS + 3 ms（> IDLE_FLUSH_MS 且 < MAX_FLUSH_MS）→ 最后一次后 idle timer 触发
    vi.advanceTimersByTime(IDLE_FLUSH_MS + 3);
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  it("IT3: 持续 16ms 不断有数据 → max timer 强制 flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-3" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    // 每隔 3ms 发送数据——idle timer 永远被重置，但 max timer 在 16ms 时强制 flush
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    vi.advanceTimersByTime(3);
    ptyOut.sendPtyOutput(new Array(300).fill(66));
    vi.advanceTimersByTime(3);
    ptyOut.sendPtyOutput(new Array(300).fill(67));
    vi.advanceTimersByTime(3);
    ptyOut.sendPtyOutput(new Array(300).fill(68));
    vi.advanceTimersByTime(3);
    ptyOut.sendPtyOutput(new Array(300).fill(69));
    // 已过 ~12ms，max timer 即将触发——再推进 10ms → 总计 22ms
    vi.advanceTimersByTime(10);

    // max timer (16ms) 已强制 flush 至少一次
    const writeCalls = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls;
    expect(writeCalls.length).toBeGreaterThan(0);
  });

  it("IT4: 混合输出：小数据直写 + 大数据累积后定时器 flush", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-4" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    // 小块直写
    ptyOut.sendPtyOutput(new Array(30).fill(80));
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    // 小块不含 DEC 2026
    const smallWritten = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(smallWritten).not.toContain("\x1b[?2026h");

    // 大块合帧
    ptyOut.sendPtyOutput(new Array(300).fill(81));
    // 累积中
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);
    // 大块 flush 带 DEC 2026
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(2);
  });

  it("IT5: 组件卸载 → cleanup 调 dispose 清除 idle + max timer（FE-18）", async () => {
    const { unmount } = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "it-5" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    // 发送大块输出 → 启动 idle+max timer
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 卸载 → cleanup 调用 dispose → clearTimeout 被调用（双定时器清除）
    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    unmount();
    expect(clearTimeoutSpy).toHaveBeenCalled();
    clearTimeoutSpy.mockRestore();

    // TE-11: 推进假定时器 30ms → timer 已被 dispose 清除，不再触发 write
    vi.advanceTimersByTime(30);
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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    vi.useRealTimers();
  });

  it("UA1: flushBuffer 合并多个 Uint8Array → term.write 收到正确拼接数据", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-1" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    // 发送多块数据，各块内容不同
    const bytes1 = new Array(300).fill(65); // 'A'
    const bytes2 = new Array(300).fill(66); // 'B'
    ptyOut.sendPtyOutput(bytes1);
    ptyOut.sendPtyOutput(bytes2);

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);

    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    // 解码后应包含 'A'*300 + 'B'*300（包裹在 DEC 2026 之间）
    expect(decoded).toContain("A".repeat(300));
    expect(decoded).toContain("B".repeat(300));
    // 'A' 块在 'B' 块之前
    expect(decoded.indexOf("A")).toBeLessThan(decoded.indexOf("B"));
  });

  it("UA2: flushBuffer 带 DEC 2026 包裹（Uint8Array 拼接）", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-2" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(300).fill(88)); // 'X'

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);

    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    expect(decoded.startsWith("\x1b[?2026h")).toBe(true);
    expect(decoded.endsWith("\x1b[?2026l")).toBe(true);
    expect(decoded).toContain("X".repeat(300));
  });

  it("UA4: 单块 Uint8Array flush 数据完整性", async () => {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "ua-4" }),
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(300).fill(90)); // 'Z' * 300

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);

    const decoded = new TextDecoder().decode(
      (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls[0][0] as Uint8Array,
    );
    const content = decoded.slice(8, decoded.length - 8); // 去除 DEC 2026 包裹
    expect(content).toBe("Z".repeat(300));
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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    vi.useRealTimers();
  });

  it("NF1: visible=false → PTY 输出累积但不 flush", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-1", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // 发送大块输出
    ptyOut.sendPtyOutput(new Array(300).fill(65));

    // TE-11: 推进假定时器 10ms → idle timer 不应触发 flush（因为 visible=false）
    vi.advanceTimersByTime(10);
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
  });

  it("NF2: visible 切回 true → 累积数据立即 flush", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-2", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // 发送大块输出（隐藏期间）
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    // TE-11: 推进假定时器 → 不触发flush（visible=false）
    vi.advanceTimersByTime(10);
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 切回可见 → useEffect 检测到 visible 变为 true → flush
    rerender({ visible: true });

    // TE-11: 推进微任务让 effect 执行
    await flushMicrotasks();
    expect(capturedTerminal!.write).toHaveBeenCalled();
  });

  it("NF3: visible=false 期间直写路径被抑制（对齐 TE-16）", async () => {
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-3", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    // 切换到隐藏
    rerender({ visible: false });
    capturedTerminal!.write.mockClear();

    // TE-16: visible=false 时，<=256 字节直写路径也走累积缓冲，不直写终端
    ptyOut.sendPtyOutput(new Array(30).fill(80)); // <=256 字节 → 被 visible 门控抑制
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
  });

  it("NF4: 组件在 visible=false 状态下卸载 → kill PTY", async () => {
    const { rerender, unmount } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "nf-4", visible }),
      { initialProps: { visible: true as boolean | undefined } },
    );
    raf.flush();
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();

    // 切换到隐藏
    rerender({ visible: false });

    // 发送数据
    ptyOut.sendPtyOutput(new Array(300).fill(65));

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
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(300).fill(65));
    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);

    // visible 未指定 → 视为可见 → 正常 flush
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
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
    // TE-11: 使用假定时器替代真实 setTimeout
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    raf.cleanup();
    ro.cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  /** TE-09/TE-11: mount 后等待 microtask 清空确保 TerminalRegistry.register 完成 */
  async function mountAndWait() {
    const result = renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "cpf-test" }),
    );
    raf.flush();
    // 等待 microtask 清空（TerminalRegistry.register 在 spawn .then() 中执行）
    await flushMicrotasks();
    expect(pty.spawn).toHaveBeenCalled();
    return result;
  }

  // ─── cancelPendingFlush 单元测试 ───

  it("CPF1: 取消时清除 idle+max 定时器并清空缓冲区", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(300).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    result.current._test!.cancelPendingFlush();

    // idle + max timer 均被清除
    expect(clearTimeoutSpy.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    // TE-11: 推进假定时器 → 确认清后不会再有 write
    vi.advanceTimersByTime(5);
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
    clearTimeoutSpy.mockRestore();
  });

  it("CPF2: 取消后新数据从零开始累积", async () => {
    const { result } = await mountAndWait();

    // 先积攒数据
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    ptyOut.sendPtyOutput(new Array(400).fill(66));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(2);

    // 取消 → 缓冲归零
    result.current._test!.cancelPendingFlush();
    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    // 新数据从 0 开始累积
    ptyOut.sendPtyOutput(new Array(300).fill(67));
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

    ptyOut.sendPtyOutput(new Array(300).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    result.current._test!.cancelPendingFlush();
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    ptyOut.sendPtyOutput(new Array(300).fill(66));
    result.current._test!.flushBuffer();
    expect(capturedTerminal!.write).toHaveBeenCalledTimes(1);
  });

  // ─── ResizeObserver 集成测试 ───

  it("CPF6: ResizeObserver 用 cancelPendingFlush 不写入旧数据", async () => {
    await mountAndWait();
    capturedTerminal!.write.mockClear();

    // 发送旧数据 → 启动 idle+max 定时器
    ptyOut.sendPtyOutput(new Array(300).fill(65));
    expect(capturedTerminal!.write).not.toHaveBeenCalled();

    // 首次 resize 触发 cancelPendingFlush → 丢弃旧数据 + 定时器
    mockProposeDimensions.mockReturnValue({ cols: 70, rows: 24 });
    ro.trigger();
    vi.advanceTimersByTime(10);

    // 第二次 resize → 再次 cancelPendingFlush + debounce
    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 24 });
    ro.trigger();
    vi.advanceTimersByTime(150);

    // TE-09: resize 完成后发新数据，验证旧数据未混入
    capturedTerminal!.write.mockClear();
    ptyOut.sendPtyOutput(new Array(300).fill(88)); // 'X'
    vi.advanceTimersByTime(5);

    const writeCalls = (capturedTerminal!.write as ReturnType<typeof vi.fn>).mock.calls;
    // TE-09: 先显式断言有写入，再检查内容——消除 if-guard 假阳性
    expect(writeCalls.length).toBeGreaterThan(0);
    for (const call of writeCalls) {
      const decoded = new TextDecoder().decode(call[0] as Uint8Array);
      expect(decoded).not.toContain("A".repeat(300));
    }
  });

  it("CPF7: resize 后新数据正常 DEC 2026 合帧", async () => {
    const { result } = await mountAndWait();
    capturedTerminal!.write.mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 30 });
    ro.trigger();
    vi.advanceTimersByTime(150);

    expect(result.current._test!.getPendingBuffer()).toHaveLength(0);

    capturedTerminal!.write.mockClear();

    ptyOut.sendPtyOutput(new Array(300).fill(88));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);

    // TE-11: 推进假定时器 → idle timer 触发flush
    vi.advanceTimersByTime(5);
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

    ptyOut.sendPtyOutput(new Array(300).fill(65));
    expect(result.current._test!.getPendingBuffer()).toHaveLength(1);

    mockProposeDimensions.mockReturnValue({ cols: 70, rows: 30 });
    ro.trigger();
    mockProposeDimensions.mockReturnValue({ cols: 60, rows: 30 });
    ro.trigger();
    mockProposeDimensions.mockReturnValue({ cols: 50, rows: 30 });
    ro.trigger();

    // TE-11: 推进假定时器 → debounce 只触发一次 resize
    vi.advanceTimersByTime(150);

    expect(pty.resize).toHaveBeenCalledTimes(1);
    expect(mockFit).toHaveBeenCalledTimes(1);
  });

  it("CPF10: 仅行变化立即 fit + resize", async () => {
    await mountAndWait();
    capturedTerminal!.write.mockClear();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    vi.advanceTimersByTime(150);

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();
    capturedTerminal!.write.mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();

    // TE-11: 仅行变化立即执行，推进少量时间
    vi.advanceTimersByTime(10);

    expect(mockFit).toHaveBeenCalled();
    expect(pty.resize).toHaveBeenCalledWith(
      expect.any(String),
      expect.any(String),
      80,
      30,
    );
  });

  // ─── 真实读取路径断言（TRM-02：删除 setBufferType 虚假前提，保留 distinct 断言）───

  it("CPF13: 行变化 → fit 先于 pty.resize（网格先更新再发 SIGWINCH）", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    ro.trigger();
    vi.advanceTimersByTime(150);
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 30 });
    ro.trigger();
    // TE-11: 仅行变化立即执行
    vi.advanceTimersByTime(10);

    // 验证调用顺序：fit 在 resize 之前（resize/fit 链路真实读取路径，无 buffer.type 依赖）
    const fitOrder = mockFit.mock.invocationCallOrder[0];
    const resizeOrder = (pty.resize as ReturnType<typeof vi.fn>).mock.invocationCallOrder[0];
    expect(fitOrder).toBeLessThan(resizeOrder);
  });

  it("CPF15: 尺寸无变化 → 跳过 fit/resize（无变化守卫）", async () => {
    await mountAndWait();
    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 设置 prevDimsRef
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    ro.trigger();
    vi.advanceTimersByTime(150);

    mockFit.mockClear();
    (pty.resize as ReturnType<typeof vi.fn>).mockClear();

    // 尺寸不变
    mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
    ro.trigger();
    // TE-11: 推进假定时器
    vi.advanceTimersByTime(50);

    expect(mockFit).not.toHaveBeenCalled();
    expect(pty.resize).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════
// TRM-04: usePtyOutput 直接驱动——64KB 上限淘汰 / 退出码 / E2E 缓冲截断
// ═══════════════════════════════════════════════════════════

describe("usePtyOutput 缓冲上限与退出码（直接驱动）", () => {
  // usePtyOutput 零运行时依赖（仅类型导入），直接 renderHook 驱动即可，
  // 不经过 useXterm 编排层——专注测试 64KB 淘汰 / 退出码透传 / E2E 缓冲截断逻辑。
  // （MAX_PENDING_BYTES 经 import 引用生产常量，TQ-A-06）

  beforeEach(() => {
    // 假定时器：防止 idle/max 定时器在测试期间真实触发 flush 干扰缓冲断言
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** 渲染 usePtyOutput，返回 result 与 mock terminal（供 writeln/write 断言） */
  function renderPtyOutput(visible = true, e2eBuffer?: { current: string[] }) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const terminalRef: any = {
      current: {
        write: vi.fn(),
        writeln: vi.fn(),
        onData: vi.fn(() => ({ dispose: vi.fn() })),
      },
    };
    const terminal = terminalRef.current;
    const { result } = renderHook(() =>
      usePtyOutput(terminalRef, "trm4", visible, undefined, undefined, e2eBuffer),
    );
    return { result, terminal };
  }

  /** 解码缓冲块内容（UTF-8） */
  function decodeChunk(chunk: Uint8Array): string {
    return new TextDecoder().decode(chunk);
  }

  it("TRM4-1: 缓冲恰好 64KB（2×32KB）→ 不淘汰", () => {
    const { result } = renderPtyOutput();
    const half = new Array(MAX_PENDING_BYTES / 2).fill(65); // 'A' × 32KB

    result.current.handlePtyOutput({ type: "output", data: { bytes: half } });
    result.current.handlePtyOutput({ type: "output", data: { bytes: half } });

    // 32768+32768 = MAX_PENDING_BYTES，未超过上限 → 两块全保留
    expect(result.current.getPendingBuffer()).toHaveLength(2);
  });

  it("TRM4-2: 缓冲超过 64KB（40KB+40KB）→ 丢弃最旧块", () => {
    const { result } = renderPtyOutput();
    const chunkA = new Array(40960).fill(65); // 'A' × 40KB
    const chunkB = new Array(40960).fill(66); // 'B' × 40KB

    result.current.handlePtyOutput({ type: "output", data: { bytes: chunkA } });
    result.current.handlePtyOutput({ type: "output", data: { bytes: chunkB } });

    // 40960+40960 > MAX_PENDING_BYTES → 淘汰最旧 40KB，仅剩新块
    const buffer = result.current.getPendingBuffer();
    expect(buffer).toHaveLength(1);
    expect(decodeChunk(buffer[0])).toBe("B".repeat(40960));
  });

  it("TRM4-3: 多块超限（3×20KB+30KB）→ 循环淘汰直到放得下", () => {
    const { result } = renderPtyOutput();
    const chunk20k = () => new Array(20480).fill(68); // 'D' × 20KB
    const chunkG = new Array(30720).fill(71); // 'G' × 30KB

    result.current.handlePtyOutput({ type: "output", data: { bytes: chunk20k() } });
    result.current.handlePtyOutput({ type: "output", data: { bytes: chunk20k() } });
    result.current.handlePtyOutput({ type: "output", data: { bytes: chunk20k() } });
    // 61440+30720 > MAX_PENDING_BYTES → 先淘汰 2 块旧数据（20480×2），剩余 20480+30720=51200 ≤ MAX_PENDING_BYTES
    result.current.handlePtyOutput({ type: "output", data: { bytes: chunkG } });

    const buffer = result.current.getPendingBuffer();
    expect(buffer).toHaveLength(2);
    expect(decodeChunk(buffer[0])).toBe("D".repeat(20480));
    expect(decodeChunk(buffer[1])).toBe("G".repeat(30720));
  });

  it("TRM4-4: 退出码 0 与非空数字透传到退出提示", () => {
    const { result, terminal } = renderPtyOutput();

    result.current.handlePtyOutput({ type: "exit", data: { code: 0 } });
    expect(terminal.writeln).toHaveBeenCalledWith(expect.stringContaining("退出码: 0"));

    terminal.writeln.mockClear();
    result.current.handlePtyOutput({ type: "exit", data: { code: 7 } });
    expect(terminal.writeln).toHaveBeenCalledWith(expect.stringContaining("退出码: 7"));
  });

  it("TRM4-5: E2E 文本缓冲超 1000 行 → 截断最旧行（保持 1000）", () => {
    const e2eBuffer = { current: [] as string[] };
    const { result } = renderPtyOutput(true, e2eBuffer);

    // 推送 E2E_BUFFER_MAX_LINES + 5 行（每行 1 字节、内容按序变化，便于区分哪行被丢弃）
    for (let i = 1; i <= E2E_BUFFER_MAX_LINES + 5; i++) {
      result.current.handlePtyOutput({
        type: "output",
        data: { bytes: [(i % 26) + 65] },
      });
    }

    // 截断无条件生效（每事件累积后超限即删最旧）——测试对齐当前实现
    expect(e2eBuffer.current.length).toBe(E2E_BUFFER_MAX_LINES);
    // 最旧 5 行被丢弃：buffer[0] 为第 6 行（i=6 → 'G'）
    expect(e2eBuffer.current[0]).toBe("G");
    // 末尾保持最新行（i=E2E_BUFFER_MAX_LINES + 5 → 'R'）
    expect(e2eBuffer.current[E2E_BUFFER_MAX_LINES - 1]).toBe("R");
  });

  it("DSP1: dispose 清除 idle/max 定时器并清空待输出缓冲（FE-18）", () => {
    const { result, terminal } = renderPtyOutput();

    // 发送 >256 字节 → 走合帧路径并启动 idle+max 定时器
    result.current.handlePtyOutput({
      type: "output",
      data: { bytes: new Array(300).fill(65) },
    });
    expect(result.current.getPendingBuffer()).toHaveLength(1);

    // dispose → 缓冲清空
    result.current.dispose();
    expect(result.current.getPendingBuffer()).toHaveLength(0);

    // 推进假定时器 30ms → idle/max 定时器已被清除，不触发 flush
    // （terminalRef.current 非 null，若定时器未清此处会误触发 write——强断言）
    vi.advanceTimersByTime(30);
    expect(terminal.write).not.toHaveBeenCalled();
  });

  it("DSP2: dispose 幂等——空缓冲/重复调用不抛异常，dispose 后新输出可正常累积（FE-18）", () => {
    const { result } = renderPtyOutput();

    // 空缓冲 dispose 不抛
    expect(() => result.current.dispose()).not.toThrow();
    // 重复调用不抛
    expect(() => result.current.dispose()).not.toThrow();

    // dispose 后新输出仍可正常累积（组件存活语义不受影响）
    result.current.handlePtyOutput({
      type: "output",
      data: { bytes: new Array(300).fill(66) },
    });
    expect(result.current.getPendingBuffer()).toHaveLength(1);
  });
});
