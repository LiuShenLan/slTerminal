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
  // onDidTitleChange 回调捕获（B12：TerminalPanel 订阅标题变化同步 originalTitleRef）
  let titleCb: ((e: { title: string }) => void) | null = null;

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
    // 联动 onDidTitleChange（照 dockview 生产行为——setTitle 同步触发标题变化事件），
    // B12 的 originalTitleRef 同步订阅依赖此链
    setTitle: vi.fn((t: string) => {
      mockApi.title = t;
      titleCb?.({ title: t });
    }),
    // 联动 onDidParametersChange（照 dockview 生产行为——updateParameters 同步触发参数变化事件），
    // 组件 latestParamsRef 依赖此同步链（参数覆盖回归守卫用例断言两键共存）
    updateParameters: vi.fn((p: Record<string, unknown>) => {
      paramsCb?.(p);
    }),
    onDidTitleChange: vi.fn((cb: (e: { title: string }) => void) => {
      titleCb = cb;
      return { dispose: vi.fn() };
    }),
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
    getTitleCb: () => titleCb,
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
import { useLayout } from "../stores/layout";
// claude profile 注册（side-effect，等价旧 cliIcons.ts 内嵌注册）
import "../features/cliProfiles/profiles";
// 真实 TerminalRegistry：F9 行为修订后 TerminalPanel 经 subscribe/get 订阅会话状态，
// 测试手动 register stub 条目 + setAgentSession 驱动 sessionChange（照生产语义）
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal } from "../panels/terminal/TerminalRegistry";

/** 注册 stub 终端条目（agentSession 缺省 undefined）——useXterm 真实 register
 *  幂等覆盖保留旧值，测试注入的会话不被 spawn 完成后的注册冲掉 */
