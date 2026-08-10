// L2 终端面板测试——使用真实 useXterm + mock 依赖
// 复用 terminal-lifecycle.test.ts 的 mock 策略，避免空壳测试
import { describe, it, expect, afterEach, vi } from "vitest";

// ─── Hoisted mocks（复用 terminal-lifecycle 模式） ───
const mocks = vi.hoisted(() => {
  // OSC 133 handler 注册捕获（useCommandDetection 经 parser.registerOscHandler(133, cb) 注册）
  const oscHandlers: Record<number, (data: string) => void> = {};
  // agentHooks.onAgentEvent 回调捕获（useXterm 订阅 agent-event）
  let hookEventCb: ((payload: unknown) => void) | null = null;
  // onDidParametersChange 回调捕获（TerminalPanel 订阅参数变化同步 originalTitleRef）
  let paramsCb: ((p: Record<string, unknown>) => void) | null = null;

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
      registerOscHandler: vi.fn((id: number, cb: (data: string) => void) => {
        oscHandlers[id] = cb;
        return { dispose: vi.fn() };
      }),
    },
  };
  const fitAddon = {
    fit: vi.fn(),
    proposeDimensions: vi.fn(() => ({ cols: 80, rows: 24 })),
    dispose: vi.fn(),
  };
  const pty = {
    spawn: vi.fn().mockResolvedValue("mock-session-001"),
    // 必须返回 Promise——useXterm 卸载清理执行 pty.kill(...).catch(...)，undefined 会抛 TypeError
    kill: vi.fn().mockResolvedValue(undefined),
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
    onDidParametersChange: vi.fn((cb: (p: Record<string, unknown>) => void) => {
      paramsCb = cb;
      return { dispose: vi.fn() };
    }),
    close: vi.fn(),
  };
  const hooks = {
    onAgentEvent: vi.fn((cb: (payload: unknown) => void) => {
      hookEventCb = cb;
      return vi.fn();
    }),
    inject: vi.fn(),
    uninstall: vi.fn(),
    getInjectionStatus: vi.fn(),
  };
  return {
    terminal,
    fitAddon,
    pty,
    mockApi,
    hooks,
    getOscHandler: (id: number) => oscHandlers[id] ?? null,
    getHookEventCb: () => hookEventCb,
    getParamsCb: () => paramsCb,
  };
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
  agentHooks: mocks.hooks,
}));

// 文件级覆盖 setup.ts 全局 mock（照 setup.ts:93-94 形态）：捕获 onAgentEvent 回调，
// 否则 getHookEventCb() 恒 null、hookCb(...) 抛 TypeError
vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: mocks.hooks.onAgentEvent,
  inject: mocks.hooks.inject,
  uninstall: mocks.hooks.uninstall,
  getInjectionStatus: mocks.hooks.getInjectionStatus,
}));

import React from "react";
import { render, screen, waitFor, act, cleanup } from "@testing-library/react";
import TerminalPanel from "../panels/terminal/TerminalPanel";
import { cliProfileRegistry } from "../features/cliProfiles";
import { claudeProfile } from "../features/cliProfiles/profiles/claude";
// claude profile 注册（side-effect，等价旧 cliIcons.ts 内嵌注册）
import "../features/cliProfiles/profiles";

