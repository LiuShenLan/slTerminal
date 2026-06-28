// terminal-lifecycle.test.ts — useXterm 终端生命周期集成测试
//
// 多 Dockview 实例架构：页面切换时不销毁终端（CSS 显隐），仅用户关闭面板时 dispose。
// 此测试验证 mount → spawn + cleanup → dispose 的基本生命周期路径。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render } from "@testing-library/react";
import { useLayout } from "../stores/layout";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";

// ─── Hoisted mock 实例（vi.mock 工厂通过闭包引用） ───
const mocks = vi.hoisted(() => {
  let terminalCreateCount = 0;

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

  const webglAddon = {
    onContextLoss: vi.fn(),
    dispose: vi.fn(),
  } as unknown as WebglAddon;

  const pty = {
    spawn: vi.fn().mockResolvedValue("mock-session-001"),
    kill: vi.fn().mockResolvedValue(undefined),
    write: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
  };

  return {
    terminal,
    fitAddon,
    webglAddon,
    pty,
    get terminalCreateCount() {
      return terminalCreateCount;
    },
    incTerminalCreateCount() {
      terminalCreateCount++;
    },
    /** 重置所有 spy 状态 + 构造计数 */
    resetAll() {
      terminalCreateCount = 0;
      for (const v of Object.values(terminal)) {
        if (vi.isMockFunction(v)) v.mockClear();
      }
      for (const v of Object.values(fitAddon)) {
        if (vi.isMockFunction(v)) v.mockClear();
      }
      for (const v of Object.values(webglAddon)) {
        if (vi.isMockFunction(v)) v.mockClear();
      }
      for (const v of Object.values(pty)) {
        if (vi.isMockFunction(v)) v.mockClear();
      }
    },
  };
});

// ─── Module mocks（hoisted 到文件顶部） ───
vi.mock("@xterm/xterm", () => ({
  // 必须用普通函数（非箭头），否则 new Terminal() 报 "is not a constructor"
  Terminal: vi.fn(function () {
    mocks.incTerminalCreateCount();
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
    return mocks.webglAddon;
  }),
}));

vi.mock("../ipc", () => ({
  pty: mocks.pty,
}));

// ─── 导入被测模块（在 mocks 之后，模块解析时自动使用 mock） ───
import { useXterm } from "../panels/terminal/useXterm";

// ─── 测试辅助 ───

/** 构造有正尺寸的容器 stub（绕过 pollFitAndSpawn 尺寸检查） */
function containerStub(): HTMLElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, writable: true, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, writable: true, configurable: true });
  return el;
}

/** 最小测试组件：仅调用 useXterm，不渲染 UI */
interface TestHarnessProps {
  container: HTMLElement | null;
  panelId: string;
  cwd?: string;
}

function TestHarness({ container, panelId, cwd }: TestHarnessProps) {
  useXterm({ container, cols: 80, rows: 24, panelId, cwd });
  return React.createElement("div", { "data-testid": "harness" });
}

// ─── 测试套件 ───

describe("终端基本生命周期", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    useLayout.setState({ activePageId: null });
    mocks.resetAll();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("1. 挂载 → 创建 Terminal + spawn PTY", async () => {
    const container = containerStub();
    const { unmount } = render(
      React.createElement(TestHarness, { container, panelId: "test-p1" }),
    );

    await vi.runAllTimersAsync();

    expect(mocks.terminalCreateCount).toBe(1);
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);
    expect(mocks.pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ panelId: "test-p1" }),
      expect.any(Function),
    );

    unmount();
  });

  it("2. 卸载 → dispose Terminal + kill PTY", async () => {
    const container = containerStub();
    const { unmount } = render(
      React.createElement(TestHarness, { container, panelId: "test-p2" }),
    );

    await vi.runAllTimersAsync();

    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    unmount();

    expect(mocks.pty.kill).toHaveBeenCalledWith("mock-session-001");
    expect(mocks.terminal.dispose).toHaveBeenCalled();
  });

  it("3. 卸载后不残留状态（多次挂载卸载各自独立）", async () => {
    const container = containerStub();

    // 第一次挂载卸载
    const { unmount: unmount1 } = render(
      React.createElement(TestHarness, { container, panelId: "test-p3" }),
    );
    await vi.runAllTimersAsync();
    unmount1();

    // 第二次挂载（同一 panelId）→ 全新 Terminal + spawn
    mocks.resetAll();
    const { unmount: unmount2 } = render(
      React.createElement(TestHarness, { container, panelId: "test-p3" }),
    );
    await vi.runAllTimersAsync();

    expect(mocks.terminalCreateCount).toBe(1);
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    unmount2();
    expect(mocks.terminal.dispose).toHaveBeenCalled();
  });

  it("4. 无容器时 effect 应提前返回", () => {
    const { unmount } = render(
      React.createElement(TestHarness, { container: null, panelId: "test-p4" }),
    );

    // 无 container → useEffect 提前 return → 不创建任何东西
    expect(mocks.terminalCreateCount).toBe(0);
    expect(mocks.pty.spawn).not.toHaveBeenCalled();

    unmount();
  });
});
