// use-xterm-integration.test.ts — TE-02: useXterm 轻 mock 集成测试
//
// 与现有 use-xterm-*.test.ts（mock 7 个子 hook 测编排）不同：
// 本文件使用真实 Terminal/FitAddon 和真实子 hook，仅 mock IPC 层。
//
// 覆盖：
// - INT-1: rAF 轮询容器宽度为 0 → 超时回退 80×24
// - INT-2: term.onData → pty.write 调用链
// - INT-3: visible 切换 → WebGL addon 释放/重建

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// ═══════════════════════════════════════════════════════════════
// 0. Canvas 2D context mock（xterm.js DOM 渲染器 WidthCache 需要）
//    覆盖 setup.ts 的 getContext → null mock，仅对 "2d" 返回 stub。
// ═══════════════════════════════════════════════════════════════

/** 构造一个最低限度可用的 CanvasRenderingContext2D stub */
function makeMock2DContext(): CanvasRenderingContext2D {
  const state = {
    fillStyle: "#000" as string | CanvasGradient | CanvasPattern,
    strokeStyle: "#000" as string | CanvasGradient | CanvasPattern,
    font: "10px monospace",
    globalAlpha: 1,
    globalCompositeOperation: "source-over" as string,
    lineWidth: 1,
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const noop = (): any => {};
  return {
    ...state,
    canvas: { width: 100, height: 100 } as HTMLCanvasElement,
    measureText: (text: string) =>
      ({
        width: text.length * 8,
        actualBoundingBoxAscent: 8,
        actualBoundingBoxDescent: 2,
        actualBoundingBoxLeft: 0,
        actualBoundingBoxRight: text.length * 8,
        fontBoundingBoxAscent: 8,
        fontBoundingBoxDescent: 2,
        emHeightAscent: 8,
        emHeightDescent: 2,
        hangingBaseline: 8,
        alphabeticBaseline: 0,
        ideographicBaseline: -2,
      }) as TextMetrics,
    // 绘图方法（no-op）
    fillText: noop,
    strokeText: noop,
    fillRect: noop,
    strokeRect: noop,
    clearRect: noop,
    drawImage: noop,
    putImageData: noop,
    getImageData: () =>
      ({ data: new Uint8ClampedArray(0), width: 0, height: 0, colorSpace: "srgb" }) as ImageData,
    createImageData: () =>
      ({ data: new Uint8ClampedArray(0), width: 0, height: 0, colorSpace: "srgb" }) as ImageData,
    // 状态管理（no-op）
    save: noop,
    restore: noop,
    beginPath: noop,
    closePath: noop,
    fill: noop,
    stroke: noop,
    clip: noop,
    rect: noop,
    moveTo: noop,
    lineTo: noop,
    arc: noop,
    arcTo: noop,
    bezierCurveTo: noop,
    quadraticCurveTo: noop,
    ellipse: noop,
    translate: noop,
    scale: noop,
    rotate: noop,
    transform: noop,
    setTransform: noop,
    resetTransform: noop,
    createLinearGradient: () =>
      ({ addColorStop: noop }) as unknown as CanvasGradient,
    createRadialGradient: () =>
      ({ addColorStop: noop }) as unknown as CanvasGradient,
    createPattern: () => null,
    createConicGradient: noop,
    roundRect: noop,
    // 杂项
    isPointInPath: () => false,
    isPointInStroke: () => false,
    getContextAttributes: () =>
      ({ alpha: true, desynchronized: false, colorSpace: "srgb", willReadFrequently: false }),
    getTransform: () =>
      ({ a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 }) as DOMMatrix,
    drawFocusIfNeeded: noop,
    scrollPathIntoView: noop,
    // 文本
    get textBaseline() {
      return "alphabetic";
    },
    set textBaseline(_v: string) {},
    get textAlign() {
      return "start";
    },
    set textAlign(_v: string) {},
    get direction() {
      return "ltr";
    },
    set direction(_v: string) {},
    get imageSmoothingEnabled() {
      return true;
    },
    set imageSmoothingEnabled(_v: boolean) {},
    get imageSmoothingQuality() {
      return "low";
    },
    set imageSmoothingQuality(_v: string) {},
    get lineCap() {
      return "butt";
    },
    set lineCap(_v: string) {},
    get lineJoin() {
      return "miter";
    },
    set lineJoin(_v: string) {},
    get miterLimit() {
      return 10;
    },
    set miterLimit(_v: number) {},
    get lineDashOffset() {
      return 0;
    },
    set lineDashOffset(_v: number) {},
    getLineDash: () => [],
    setLineDash: noop,
    get shadowBlur() {
      return 0;
    },
    set shadowBlur(_v: number) {},
    get shadowColor() {
      return "rgba(0,0,0,0)";
    },
    set shadowColor(_v: string) {},
    get shadowOffsetX() {
      return 0;
    },
    set shadowOffsetX(_v: number) {},
    get shadowOffsetY() {
      return 0;
    },
    set shadowOffsetY(_v: number) {},
    get filter() {
      return "none";
    },
    set filter(_v: string) {},
  } as unknown as CanvasRenderingContext2D;
}

// 覆盖 setup.ts 的 getContext → null mock
vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
  (contextType: string) => {
    if (contextType === "2d") return makeMock2DContext();
    return null; // WebGL 等上下文不可用（同 setup.ts 默认行为）
  },
);