afterEach(() => {
  // RTL 无全局 cleanup（vitest.config.ts 未开 globals）——必须显式卸载，否则遮罩 DOM 跨用例累积
  cleanup();
  // 防用例抛错后 fake timers 泄漏到后续用例
  vi.useRealTimers();
  vi.clearAllMocks();
  // 恢复默认 claude profile 注册（_reset 用例污染隔离）
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
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

  it("1.5s 兜底超时后加载遮罩自动隐藏", () => {
    vi.useFakeTimers();
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p4" } }));
    expect(screen.getByText("正在连接...")).toBeTruthy();
    // 1.5s 未到 → 遮罩仍在
    vi.advanceTimersByTime(1499);
    expect(screen.getByText("正在连接...")).toBeTruthy();
    // 到达 LOADING_MASK_TIMEOUT_MS(1500) → 遮罩消失
    // act 包裹：setTimeout 回调中的 setLoading(false) 在 React 18 并发根下默认异步 flush，
    // 不包 act 则同步断言读到的是未更新的 DOM
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByText("正在连接...")).toBeNull();
    vi.useRealTimers();
  });

  it("Windows build 号到达后写入 term.options.windowsPty（F3 动态设置 ConPTY 阈值）", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p5" } }));
    // buildNumber 初始 undefined 不写，异步 resolve 后经独立 effect 写入
    await waitFor(() => {
      expect(mocks.terminal.options.windowsPty).toEqual({
        backend: "conpty",
        buildNumber: 26100,
      });
    }, { timeout: 3000 });
  });

  it("OSC 133 C/D → handleTabStateChange：active=true 更新标题图标，active=false 恢复原标题", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p6" } }));
    // 等待 useCommandDetection 注册 OSC 133 handler
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // OSC 133 C：命令启动 → 标题更新为规则标题 + 🟡 图标
    await act(async () => {
      oscHandler("C;claude");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenCalledWith("claude");
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabIcon: "🟡" }),
    );

    // OSC 133 D：命令退出 → active=false 恢复原标题并清图标（icon + logo 双清）
    await act(async () => {
      oscHandler("D;0");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("terminal-0");
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabIcon: null, tabLogo: null }),
    );
  });

  it("agent-event SessionEnd → handleTabStateChange active=false 恢复原标题", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p7" } }));
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const hookCb = mocks.getHookEventCb()!;

    // SessionStart → attention → 设置 🟡 图标（无 title 不更新标题）
    await act(async () => {
      hookCb({ panelId: "test-p7", event: "SessionStart" });
    });
    expect(mocks.mockApi.setTitle).not.toHaveBeenCalled();
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabIcon: "🟡" }),
    );

    // SessionEnd → active=false → 恢复原标题 + 清图标（icon + logo 双清）
    await act(async () => {
      hookCb({ panelId: "test-p7", event: "SessionEnd" });
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("terminal-0");
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabIcon: null, tabLogo: null }),
    );
  });

  it("params 含 customTitle 挂载 → active=false 恢复自定义名（而非 api.title）", async () => {
    render(React.createElement(TerminalPanel, {
      api: mocks.mockApi,
      params: { panelId: "test-p8", customTitle: "我的终端" },
    }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // OSC 133 C：命令启动 → 标题覆盖为规则标题
    await act(async () => {
      oscHandler("C;claude");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenCalledWith("claude");

    // OSC 133 D：命令退出 → 恢复自定义名（而非挂载时 api.title "terminal-0"）
    await act(async () => {
      oscHandler("D;0");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("我的终端");
  });

  it("onDidParametersChange 收到 customTitle → ref 同步 → active=false 恢复新名", async () => {
    render(React.createElement(TerminalPanel, {
      api: mocks.mockApi,
      params: { panelId: "test-p9" },
    }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    await waitFor(() => expect(mocks.getParamsCb()).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // 模拟重命名动作：updateParameters({ customTitle: "改名" }) 触发订阅回调
    await act(async () => {
      mocks.getParamsCb()!({ panelId: "test-p9", customTitle: "改名" });
    });

    // OSC 133 C → D：命令退出后恢复重命名后的新名
    await act(async () => {
      oscHandler("C;claude");
    });
    await act(async () => {
      oscHandler("D;0");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("改名");
  });

  it("OSC 133 C 命中 claude → updateParameters 携 tabIcon 🟡 + tabLogo claude logo", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo1" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    await act(async () => {
      oscHandler("C;claude");
    });
    // CliIconRegistry.match("claude") 命中默认注册 → tabLogo = claude.png
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabIcon: "🟡", tabLogo: "/cli-icons/claude.png" }),
    );
  });

  it("hook 事件（无 logo 字段）→ tabLogo 保持前值（logoRef 不清）", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo2" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;
    const hookCb = mocks.getHookEventCb()!;

    // 先 OSC 133 C：logo 建立
    await act(async () => {
      oscHandler("C;claude");
    });
    // 再 hook 事件（Stop → ✅）：无 logo 字段 → logoRef 保持前值
    await act(async () => {
      hookCb({ panelId: "test-logo2", event: "Stop" });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabIcon: "✅", tabLogo: "/cli-icons/claude.png" }),
    );
  });

  it("OSC 133 C 未命中 profile（注册表清空）→ 零副作用（标题/图标均不更新）", async () => {
    cliProfileRegistry._reset(); // 清空全部 profile → matchByCommand("claude") 返回 null
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo3" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // 清除 spawn 成功时 resetCommandState 产生的页签更新调用
    mocks.mockApi.setTitle.mockClear();
    mocks.mockApi.updateParameters.mockClear();

    await act(async () => {
      oscHandler("C;claude");
    });
    // 未命中 → 零副作用：不触发任何页签更新（现状 rule == null 分支语义保留）
    expect(mocks.mockApi.setTitle).not.toHaveBeenCalled();
    expect(mocks.mockApi.updateParameters).not.toHaveBeenCalled();
  });

  it("hook 事件路径无 logo 历史 → tabLogo null", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo4" } }));
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const hookCb = mocks.getHookEventCb()!;

    // SessionStart → 🟡：logoRef 初始 null（无 params.tabLogo 残留）→ tabLogo null
    await act(async () => {
      hookCb({ panelId: "test-logo4", event: "SessionStart" });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabIcon: "🟡", tabLogo: null }),
    );
  });
});
