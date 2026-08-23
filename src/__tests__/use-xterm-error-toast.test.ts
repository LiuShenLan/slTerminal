// use-xterm-error-toast.test.ts — useXterm 错误可感知化测试（FE-08）
//
// 覆盖：关键路径 toast（spawn 失败、write 连续失败 ≥3 次）、
// 非关键路径 console.error（openUrl）、write 成功清零连续失败计数

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
  mockRegistryMap,
  mockRegistryRegister,
  mockRegistryGet,
  mockRegistryRemove,
  mockSetAgentSession,
  mockOnAgentEvent,
  mockUnsubscribeAgentEvent,
  mockCliProfileGet,
  mockReadHistoryTitle,
  mockWriteText,
  mockOpenUrl,
  // FE-08: 应用内浮层 + 错误消息 mock（契约：toast.show / getErrorMessage）
  mockToastShow,
  mockGetErrorMessage,
} = vi.hoisted(() => {
  const registry = new Map<string, { sessionId: string; agentSession?: { cliId?: string; sessionId?: string } | null }>();
  return {
    mockPushContext: vi.fn(),
    mockPopContext: vi.fn(),
    mockRegister: vi.fn(() => vi.fn()),
    mockUnregisterFn: vi.fn(),
    mockResolve: vi.fn<(e: KeyboardEvent, ctx?: string) => boolean>(() => false),
    mockFit: vi.fn(),
    mockProposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    mockRegistryMap: registry,
    mockRegistryRegister: vi.fn((panelId: string, entry: { sessionId: string; agentSession?: { cliId?: string } | null }) => {
      registry.set(panelId, entry);
    }),
    mockRegistryGet: vi.fn((panelId: string) => registry.get(panelId)),
    mockRegistryRemove: vi.fn((panelId: string) => {
      registry.delete(panelId);
      return true;
    }),
    mockSetAgentSession: vi.fn(),
    mockOnAgentEvent: vi.fn(() => mockUnsubscribeAgentEvent),
    mockUnsubscribeAgentEvent: vi.fn(),
    mockCliProfileGet: vi.fn(),
    mockReadHistoryTitle: vi.fn(() => Promise.resolve({ title: null, titleSource: "none" })),
    mockWriteText: vi.fn(),
    mockOpenUrl: vi.fn(() => Promise.resolve()),
    mockToastShow: vi.fn(),
    // 契约兜底：Error → message，其余 String
    mockGetErrorMessage: vi.fn((err: unknown) =>
      err instanceof Error ? err.message : String(err),
    ),
  };
});

// ─── 捕获 mock Terminal 实例 ───
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
  parser: { registerOscHandler: ReturnType<typeof vi.fn> };
} | null = null;

// ─── 模块级 mock（路径相对被测源文件 useXterm.ts 的 import 解析） ───
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
    parser = { registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })) };

    constructor() {
      const el = document.createElement("div");
      this.element = el;
      capturedTerminal = this as unknown as typeof capturedTerminal;
    }
  }
  return { Terminal: MockTerminal };
});

vi.mock("../features/shortcuts", () => ({
  usePanelFocus: vi.fn(),
  getShortcutRegistry: () => ({
    register: mockRegister,
    unregister: mockUnregisterFn,
    pushContext: mockPushContext,
    popContext: mockPopContext,
    resolve: mockResolve,
    _reset: vi.fn(),
  }),
}));

vi.mock("../ipc", () => ({
  pty: {
    spawn: vi.fn().mockResolvedValue("test-session-id"),
    write: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    kill: vi.fn().mockResolvedValue(undefined),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  },
}));

vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: mockOnAgentEvent,
}));

vi.mock("../ipc/agentHistory", () => ({
  readHistoryTitle: mockReadHistoryTitle,
  scanAgentHistory: vi.fn().mockResolvedValue([]),
  deleteHistorySession: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: mockWriteText,
  readText: vi.fn(() => Promise.resolve("")),
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

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: mockOpenUrl,
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    register: mockRegistryRegister,
    get: mockRegistryGet,
    remove: mockRegistryRemove,
    setAgentSession: mockSetAgentSession,
  },
}));

vi.mock("../features/cliProfiles", () => ({
  cliProfileRegistry: {
    matchByCommand: vi.fn(() => null),
    register: vi.fn(),
    get: mockCliProfileGet,
    getAll: vi.fn(),
    _reset: vi.fn(),
  },
}));

// FE-08: mock 应用内浮层 toast + getErrorMessage（子路径 ../lib/useFontSizeWheel、
// ../lib/e2eEnabled 不受 barrel mock 影响）
// TQ-A-05: importOriginal 保留 barrel 其余真实导出——新增 ../lib 引用即自动获得真实现，不再 undefined
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return {
    ...actual,
    toast: { ...actual.toast, show: mockToastShow },
    getErrorMessage: mockGetErrorMessage,
  };
});

// ─── 导入被测模块 ───
import { useXterm } from "../panels/terminal/useXterm";
import { pty } from "../ipc";
import { flushMicrotasks } from "./helpers/xterm-test-utils";

