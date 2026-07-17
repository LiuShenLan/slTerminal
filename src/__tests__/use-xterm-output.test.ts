// use-xterm-output.test.ts — useXterm 输出合帧测试
//
// 覆盖 PTY 输出处理全链路：DEC 2026 同步更新、直写阈值路由、交替缓冲检查、
// Idle+Max 双定时器合帧、Uint8Array 缓冲、非焦点终端降频、cancelPendingFlush。

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
import { pty } from "../ipc";
import {
  createContainer,
  mockRaf,
  ptyOutputSpy,
  mockResizeObserver,
  setBufferType,
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

  it("NF3: visible=false 期间直写路径被抑制（对齐 TE-16）", async () => {
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

    // TE-16: visible=false 时，<64 字节直写路径也走累积缓冲，不直写终端
    ptyOut.sendPtyOutput(new Array(30).fill(80)); // <64 字节 → 被 visible 门控抑制
    expect(capturedTerminal!.write).not.toHaveBeenCalled();
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
