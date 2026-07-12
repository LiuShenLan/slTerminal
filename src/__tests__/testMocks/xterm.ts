// testMocks/xterm.ts — xterm.js 相关共享 mock 工厂
//
// 消除 ~6 个文件中重复的 Terminal / FitAddon / WebglAddon mock 样板。
// 工厂函数在 vi.mock() 回调中调用（非 vi.hoisted），因此可以用正常 ESM import。
//
// 用法示例：
//
//   import { createTerminalMock, createFitAddonMock, createWebglAddonMock } from "./testMocks/xterm";
//
//   // 基本用法（不需要捕获）：
//   vi.mock("@xterm/xterm", () => ({ Terminal: createTerminalMock().MockTerminal }));
//   vi.mock("@xterm/addon-fit", () => ({ FitAddon: createFitAddonMock().MockFitAddon }));
//   vi.mock("@xterm/addon-webgl", () => ({ WebglAddon: createWebglAddonMock().MockWebglAddon }));
//
//   // 带捕获（useXterm.test.ts 专用）：
//   const termMocks = vi.hoisted(() => createTerminalMock({ captureOsc52: true, captureOsc133: true, captureKeyHandler: true, captureInstance: true }));
//   vi.mock("@xterm/xterm", () => ({ Terminal: termMocks.MockTerminal }));
//   // 之后通过 termMocks.refs.osc52Handler / osc133Handler / keyEventHandler / capturedTerminal 访问捕获实例

import { vi } from "vitest";

// ─── 捕获回调类型 ───

export interface TerminalCaptureRefs {
  /** OSC 52 handler（claude /copy 命令） */
  osc52Handler: ((data: string) => boolean) | null;
  /** OSC 133 handler（命令边界检测：C=开始, D=退出） */
  osc133Handler: ((data: string) => boolean) | null;
  /** attachCustomKeyEventHandler 注册的键盘 handler */
  keyEventHandler: ((event: KeyboardEvent) => boolean) | null;
  /** 上一次构造的 Terminal 实例引用 */
  capturedTerminal: Record<string, unknown> | null;
}

export interface TerminalMockOptions {
  /** 是否捕获 OSC 52 handler（claude /copy） */
  captureOsc52?: boolean;
  /** 是否捕获 OSC 133 handler（命令边界） */
  captureOsc133?: boolean;
  /** 是否捕获 attachCustomKeyEventHandler handler */
  captureKeyHandler?: boolean;
  /** 是否将构造的实例保存到 refs.capturedTerminal */
  captureInstance?: boolean;
  /** 外部提供的捕获引用对象（若不传则内部创建） */
  refs?: TerminalCaptureRefs;
  /** 覆盖 getSelection 返回值 */
  selectionReturn?: string;
}

export interface TerminalMockResult {
  MockTerminal: new () => Record<string, unknown>;
  refs: TerminalCaptureRefs;
}

/** 创建 Terminal mock 类 + 捕获引用 */
export function createTerminalMock(options: TerminalMockOptions = {}): TerminalMockResult {
  const refs: TerminalCaptureRefs = options.refs ?? {
    osc52Handler: null,
    osc133Handler: null,
    keyEventHandler: null,
    capturedTerminal: null,
  };

  class MockTerminal {
    [key: string]: unknown;
    element: HTMLDivElement;
    open = vi.fn();
    loadAddon = vi.fn();
    dispose = vi.fn();
    onData = vi.fn(() => ({ dispose: vi.fn() }));
    write = vi.fn();
    writeln = vi.fn();
    focus = vi.fn();
    getSelection = vi.fn(() => options.selectionReturn ?? "");
    paste = vi.fn();
    attachCustomKeyEventHandler = vi.fn((handler: (event: KeyboardEvent) => boolean) => {
      if (options.captureKeyHandler) refs.keyEventHandler = handler;
    });
    options: Record<string, unknown> = {};
    parser = {
      registerOscHandler: vi.fn((osc: number, handler: (data: string) => boolean) => {
        if (options.captureOsc52 && osc === 52) refs.osc52Handler = handler;
        if (options.captureOsc133 && osc === 133) refs.osc133Handler = handler;
        return { dispose: vi.fn() };
      }),
    };

    constructor() {
      const el = document.createElement("div");
      this.element = el;
      if (options.captureInstance) refs.capturedTerminal = this as unknown as Record<string, unknown>;
    }
  }

  return { MockTerminal, refs };
}

// ─── FitAddon mock ───

export interface FitAddonMockOptions {
  /** proposeDimensions 返回值 */
  cols?: number;
  rows?: number;
}

export interface FitAddonMockResult {
  MockFitAddon: new () => Record<string, unknown>;
  fit: ReturnType<typeof vi.fn>;
  proposeDimensions: ReturnType<typeof vi.fn>;
}

/** 创建 FitAddon mock 类 */
export function createFitAddonMock(options: FitAddonMockOptions = {}): FitAddonMockResult {
  const fit = vi.fn();
  const proposeDimensions = vi.fn(() => ({ cols: options.cols ?? 80, rows: options.rows ?? 24 }));

  class MockFitAddon {
    [key: string]: unknown;
    fit = fit;
    proposeDimensions = proposeDimensions;
    dispose = vi.fn();
  }

  return { MockFitAddon, fit, proposeDimensions };
}

// ─── WebglAddon mock ───

/** 创建 WebglAddon mock 类 */
export function createWebglAddonMock() {
  class MockWebglAddon {
    dispose = vi.fn();
    onContextLoss = vi.fn();
  }
  return { MockWebglAddon };
}