// ═══════════════════════════════════════════════════════════════
// 1. Hoisted 共享状态
// ═══════════════════════════════════════════════════════════════

const {
  mockPtySpawn,
  mockPtyWrite,
  mockPtyResize,
  mockPtyKill,
  mockPtyGetBuildNumber,
  mockOpenUrl,
  mockClipboardWriteText,
  mockClipboardReadText,
  webglConfig,
  addonInstances,
  mockSetupWebglWithRetry,
} = vi.hoisted(() => {
  const _addonInstances: Array<{ dispose: ReturnType<typeof vi.fn> }> = [];
  return {
    mockPtySpawn: vi.fn(),
    mockPtyWrite: vi.fn(),
    mockPtyResize: vi.fn(),
    mockPtyKill: vi.fn(),
    mockPtyGetBuildNumber: vi.fn(),
    mockOpenUrl: vi.fn(),
    mockClipboardWriteText: vi.fn(),
    mockClipboardReadText: vi.fn(),
    // WebGL 受控配置：available=true 时 detectWebgl→true，setupWebglWithRetry 立即成功
    webglConfig: { available: false },
    addonInstances: _addonInstances,
    mockSetupWebglWithRetry: vi.fn(),
  };
});

// ═══════════════════════════════════════════════════════════════
// 2. Module mocks（相对于被测源文件 import 解析路径）
// ═══════════════════════════════════════════════════════════════

// IPC 层 — 唯一 mock 的应用代码层
vi.mock("../ipc", () => ({
  pty: {
    spawn: mockPtySpawn,
    write: mockPtyWrite,
    resize: mockPtyResize,
    kill: mockPtyKill,
    getWindowsBuildNumber: mockPtyGetBuildNumber,
  },
}));

vi.mock("../ipc/shell", () => ({
  openUrl: mockOpenUrl,
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: mockClipboardWriteText,
  readText: mockClipboardReadText,
}));

// E2E helpers — 避免 import Tauri API 导致模块解析失败
vi.mock("../../../e2e-tests/helpers", () => ({
  installTerminalWriteToPty: vi.fn(),
  setTerminalSessionReady: vi.fn(),
  setTerminalSessionError: vi.fn(),
  initTerminalE2e: vi.fn(),
}));

// tabRules — 副作用导入（注册 tab 规则），stub 防止加载图片等资源
vi.mock("../panels/terminal/tabRules", () => ({}));

// WebGL — 可控 mock：默认不可用，WebGL 测试中设为可用
vi.mock("../panels/terminal/webgl", () => ({
  detectWebgl: () => webglConfig.available,
  resetWebglCache: vi.fn(),
  setupWebglWithRetry: mockSetupWebglWithRetry,
}));

// ═══════════════════════════════════════════════════════════════
// 3. 导入被测模块（mocks 全部就绪后）
// ═══════════════════════════════════════════════════════════════

import { useXterm } from "../panels/terminal/useXterm";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";

// ═══════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════

/** 创建指定尺寸的容器 div */
function createContainer(w: number, h: number): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: w, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: h, configurable: true });
  return el;
}

/** 构造 rAF mock：存储回调不自动执行，由 advanceFrames 手动推进 */
function installRafMock() {
  const origRAF = globalThis.requestAnimationFrame;
  const origCAF = globalThis.cancelAnimationFrame;
  let rafCallbacks: Array<FrameRequestCallback> = [];
  let perfNowValue = 0;

  globalThis.requestAnimationFrame = vi.fn((cb: FrameRequestCallback) => {
    rafCallbacks.push(cb);
    return rafCallbacks.length;
  });
  globalThis.cancelAnimationFrame = vi.fn();
  const perfNowSpy = vi.spyOn(performance, "now").mockImplementation(() => perfNowValue);

  return {
    /** 推进 N 帧，每帧经过 elapsedMs */
    advanceFrames(n: number, elapsedMs = 16) {
      for (let i = 0; i < n; i++) {
        perfNowValue += elapsedMs;
        const batch = rafCallbacks;
        rafCallbacks = [];
        for (const cb of batch) {
          cb(perfNowValue);
        }
      }
    },
    /** 恢复原始 API */
    cleanup() {
      globalThis.requestAnimationFrame = origRAF;
      globalThis.cancelAnimationFrame = origCAF;
      perfNowSpy.mockRestore();
    },
  };
}

