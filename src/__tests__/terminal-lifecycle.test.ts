// terminal-lifecycle.test.ts — useXterm H6 集成测试
//
// 用最小 TestHarness 组件包裹 useXterm，通过 render + unmount + store 控制
// isLayoutSwitching 标志，验证页面切换时 Terminal 的缓存/复用/销毁三条路径。
//
// H6 路径 1：页面切换 → isLayoutSwitching=true → 卸载时缓存到 Registry
// H6 路径 2：用户关闭  → isLayoutSwitching=false → 卸载时 dispose + kill
// H6 路径 3（删除页面）：同路径 2

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render } from "@testing-library/react";
import { useLayout } from "../stores/layout";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
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
    kill: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
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

describe("H6 页面切换——Terminal 保留（路径 1）", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    useLayout.setState({ activePageId: null, isLayoutSwitching: false });
    TerminalRegistry._clear();
    mocks.resetAll();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("1. isLayoutSwitching=true 时卸载 → Terminal 缓存到 Registry，不 dispose 不 kill", async () => {
    useLayout.getState().setLayoutSwitching(true);
    const container = containerStub();

    const { unmount } = render(
      React.createElement(TestHarness, { container, panelId: "h6-p1" }),
    );

    // 推进 rAF 轮询 + PTY spawn 微任务
    await vi.runAllTimersAsync();

    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    // 卸载 → 触发 useEffect cleanup
    unmount();

    expect(TerminalRegistry.has("h6-p1")).toBe(true);
    expect(mocks.terminal.dispose).not.toHaveBeenCalled();
    expect(mocks.pty.kill).not.toHaveBeenCalled();
  });

  it("2. 有缓存时重新挂载 → 复用缓存的 Terminal，不创建新实例", async () => {
    // 预填充 Registry（模拟上次切换时缓存）
    TerminalRegistry.register("h6-p2", {
      term: mocks.terminal,
      sessionId: "sid-cached",
      webglAddon: null,
      fitAddon: mocks.fitAddon,
    });

    const createCountBefore = mocks.terminalCreateCount;

    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "h6-p2" }),
    );

    await vi.runAllTimersAsync();

    // 不应创建新 Terminal
    expect(mocks.terminalCreateCount).toBe(createCountBefore);
    // 缓存的 term.open 应被调用来重新挂载 DOM
    expect(mocks.terminal.open).toHaveBeenCalled();

    unmount();
  });

  it("3. 有缓存时重新挂载 → 不重复注册 onData 监听器", async () => {
    TerminalRegistry.register("h6-p3", {
      term: mocks.terminal,
      sessionId: "sid-cached",
      webglAddon: null,
      fitAddon: mocks.fitAddon,
    });

    // onData 在 Terminal 接口中类型为 IEvent<string, void>，需 cast 访问 spy
    const onDataSpy = mocks.terminal.onData as unknown as ReturnType<typeof vi.fn>;
    const onDataCallsBefore = onDataSpy.mock.calls.length;

    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "h6-p3" }),
    );

    await vi.runAllTimersAsync();

    // onData 不应被重复调用（缓存路径跳过 onData 注册）
    expect(onDataSpy).toHaveBeenCalledTimes(onDataCallsBefore);

    unmount();
  });
});

describe("H6 用户关闭——Terminal 销毁（路径 2）", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    useLayout.setState({ activePageId: null, isLayoutSwitching: false });
    TerminalRegistry._clear();
    mocks.resetAll();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("4. isLayoutSwitching=false 时卸载 → pty.kill + Registry.remove + term.dispose", async () => {
    // 确保 isLayoutSwitching 为 false（默认）
    expect(useLayout.getState().isLayoutSwitching).toBe(false);

    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "close-p1" }),
    );

    await vi.runAllTimersAsync();

    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);

    unmount();

    expect(mocks.pty.kill).toHaveBeenCalledWith("mock-session-001");
    expect(TerminalRegistry.has("close-p1")).toBe(false);
    expect(mocks.terminal.dispose).toHaveBeenCalled();
  });

  it("5. 无缓存时正常挂载 → 创建新 Terminal + spawn PTY", async () => {
    expect(TerminalRegistry.has("fresh-p1")).toBe(false);

    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "fresh-p1" }),
    );

    await vi.runAllTimersAsync();

    expect(mocks.terminalCreateCount).toBe(1);
    expect(mocks.pty.spawn).toHaveBeenCalledTimes(1);
    expect(mocks.pty.spawn).toHaveBeenCalledWith(
      expect.objectContaining({ panelId: "fresh-p1" }),
      expect.any(Function),
    );

    unmount();
  });

  it("6. 卸载后 Registry 中无残留", async () => {
    const { unmount } = render(
      React.createElement(TestHarness, { container: containerStub(), panelId: "no-residue" }),
    );

    await vi.runAllTimersAsync();

    unmount();

    expect(TerminalRegistry.has("no-residue")).toBe(false);
  });
});
