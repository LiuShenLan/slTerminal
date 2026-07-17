// L2 终端面板测试——使用真实 useXterm + mock 依赖
// 复用 terminal-lifecycle.test.ts 的 mock 策略，避免空壳测试
import { describe, it, expect, afterEach, vi } from "vitest";

// ─── Hoisted mocks（复用 terminal-lifecycle 模式） ───
const mocks = vi.hoisted(() => {
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
  };
  const fitAddon = {
    fit: vi.fn(),
    proposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    dispose: vi.fn(),
  };
  const pty = {
    spawn: vi.fn().mockResolvedValue("mock-session-001"),
    kill: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
    getWindowsBuildNumber: vi.fn().mockResolvedValue(26100),
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const mockApi: any = {
    title: "terminal-0",
    setTitle: vi.fn(),
    updateParameters: vi.fn(),
    onDidTitleChange: vi.fn(() => ({ dispose: vi.fn() })),
    onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
    close: vi.fn(),
  };
  return { terminal, fitAddon, pty, mockApi };
});

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(function () {
    return mocks.terminal;
  }),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function () {
    return mocks.fitAddon;
  }),
}));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: vi.fn(function () {
    return { onContextLoss: vi.fn(), dispose: vi.fn() };
  }),
}));

vi.mock("../ipc", () => ({
  pty: mocks.pty,
}));

import React from "react";
import { render, screen, waitFor } from "@testing-library/react";
import TerminalPanel from "../panels/terminal/TerminalPanel";

afterEach(() => {
  vi.clearAllMocks();
});

describe("TerminalPanel", () => {
  it("挂载时显示'正在连接…'加载遮罩", () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p1" } }));
    expect(screen.getByText("正在连接...")).toBeTruthy();
  });

  it("挂载后通过 IPC 获取 Windows build 号（F3 检测）", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p1" } }));
    await waitFor(() => {
      expect(mocks.pty.getWindowsBuildNumber).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it("挂载后 spawn PTY session", async () => {
    vi.useFakeTimers();
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p2" } }));
    await vi.runAllTimersAsync();
    expect(mocks.pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ panelId: "test-p2" }),
      expect.any(Function),
    );
    vi.useRealTimers();
  });

  it("渲染 .xterm 容器供 xterm.js 挂载", async () => {
    vi.useFakeTimers();
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p3" } }));
    await vi.runAllTimersAsync();
    // Terminal.open(container) 被调用——验证 xterm 挂载
    expect(mocks.terminal.open).toHaveBeenCalled();
    vi.useRealTimers();
  });
});