// ═══════════════════════════════════════════════════════════════
// beforeEach / afterEach 公共初始化
// ═══════════════════════════════════════════════════════════════

beforeEach(() => {
  vi.clearAllMocks();
  // 默认：WebGL 不可用（对齐 jsdom 无 WebGL 上下文）
  webglConfig.available = false;
  addonInstances.length = 0;
  mockSetupWebglWithRetry.mockImplementation((_term, _onSuccess, onFail) => {
    onFail();
    return { cancel: vi.fn() };
  });
  // 默认 pty.spawn resolve
  mockPtySpawn.mockResolvedValue("test-session-id");
  mockPtyWrite.mockResolvedValue(undefined);
  mockPtyResize.mockResolvedValue(undefined);
  mockPtyKill.mockResolvedValue(undefined);
  mockPtyGetBuildNumber.mockResolvedValue(22621);
  mockClipboardWriteText.mockResolvedValue(undefined);
  mockClipboardReadText.mockResolvedValue("");
  mockOpenUrl.mockResolvedValue(undefined);
  // 清空 TerminalRegistry
  TerminalRegistry._reset();
});

// ═══════════════════════════════════════════════════════════════
// INT-1: rAF 轮询容器宽度失败回退 80×24
// ═══════════════════════════════════════════════════════════════

describe("INT-1: rAF 轮询容器宽度失败回退", () => {
  let raf: ReturnType<typeof installRafMock>;

  afterEach(() => {
    raf?.cleanup();
  });

  it("INT-1.1: 容器 offsetWidth=0 → 30 帧后回退 80×24 调用 pty.spawn", () => {
    raf = installRafMock();
    const container = createContainer(0, 0);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "int-1-1" }),
    );

    // 前 29 帧内不应 spawn
    raf.advanceFrames(29, 16);
    expect(mockPtySpawn).not.toHaveBeenCalled();

    // 第 30 帧触发超时回退（帧数条件满足）
    raf.advanceFrames(1, 16);
    expect(mockPtySpawn).toHaveBeenCalledTimes(1);
    expect(mockPtySpawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24, panelId: "int-1-1" }),
      expect.any(Function),
    );
  });

  it("INT-1.2: 超过 500ms 后无论帧数是否达 30 都回退 80×24", () => {
    raf = installRafMock();
    const container = createContainer(0, 0);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "int-1-2" }),
    );

    // 仅 10 帧，但每帧 51ms → 总耗时 510ms > 500ms → 触发时间超时
    raf.advanceFrames(10, 51);

    expect(mockPtySpawn).toHaveBeenCalledWith(
      expect.objectContaining({ cols: 80, rows: 24, panelId: "int-1-2" }),
      expect.any(Function),
    );
  });

  it("INT-1.3: 容器有效尺寸 → 首帧 fit + proposeDimensions → spawn(真实尺寸)", () => {
    raf = installRafMock();
    const container = createContainer(800, 600);

    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "int-1-3" }),
    );

    // 首帧检测到有效尺寸 → fit → proposeDimensions → spawn
    raf.advanceFrames(1, 16);

    expect(mockPtySpawn).toHaveBeenCalledTimes(1);
    const spawnArgs = mockPtySpawn.mock.calls[0][0];
    // 传入的是 proposeDimensions 计算出的真实 cols/rows（非默认 80×24）
    expect(spawnArgs.cols).toBeGreaterThan(0);
    expect(spawnArgs.rows).toBeGreaterThan(0);
    expect(spawnArgs.panelId).toBe("int-1-3");
    // spawn 后不再调度新 rAF 帧
    expect(mockPtySpawn).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════
// INT-2: term.onData → pty.write 调用链
// ═══════════════════════════════════════════════════════════════

