// xterm-test-utils.ts — useXterm 测试共享工厂函数
//
// 消除 use-xterm-*.test.ts 中 5 类重复模式：
//   rAF mock、容器创建、PTY output spy、ResizeObserver mock、setBufferType

import { vi } from "vitest";
import { pty } from "../../ipc";

// ─── 容器创建 ───

/** 创建模拟容器（非零尺寸避免 rAF 轮询超时循环） */
export function createContainer(w = 800, h = 600): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: w, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: h, configurable: true });
  return el;
}

// ─── rAF mock ───

export interface RafMock {
  /** 推进所有排队的 rAF 回调 */
  flush: () => void;
  /** 恢复原始 requestAnimationFrame / cancelAnimationFrame */
  cleanup: () => void;
}

/**
 * 替换 globalThis.requestAnimationFrame / cancelAnimationFrame
 * 为可控回调队列。调用方在 afterEach 中必须调用 cleanup()。
 */
export function mockRaf(): RafMock {
  const callbacks: Array<FrameRequestCallback> = [];
  let idCounter = 0;
  const origRAF = globalThis.requestAnimationFrame;
  const origCAF = globalThis.cancelAnimationFrame;

  globalThis.requestAnimationFrame = vi.fn((cb: FrameRequestCallback) => {
    const id = ++idCounter;
    callbacks.push(cb);
    return id;
  });
  globalThis.cancelAnimationFrame = vi.fn();

  return {
    flush() {
      while (callbacks.length > 0) {
        const cb = callbacks.shift()!;
        cb(performance.now());
      }
    },
    cleanup() {
      globalThis.requestAnimationFrame = origRAF;
      globalThis.cancelAnimationFrame = origCAF;
    },
  };
}

// ─── PTY output spy ───

export interface PtyOutputSpy {
  /** 发送 PTY 输出事件到 handler */
  sendPtyOutput: (bytes: number[]) => void;
}

/**
 * 捕获 pty.spawn mock 的 onOutput 回调，提供 sendPtyOutput。
 * 调用时机：必须在 pty.spawn 被调用后才能使用 sendPtyOutput。
 */
export function ptyOutputSpy(): PtyOutputSpy {
  function getSpawnOutputCallback(): ((event: { type: string; data: { bytes: number[] } }) => void) | null {
    const calls = (pty.spawn as ReturnType<typeof vi.fn>).mock.calls;
    if (calls.length === 0) return null;
    return calls[calls.length - 1][1];
  }

  return {
    sendPtyOutput(bytes: number[]) {
      const cb = getSpawnOutputCallback();
      if (cb) cb({ type: "output", data: { bytes } });
    },
  };
}

// ─── ResizeObserver mock ───

export interface ResizeObserverMock {
  /** 触发 ResizeObserver 回调（模拟容器尺寸变化） */
  trigger: () => void;
  /** 恢复原始 ResizeObserver */
  cleanup: () => void;
}

/**
 * 替换 globalThis.ResizeObserver，返回可手动触发的 trigger。
 * 调用方在 afterEach 中必须调用 cleanup()。
 */
export function mockResizeObserver(): ResizeObserverMock {
  let triggerFn: () => void = () => {};
  const OrigRO = globalThis.ResizeObserver;

  globalThis.ResizeObserver = class {
    constructor(cb: (entries: unknown[], observer: unknown) => void) {
      triggerFn = () => cb([], this as unknown as ResizeObserver);
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;

  return {
    trigger: () => triggerFn(),
    cleanup() {
      globalThis.ResizeObserver = OrigRO;
    },
  };
}

// ─── setBufferType helper ───

/**
 * 在 capturedTerminal 上挂载 buffer.type 属性。
 * used by AB / CPF 交替缓冲测试。
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function setBufferType(terminal: any, type: string) {
  if (terminal) {
    terminal.buffer = { active: { type } };
  }
}

// ─── makeKeyEvent helper ───

/** 构造 keydown KeyboardEvent */
export function makeKeyEvent(overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
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