/** 创建带尺寸的容器（pollFitAndSpawn 首帧即 fit + spawn） */
function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

beforeEach(() => {
  vi.clearAllMocks();
  mockRegistryMap.clear();
  capturedTerminal = null;
  mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
  (pty.spawn as ReturnType<typeof vi.fn>).mockResolvedValue("test-session-id");
  (pty.write as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** 渲染 useXterm 并等待 PTY spawn 完成（register 已调用、session 可查） */
async function renderAndWaitForSpawn() {
  const container = createContainer();
  renderHook(() =>
    useXterm({ container, cols: 80, rows: 24, panelId: "fe-08" }),
  );
  await waitFor(() => {
    expect(pty.spawn).toHaveBeenCalled();
  }, { timeout: 3000 });
  await waitFor(() => {
    expect(mockRegistryRegister).toHaveBeenCalled();
  }, { timeout: 3000 });
  return container;
}

/** 触发一次终端输入（走主 effect 注册的 onData → pty.write） */
function sendInput(data: string): void {
  const cb = capturedTerminal!.onData.mock.calls[0][0];
  cb(data);
}

// =====================================================================
// 关键路径：spawn 失败 → toast
// =====================================================================

describe("spawn 失败 toast（FE-08）", () => {
  it("T1: pty.spawn reject → toast.show('error') 含『终端启动失败』+ 终端内重连提示", async () => {
    (pty.spawn as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("conpty init failed"),
    );

    const container = createContainer();
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "fe-08-t1" }),
    );

    await waitFor(() => {
      expect(pty.spawn).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(mockToastShow).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("终端启动失败"),
      );
    }, { timeout: 3000 });
    // 消息经 getErrorMessage 提取（Error.message）
    expect(mockToastShow).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("conpty init failed"),
    );
    // 终端内重连提示保留
    expect(capturedTerminal!.writeln).toHaveBeenCalledWith(
      expect.stringContaining("重新连接"),
    );
  });
});

// =====================================================================
// 关键路径：write 连续失败 ≥3 次 → toast；成功清零
// =====================================================================

describe("write 连续失败 toast（FE-08）", () => {
  it("T2: write 失败 2 次仅 console.error；第 3 次连续失败 → toast", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    (pty.write as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error("pipe closed"),
    );

    await renderAndWaitForSpawn();

    sendInput("a");
    sendInput("b");
    // 排空微任务：write reject 的 .catch 在微任务队列执行，同步断言前须 flush
    await flushMicrotasks();
    // 前 2 次：console.error，无 toast
    expect(mockToastShow).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledTimes(2);
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining("PTY write 失败"),
      "pipe closed",
    );

    // 第 3 次连续失败 → toast
    sendInput("c");
    await waitFor(() => {
      expect(mockToastShow).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("终端输入发送失败"),
      );
    }, { timeout: 3000 });

    errorSpy.mockRestore();
  });

  it("T3: 失败 2 次后成功 1 次 → 计数清零，再失败不 toast", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    // 序列：失败、失败、成功、失败
    (pty.write as ReturnType<typeof vi.fn>)
      .mockRejectedValueOnce(new Error("e1"))
      .mockRejectedValueOnce(new Error("e2"))
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("e3"));

    await renderAndWaitForSpawn();

    // 预置 mock promise 全部同时 settle：.then(reset) 反应（1 跳）先于 .catch（2 跳）执行，
    // 末尾一次性 flush 会得到 reset→err→err→err 的错序。逐次 flush 模拟真实串行完成：
    // 每次输入的 then/catch 在处理下一次输入前已落定（同真实 pty.write 完成时序）
    sendInput("a");
    await flushMicrotasks(); // catch e1 → 计数 1
    sendInput("b");
    await flushMicrotasks(); // catch e2 → 计数 2
    sendInput("c"); // 成功 → 清零
    await flushMicrotasks();
    sendInput("d"); // 失败 1（计数 1，<3 不 toast）
    await flushMicrotasks();

    expect(mockToastShow).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledTimes(3); // 三次失败各记日志

    errorSpy.mockRestore();
  });
});

// =====================================================================
// 非关键路径：openUrl 失败 → console.error，无 toast
// =====================================================================

describe("openUrl 失败（FE-08）", () => {
  it("T4: OSC 8 链接打开失败 → console.error（非关键路径无 toast）", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockOpenUrl.mockRejectedValueOnce(new Error("no handler"));

    const container = createContainer();
    renderHook(() =>
      useXterm({ container, cols: 80, rows: 24, panelId: "fe-08-t4" }),
    );

    // 等 linkHandler 设置完成
    await waitFor(() => {
      expect(capturedTerminal!.options.linkHandler).toBeTruthy();
    }, { timeout: 3000 });

    const linkHandler = capturedTerminal!.options.linkHandler as {
      activate: (e: unknown, url: string) => void;
    };
    linkHandler.activate(null, "https://example.com");

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith(
        expect.stringContaining("打开链接失败"),
        "no handler",
      );
    }, { timeout: 3000 });
    expect(mockToastShow).not.toHaveBeenCalled();

    errorSpy.mockRestore();
  });
});