describe("INT-2: term.onData → pty.write 调用链", () => {
  let raf: ReturnType<typeof installRafMock>;

  beforeEach(() => {
    raf = installRafMock();
  });

  afterEach(() => {
    raf?.cleanup();
  });

  /** 渲染 useXterm 并等待 pty.spawn 完成，返回 panelId 供 TerminalRegistry 查询 */
  async function mountAndSpawn(
    container: HTMLDivElement,
    panelId = "int-2",
  ): Promise<string> {
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId }),
    );
    raf.advanceFrames(5, 16);
    await waitFor(() => {
      expect(mockPtySpawn).toHaveBeenCalled();
    }, { timeout: 3000 });
    // 等待 spawn promise resolve → TerminalRegistry 注册完成
    await waitFor(() => {
      expect(TerminalRegistry.get(panelId)).toBeDefined();
    }, { timeout: 3000 });
    return panelId;
  }

  it("INT-2.1: 用户键盘输入 → term.input() 触发 onData → pty.write 被调用", async () => {
    const container = createContainer(800, 600);
    const panelId = await mountAndSpawn(container, "int-2-1");

    mockPtyWrite.mockClear();
    const term = TerminalRegistry.get(panelId)!.term;

    // term.input() 将数据当作用户键盘输入处理，触发 onData 事件
    term.input("echo hello\r", true);

    await waitFor(() => {
      expect(mockPtyWrite).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 验证 pty.write 参数：sessionId + Uint8Array（jsdom 跨 realm 边界，改用 ArrayBuffer.isView）
    const callArgs = mockPtyWrite.mock.calls[0];
    expect(callArgs[0]).toBe("test-session-id");
    expect(ArrayBuffer.isView(callArgs[1])).toBe(true);
    const decoded = new TextDecoder("utf-8").decode(callArgs[1] as Uint8Array);
    expect(decoded).toBe("echo hello\r");
  });

  it("INT-2.2: 终端输入包含中文 → pty.write 接收 UTF-8 编码的 Uint8Array", async () => {
    const container = createContainer(800, 600);
    const panelId = await mountAndSpawn(container, "int-2-2");

    mockPtyWrite.mockClear();
    const term = TerminalRegistry.get(panelId)!.term;

    term.input("你好世界", true);

    await waitFor(() => {
      expect(mockPtyWrite).toHaveBeenCalled();
    }, { timeout: 3000 });

    const data = mockPtyWrite.mock.calls[0][1] as Uint8Array;
    const decoded = new TextDecoder("utf-8").decode(data);
    expect(decoded).toBe("你好世界");
  });

  it("INT-2.3: TerminalRegistry 中无 sessionId → onData 不调用 pty.write（防御）", async () => {
    const container = createContainer(800, 600);
    const panelId = await mountAndSpawn(container, "int-2-3");

    mockPtyWrite.mockClear();
    const term = TerminalRegistry.get(panelId)!.term;

    // 手动清空 registry 后输入
    TerminalRegistry._reset();
    term.input("test", true);

    // pty.write 不应被调用（onData handler 中 TerminalRegistry.get 返回 undefined）
    // 使用小延迟确保异步 catch 有机会执行
    await new Promise((r) => setTimeout(r, 10));
    expect(mockPtyWrite).not.toHaveBeenCalled();
  });

  it("INT-2.4: pty.write reject → 不抛异常（错误被 console.error 捕获）", async () => {
    const container = createContainer(800, 600);
    const panelId = await mountAndSpawn(container, "int-2-4");

    mockPtyWrite.mockRejectedValue(new Error("写入失败"));
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    const term = TerminalRegistry.get(panelId)!.term;
    expect(() => term.input("echo test\r", true)).not.toThrow();

    await waitFor(() => {
      expect(mockPtyWrite).toHaveBeenCalled();
    }, { timeout: 3000 });

    // console.error 被调用（onData handler 中的 .catch 路径）
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "PTY write 失败:",
      expect.any(Error),
    );

    consoleErrorSpy.mockRestore();
  });
});

// ═══════════════════════════════════════════════════════════════
// INT-3: visible 切换 → WebGL addon 释放/重建
// ═══════════════════════════════════════════════════════════════