function registerStub(panelId: string, agentSession?: RegisteredTerminal["agentSession"]): void {
  const entry = {
    term: mocks.terminal,
    sessionId: "stub-session",
    webglAddon: null,
    fitAddon: mocks.fitAddon,
    ...(agentSession !== undefined ? { agentSession } : {}),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any as RegisteredTerminal;
  TerminalRegistry.register(panelId, entry);
}

afterEach(() => {
  // RTL 无全局 cleanup（vitest.config.ts 未开 globals）——必须显式卸载，否则遮罩 DOM 跨用例累积
  cleanup();
  // 防用例抛错后 fake timers 泄漏到后续用例
  vi.useRealTimers();
  vi.clearAllMocks();
  // 恢复默认 claude profile 注册（_reset 用例污染隔离）
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
  // 清空测试手动 register 的 stub 条目与订阅（真实注册表跨用例污染隔离）
  TerminalRegistry._reset();
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

  it("Win10 build 号（19045）钳制至 21376 写入 windowsPty（ADR-0004）", async () => {
    // Once 覆盖默认 26100，仅本用例生效；afterEach 的 clearAllMocks 不清实现
    mocks.pty.getWindowsBuildNumber.mockResolvedValueOnce(19045);
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p5b" } }));
    await waitFor(() => {
      expect(mocks.terminal.options.windowsPty).toEqual({
        backend: "conpty",
        buildNumber: 21376,
      });
    }, { timeout: 3000 });
  });

  it("OSC 133 C/D → handleTabStateChange：active=true 更新标题图标，active=false 恢复原标题", async () => {
    // 面板须已注册（生产 register 由 spawn 完成触发）——C 的 setAgentSession
    // 仅在已注册面板上生效，B12 守卫依赖 agentSession 置位
    registerStub("test-p6");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p6" } }));
    // 等待 useCommandDetection 注册 OSC 133 handler
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // OSC 133 C：命令启动 → 标题更新为规则标题 + attention 状态
    await act(async () => {
      oscHandler("C;claude");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenCalledWith("claude");
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );

    // OSC 133 D：命令退出 → active=false 恢复原标题并单清图标（logo 由
    // sessionChange 驱动清除，不再经此路径双清——F9 行为修订）
    await act(async () => {
      oscHandler("D;0");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("terminal-0");
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabStatus: null }),
    );
  });

  it("B13: SessionStart 补 title / SessionEnd 仅清图标不恢复标题（/resume 序列）", async () => {
    registerStub("test-p7");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-p7" } }));
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const hookCb = mocks.getHookEventCb()!;

    // SessionStart → attention → attention 状态 + 标题重设为 profile.tabTitle（B13：
    // /resume 无 OSC 133 C，标题经 hook 事件补位保持 claude）
    await act(async () => {
      hookCb({ panelId: "test-p7", event: "SessionStart" });
    });
    expect(mocks.mockApi.setTitle).toHaveBeenCalledWith("claude");
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );

    // SessionEnd → restoreTitle=false → 仅清图标，不恢复标题（B13：claude
    // 进程仍在跑，恢复会把标题误回退为 terminal-N——/resume 根因）
    await act(async () => {
      hookCb({ panelId: "test-p7", event: "SessionEnd" });
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("claude");
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabStatus: null }),
    );

    // 真退出信号（OSC 133 D）→ 恢复原标题（B13 共识：恢复只由真退出承担）。
    // 前置：D 分支受 isCommandRunningRef 门控，需先经 OSC 133 C 置位
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    await act(async () => {
      mocks.getOscHandler(133)!("C;claude");
    });
    await act(async () => {
      mocks.getOscHandler(133)!("D;0");
    });
    expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("terminal-0");
  });

  it("B14: 旧恢复格式 panelId + activePageId 匹配 → visible 放行（PTY 输出直写防黑屏）", async () => {
    vi.useFakeTimers();
    // 旧恢复格式：terminal-{pageId}-{Date.now}-{seq}（pageId 本身含数字段——
    // 贪婪正则/切分解析会吞掉 Date.now 段得到错误 pageId，visible 恒 false 黑屏）
    useLayout.setState({ activePageId: "page-restore-x" });
    try {
      render(React.createElement(TerminalPanel, {
        api: mocks.mockApi,
        params: { panelId: "terminal-page-restore-x-1700000000000-1" },
      }));
      await vi.runAllTimersAsync();
      expect(mocks.pty.spawn).toHaveBeenCalled();
      // 提取 pty.spawn 的 onOutput 回调（= handlePtyOutput），注入短输出（<=256 字节直写路径）。
      // 事件形态 = PtyEvent 序列化：{ type: "output", data: { bytes: number[] } }
      const handlePtyOutput = mocks.pty.spawn.mock.calls[0][1] as (
        event: { type: string; data: { bytes: number[] } },
      ) => void;
      mocks.terminal.write.mockClear();
      act(() => {
        handlePtyOutput({ type: "output", data: { bytes: [104, 105] } }); // "hi"
      });
      // 前缀匹配判定 visible=true → 直写终端（旧实现恒 false → 永不 flush 黑屏）
      expect(mocks.terminal.write).toHaveBeenCalled();
    } finally {
      useLayout.setState({ activePageId: null });
      vi.useRealTimers();
    }
  });

  it("B14: activePageId 不匹配 → visible=false 输出仅缓冲不直写", async () => {
    vi.useFakeTimers();
    useLayout.setState({ activePageId: "page-other" });
    try {
      render(React.createElement(TerminalPanel, {
        api: mocks.mockApi,
        params: { panelId: "terminal-page-restore-x-1700000000000-1" },
      }));
      await vi.runAllTimersAsync();
      expect(mocks.pty.spawn).toHaveBeenCalled();
      const handlePtyOutput = mocks.pty.spawn.mock.calls[0][1] as (
        event: { type: string; data: { bytes: number[] } },
      ) => void;
      mocks.terminal.write.mockClear();
      act(() => {
        handlePtyOutput({ type: "output", data: { bytes: [104, 105] } });
      });
      // 非焦点降频：不写终端（切回后由 flushBuffer 回放）
      expect(mocks.terminal.write).not.toHaveBeenCalled();
    } finally {
      useLayout.setState({ activePageId: null });
      vi.useRealTimers();
    }
  });

  it("B12: 布局恢复重算标题 → originalTitleRef 同步 → 真退出恢复重算名（非瞬态 claude）", async () => {
    // 模拟持久化瞬态标题（claude 运行中退出保存的布局 title）
    mocks.mockApi.title = "claude";
    try {
      registerStub("test-b12");
      render(React.createElement(TerminalPanel, {
        api: mocks.mockApi,
        params: { panelId: "test-b12" },
      }));
      await waitFor(() => expect(mocks.getTitleCb()).toBeDefined());
      // 模拟 handleReady → rebuildAndRecomputeTitles 重算：setTitle("terminal-0")
      // （setTitle 联动 onDidTitleChange；agentSession 空 → 捕获重算名）
      await act(async () => {
        mocks.mockApi.setTitle("terminal-0");
      });
      // 跑 claude → 退出：恢复标题应为重算名，而非挂载快照的瞬态 "claude"
      await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
      await act(async () => {
        mocks.getOscHandler(133)!("C;claude");
      });
      await act(async () => {
        mocks.getOscHandler(133)!("D;0");
      });
      expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("terminal-0");
    } finally {
      mocks.mockApi.title = "terminal-0";
    }
  });

  it("B12: customTitle 存在时重算标题不被捕获（F8 自定义名保持）", async () => {
    mocks.mockApi.title = "claude";
    try {
      render(React.createElement(TerminalPanel, {
        api: mocks.mockApi,
        params: { panelId: "test-b12c", customTitle: "我的终端" },
      }));
      await waitFor(() => expect(mocks.getTitleCb()).toBeDefined());
      // 重算 setTitle（生产不会发生——重算条件排除 customTitle；此处验证守卫兜底）
      await act(async () => {
        mocks.mockApi.setTitle("terminal-0");
      });
      // customTitle 存在 → 不捕获（originalTitleRef 保持挂载快照 "我的终端"）
      await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
      await act(async () => {
        mocks.getOscHandler(133)!("C;claude");
      });
      await act(async () => {
        mocks.getOscHandler(133)!("D;0");
      });
      expect(mocks.mockApi.setTitle).toHaveBeenLastCalledWith("我的终端");
    } finally {
      mocks.mockApi.title = "terminal-0";
    }
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

  it("OSC 133 C 命中 claude → sessionChange 驱动 tabLogo = claude logo（会话绑定）", async () => {
    // 预置注册表条目——setAgentSession 仅在已注册面板上生效（生产 register 由 spawn 完成触发）
    registerStub("test-logo1");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo1" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    await act(async () => {
      oscHandler("C;claude");
    });
    // C 路径：先 onTabStateChange（tabStatus attention）→ 后 setAgentSession({cliId})
    // → sessionChange → syncTabLogo 按 cliId 查 profile.iconSrc 写入 tabLogo
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabLogo: "/cli-icons/claude.png" }),
    );
    // 防复发（参数覆盖回归守卫）：tabLogo 写入必须基于最新参数合并——
    // props 快照覆盖会抹掉先写入的 tabStatus（mockcli E2E 冒烟 tabStatus 丢失根因）。
    // 断言最后一次 updateParameters 参数中两键共存
    const lastArgs = mocks.mockApi.updateParameters.mock.calls.at(-1)![0] as Record<
      string,
      unknown
    >;
    expect(lastArgs.tabStatus).toBe("attention");
    expect(lastArgs.tabLogo).toBe("/cli-icons/claude.png");
  });

  it("OSC 133 D → setAgentSession(null) → sessionChange → tabLogo 清空", async () => {
    registerStub("test-logo1b");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo1b" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;

    // C 建立会话 → D 结束：setAgentSession(null) → sessionChange → tabLogo null
    await act(async () => {
      oscHandler("C;claude");
    });
    await act(async () => {
      oscHandler("D;0");
    });
    // D 路径：先 onTabStateChange 单清 tabStatus → 后 sessionChange 清 tabLogo
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: null }),
    );
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabLogo: null }),
    );
  });

  it("hook 事件（无 command）→ sessionChange 按 agentSession.cliId 查 logo（C 已写 cliId）", async () => {
    registerStub("test-logo2");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo2" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const oscHandler = mocks.getOscHandler(133)!;
    const hookCb = mocks.getHookEventCb()!;

    // 先 OSC 133 C：agentSession.cliId = claude
    await act(async () => {
      oscHandler("C;claude");
    });
    // 再 hook 事件（Stop → done）：setAgentSession merge 保留 cliId → sessionChange
    // → syncTabLogo 查 claude iconSrc → tabLogo 保持
    await act(async () => {
      hookCb({ panelId: "test-logo2", event: "Stop" });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabLogo: "/cli-icons/claude.png" }),
    );
    // onTabStateChange 路径（后执行）只写 tabStatus
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabStatus: "done" }),
    );
  });

  it("hook 事件路径 agentSession.cliId 缺省 → CLAUDE_CLI_ID 兜底查 claude logo", async () => {
    registerStub("test-logo3");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo3" } }));
    await waitFor(() => expect(mocks.getHookEventCb()).toBeDefined());
    const hookCb = mocks.getHookEventCb()!;

    // SessionStart 建行（patch 无 cliId）→ setAgentSession（sessionChange → syncTabLogo：
    // session.cliId undefined → CLAUDE_CLI_ID 兜底，与 useAgentStatus 行建行口径一致）
    // → 其后 onTabStateChange 写 tabStatus（hook 路径顺序：session 先于 status）
    await act(async () => {
      hookCb({ panelId: "test-logo3", event: "SessionStart" });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabLogo: "/cli-icons/claude.png" }),
    );
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );
  });

  it("sessionChange：agentSession.cliId 未注册 → tabLogo null（不报错）", async () => {
    registerStub("test-logo4");
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo4" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());

    await act(async () => {
      TerminalRegistry.setAgentSession("test-logo4", { cliId: "unknown-cli" });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabLogo: null }),
    );
  });

  it("挂载初始化：布局残留 tabLogo + 无会话 → 清 null（覆盖持久化残留）", async () => {
    render(React.createElement(TerminalPanel, {
      api: mocks.mockApi,
      // 模拟布局 JSON 恢复的 tabLogo 残留（会话绑定制下不持久化语义，挂载即清）
      params: { panelId: "test-logo5", tabLogo: "/cli-icons/claude.png" },
    }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabLogo: null }),
    );
  });

  it("挂载初始化：agentSession 存在（页面切回）→ tabLogo 恢复", async () => {
    // 预置会话（H6：页面切回重挂载时 TerminalRegistry 保留会话）
    registerStub("test-logo6");
    TerminalRegistry.setAgentSession("test-logo6", { cliId: "claude" });
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo6" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());
    expect(mocks.mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ tabLogo: "/cli-icons/claude.png" }),
    );
  });

  it("register 事件（携 agentSession）→ 同步 tabLogo", async () => {
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo7" } }));
    await waitFor(() => expect(mocks.getOscHandler(133)).toBeDefined());

    await act(async () => {
      registerStub("test-logo7", { cliId: "claude", lastEventAt: Date.now() });
    });
    expect(mocks.mockApi.updateParameters).toHaveBeenLastCalledWith(
      expect.objectContaining({ tabLogo: "/cli-icons/claude.png" }),
    );
  });

  it("OSC 133 C 未命中 profile（注册表清空）→ 零副作用（标题/图标均不更新）", async () => {
    cliProfileRegistry._reset(); // 清空全部 profile → matchByCommand("claude") 返回 null
    render(React.createElement(TerminalPanel, { api: mocks.mockApi, params: { panelId: "test-logo8" } }));
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
});
