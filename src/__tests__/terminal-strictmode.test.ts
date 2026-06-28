// terminal-strictmode.test.ts — useXterm StrictMode 双重挂载防御测试
//
// React StrictMode 在开发模式下会双重挂载组件（mount→unmount→mount），
// useXterm 使用 useRef guard 防止重复创建 Terminal 实例。

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
    element: document.createElement("div"),
    options: {} as Record<string, unknown>,
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
vi.mock("../ipc", () => ({ pty: mocks.pty }));

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
    useLayout.setState({ activePageId: null });
    mocks.resetAll();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("1. 首次挂载正常创建 Terminal + spawn PTY", async () => {
    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "sm-p1" }),
    );
    await vi.runAllTimersAsync();

    expect(mocks.createCount).toBe(1);
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    unmount();
  });

  it("2. 卸载 → dispose Terminal + kill PTY", async () => {
    const container = containerStub();
    const { unmount } = render(
      React.createElement(TestHarness, { container, panelId: "sm-p2" }),
    );
    await vi.runAllTimersAsync();

    unmount();

    expect(mocks.pty.kill).toHaveBeenCalledWith("mock-sid-strictmode");
    expect(mocks.terminal.dispose).toHaveBeenCalled();
  });

  it("3. 无 container 时 effect 应提前返回（不创建任何东西）", () => {
    render(React.createElement(TestHarness, { container: null, panelId: "sm-p3" }));
    expect(mocks.createCount).toBe(0);
    expect(mocks.pty.spawn).not.toHaveBeenCalled();
  });

  it("4. 重复挂载同一 panelId → 每次创建全新 Terminal（无跨实例复用）", async () => {
    const container = containerStub();

    // 第一次挂载卸载
    const { unmount: u1 } = render(
      React.createElement(TestHarness, { container, panelId: "sm-p4" }),
    );
    await vi.runAllTimersAsync();
    u1();

    mocks.resetAll();

    // 第二次挂载 → 全新 Terminal
    const { unmount: u2 } = render(
      React.createElement(TestHarness, { container, panelId: "sm-p4" }),
    );
    await vi.runAllTimersAsync();

    expect(mocks.createCount).toBe(1); // 新 Terminal
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1); // 新 PTY

    u2();
  });
});