describe("INT-3: visible 切换 → WebGL addon 释放/重建", () => {
  let raf: ReturnType<typeof installRafMock>;

  beforeEach(() => {
    raf = installRafMock();
    // 启用 WebGL
    webglConfig.available = true;
    addonInstances.length = 0;
    mockSetupWebglWithRetry.mockImplementation((_term, onSuccess) => {
      const addon = { dispose: vi.fn() };
      addonInstances.push(addon);
      onSuccess(addon as unknown as Parameters<typeof onSuccess>[0]);
      return { cancel: vi.fn() };
    });
  });

  afterEach(() => {
    raf?.cleanup();
  });

  /** 渲染 useXterm 并等待初始 spawn 完成 */
  async function mountWithWebgl(
    container: HTMLDivElement,
    panelId = "int-3",
    initialVisible = true,
  ) {
    const result = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId, visible }),
      { initialProps: { visible: initialVisible } },
    );
    raf.advanceFrames(5, 16);
    await waitFor(() => {
      expect(mockPtySpawn).toHaveBeenCalled();
    }, { timeout: 3000 });
    return result;
  }

  it("INT-3.1: 初始挂载时 WebGL 可用 → WebglAddon 实例被创建", async () => {
    const container = createContainer(800, 600);
    await mountWithWebgl(container, "int-3-1");

    // 初始挂载时 setupWebglWithRetry 被调用（useTerminalInstance 内）
    expect(mockSetupWebglWithRetry).toHaveBeenCalled();
    expect(addonInstances.length).toBeGreaterThanOrEqual(1);
  });

  it("INT-3.2: visible 从 true 切到 false → WebGL addon dispose 被调用", async () => {
    const container = createContainer(800, 600);
    const { rerender } = await mountWithWebgl(container, "int-3-2", true);

    await waitFor(() => {
      expect(addonInstances.length).toBeGreaterThanOrEqual(1);
    }, { timeout: 3000 });

    const initialAddon = addonInstances[0];
    // visible → false
    rerender({ visible: false });

    await waitFor(() => {
      expect(initialAddon.dispose).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it("INT-3.3: visible 切回 true → tryLoadWebgl 创建新 WebglAddon", async () => {
    const container = createContainer(800, 600);
    const { rerender } = await mountWithWebgl(container, "int-3-3", true);

    await waitFor(() => {
      expect(addonInstances.length).toBeGreaterThanOrEqual(1);
    }, { timeout: 3000 });

    // visible → false（释放 addon）
    rerender({ visible: false });
    await waitFor(() => {
      expect(addonInstances[0].dispose).toHaveBeenCalled();
    }, { timeout: 3000 });

    const countBefore = addonInstances.length;
    const callsBefore = mockSetupWebglWithRetry.mock.calls.length;

    // visible → true（重建 addon）
    rerender({ visible: true });

    await waitFor(() => {
      expect(addonInstances.length).toBeGreaterThan(countBefore);
    }, { timeout: 3000 });
    expect(mockSetupWebglWithRetry.mock.calls.length).toBeGreaterThan(
      callsBefore,
    );
  });

  it("INT-3.4: 初始 visible=false 挂载 → WebGL addon 在 visible effect 中被释放", async () => {
    const container = createContainer(800, 600);

    // useTerminalInstance 挂载时总是尝试加载 WebGL（与 visible 无关），
    // useXterm 的 visible effect（visible=false）检测到 addon 非空后 dispose 它。
    const { rerender } = renderHook(
      ({ visible }) =>
        useXterm({ container, cols: 80, rows: 24, panelId: "int-3-4", visible }),
      { initialProps: { visible: false } },
    );
    raf.advanceFrames(5, 16);
    await waitFor(() => {
      expect(mockPtySpawn).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 初始挂载时 useTerminalInstance 创建了 WebGL → useXterm visible effect dispose 了它
    await waitFor(() => {
      if (addonInstances.length > 0) {
        expect(addonInstances[0].dispose).toHaveBeenCalled();
      }
    }, { timeout: 3000 });

    // visible 切回 true → tryLoadWebgl 创建新实例
    const countBefore = addonInstances.length;
    rerender({ visible: true });

    await waitFor(() => {
      expect(addonInstances.length).toBeGreaterThan(countBefore);
    }, { timeout: 3000 });
  });

  it("INT-3.5: WebGL 不可用时 visible 切换不抛异常", async () => {
    // 关闭 WebGL 能力
    webglConfig.available = false;
    mockSetupWebglWithRetry.mockImplementation((_term, _onSuccess, onFail) => {
      onFail();
      return { cancel: vi.fn() };
    });
    addonInstances.length = 0;

    const container = createContainer(800, 600);
    const { rerender } = await mountWithWebgl(container, "int-3-5", true);

    // visible → false（webglAddonRef.current 为 null，不过 dispose 分支）
    expect(() => rerender({ visible: false })).not.toThrow();

    // visible → true（detectWebgl→false → tryLoadWebgl 直接返回）
    expect(() => rerender({ visible: true })).not.toThrow();

    // 无新 addon 实例
    expect(addonInstances.length).toBe(0);
  });
});
