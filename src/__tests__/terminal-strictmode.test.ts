// terminal-strictmode.test.ts — useXterm StrictMode 双重挂载防御测试
//
// React StrictMode 在开发模式下会双重挂载组件（mount→unmount→mount），
// useXterm 使用 smGuardRef 防止同 panelId 重复创建 Terminal 实例。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render } from "@testing-library/react";
import { useLayout } from "../stores/layout";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";

const mocks = vi.hoisted(() => {
  let createCount = 0;

  const terminal = {
    open: vi.fn(),
    dispose: vi.fn(),
    loadAddon: vi.fn(),
    write: vi.fn(),
    writeln: vi.fn(),
    onData: vi.fn(),
    focus: vi.fn(),
    attachCustomKeyEventHandler: vi.fn(),
    element: document.createElement("div"),
    options: {} as Record<string, unknown>,
    parser: {
      registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })),
    },
  } as unknown as Terminal;

  const fitAddon = {
    fit: vi.fn(),
    proposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    dispose: vi.fn(),
  } as unknown as FitAddon;

  const pty = {
    spawn: vi.fn().mockResolvedValue("mock-sid-strictmode"),
    kill: vi.fn().mockResolvedValue(undefined),
    write: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  };

  return {
    terminal, fitAddon, pty,
    get createCount() { return createCount; },
    incCreateCount() { createCount++; },
    resetAll() {
      createCount = 0;
      Object.values(terminal).forEach(v => { if (vi.isMockFunction(v)) v.mockClear(); });
      Object.values(fitAddon).forEach(v => { if (vi.isMockFunction(v)) v.mockClear(); });
      Object.values(pty).forEach(v => { if (vi.isMockFunction(v)) v.mockClear(); });
    },
  };
});

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(function () { mocks.incCreateCount(); return mocks.terminal; }),
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function () { return mocks.fitAddon; }),
}));
vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: vi.fn(function () { return { onContextLoss: vi.fn(), dispose: vi.fn() }; }),
}));

// Mock ipc——使用 mocks.pty 确保与测试断言中的 spy 是同一实例
vi.mock("../ipc", () => ({
  pty: mocks.pty,
}));

import { useXterm } from "../panels/terminal/useXterm";

function containerStub(): HTMLElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, writable: true, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, writable: true, configurable: true });
  return el;
}

function TestHarness({ container, panelId }: { container: HTMLElement | null; panelId: string }) {
  useXterm({ container, cols: 80, rows: 24, panelId });
  return React.createElement("div", { "data-testid": "harness" });
}

describe("useXterm StrictMode 防御", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    // rAF 立即同步执行：避免 StrictMode fake unmount 在 rAF 触发前取消回调
    vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation(
      (cb: FrameRequestCallback) => {
        cb(performance.now());
        return 1;
      },
    );
    vi.spyOn(globalThis, "cancelAnimationFrame").mockImplementation(vi.fn());

    useLayout.setState({ activePageId: null });
    mocks.resetAll();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("1. StrictMode 双重挂载 → 仅创建 1 个 Terminal + spawn 仅 1 次", async () => {
    // React.StrictMode 开发模式下 mount→unmount→mount，
    // smGuardRef 守卫应阻止第二次 mount 时重复初始化
    const { unmount } = render(
      React.createElement(React.StrictMode, null,
        React.createElement(TestHarness, { container: containerStub(), panelId: "sm-p1" }),
      ),
    );
    await vi.runAllTimersAsync();

    // 核心断言：StrictMode 双重挂载后，Terminal 仅创建 1 次
    expect(mocks.createCount).toBe(1);
    // PTY spawn 在 rAF 同步执行后触发
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    unmount();
  });

  it("2. StrictMode 卸载 → dispose 仅调用 1 次（fake unmount 或 final unmount 之一）", async () => {
    // 注意：由于 StrictMode 的 fake unmount 在 PTY spawn 的 await 完成前触发，
    // kill 可能在 fake unmount 时 sessionIdRef 尚未设置（spawn promise 未 resolve）。
    // 此处仅验证 dispose 调用次数——不测试 kill 的精确时序。
    const container = containerStub();
    const { unmount } = render(
      React.createElement(React.StrictMode, null,
        React.createElement(TestHarness, { container, panelId: "sm-p2" }),
      ),
    );
    await vi.runAllTimersAsync();

    // 最终卸载后 dispose 仅调用 1 次（在 fake unmount 中触发）
    unmount();

    expect(mocks.terminal.dispose).toHaveBeenCalledTimes(1);
  });
});
