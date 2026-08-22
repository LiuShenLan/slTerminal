// close-handler.test.ts — App onCloseRequested 关闭钩子自动化测试
//
// 用 vi.mock 替代 ipc/window + stores + layoutSerde + Workspace，
// render <App /> 后捕获 registerCloseHandler 回调，模拟关闭事件验证保存序列。
// 覆盖：正常关闭 / 保存失败仍销毁 / 无 activePageId / 无 __dockviewApi

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render, waitFor } from "@testing-library/react";

// ─── Hoisted：跨 mock 工厂共享的 mutable 状态 ───
const mocks = vi.hoisted(() => {
  let capturedHandler: (() => void | Promise<void>) | null = null;

  const mockSaveAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockLoadAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockMarkPersistenceReady = vi.fn();
  const mockUpdatePageLayout = vi.fn();
  const mockSetActivePage = vi.fn();
  /** 关闭序列第 4 步：statusline 桥接恢复（备份还原，cliId 恒 claude） */
  const mockRestoreStatusline = vi.fn().mockResolvedValue(undefined);
  /** 可变 activePageId（test 3 设为 null 模拟无活跃页面） */
  let mockActivePageId: string | null = "test-page-1";
  /** FE-28：ErrorBoundary mock 收到的 variant 序列（"fullscreen" | "inline"） */
  const mockErrorBoundaryVariants: string[] = [];

  return {
    get capturedHandler() {
      return capturedHandler;
    },
    set capturedHandler(h: (() => void | Promise<void>) | null) {
      capturedHandler = h;
    },
    get mockActivePageId() {
      return mockActivePageId;
    },
    set mockActivePageId(v: string | null) {
      mockActivePageId = v;
    },
    mockSaveAllProjects,
    mockLoadAllProjects,
    mockMarkPersistenceReady,
    mockUpdatePageLayout,
    mockSetActivePage,
    mockRestoreStatusline,
    mockErrorBoundaryVariants,
    /** 完整重置（beforeEach 调用） */
    resetAll() {
      capturedHandler = null;
      mockActivePageId = "test-page-1";
      mockSaveAllProjects.mockClear();
      mockLoadAllProjects.mockClear();
      mockMarkPersistenceReady.mockClear();
      mockUpdatePageLayout.mockClear();
      mockSetActivePage.mockClear();
      mockRestoreStatusline.mockClear();
      mockErrorBoundaryVariants.length = 0;
    },
  };
});

// ─── Module mocks ───

vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn((cb: () => Promise<void>) => {
    mocks.capturedHandler = cb;
    return () => {};
  }),
  onFocusChanged: vi.fn(() => () => {}),
}));

// 关闭序列第 4 步：restoreStatusline（覆盖 setup.ts 全局 mock，spy 断言调用与 cliId 透传）
vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: () => () => {},
  inject: () => Promise.resolve({ status: "notInjected", version: null }),
  uninstall: () => Promise.resolve(),
  getInjectionStatus: () => Promise.resolve({ status: "notInjected", version: null }),
  restoreStatusline: mocks.mockRestoreStatusline,
}));

vi.mock("../workspace", () => ({
  Workspace: () => React.createElement("div", { "data-testid": "workspace" }),
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({ panels: {}, grid: {} })),
}));

// P1-19: mock pty + TerminalRegistry（App 关闭时 kill 活跃 session）
// BE-08: ptyKillAll 兜底命令（Registry kill 之后调用）
vi.mock("../ipc", () => ({
  pty: {
    kill: vi.fn().mockResolvedValue(undefined),
    ptyKillAll: vi.fn().mockResolvedValue(0),
  },
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: vi.fn(() => new Map()),
    register: vi.fn(),
    get: vi.fn(),
    remove: vi.fn(),
    has: vi.fn(),
    _size: vi.fn(() => 0),
    _dump: vi.fn(() => []),
    _clear: vi.fn(),
  },
}));

// P1-03: mock lib ErrorBoundary（避免渲染实际组件；记录 variant 供 FE-28 断言）
// TB-02: 补 TitleBar 三窗口图标（渲染 null，避免 jsdom 渲染警告干扰 console spy 断言）
vi.mock("../lib", () => ({
  // FE-28: 记录每次 ErrorBoundary 的 variant——顶层 fullscreen 1 处 + 五个顶层组件 inline 各 1 处
  ErrorBoundary: ({ children, variant }: { children: React.ReactNode; variant?: string }) => {
    mocks.mockErrorBoundaryVariants.push(variant ?? "fullscreen");
    return React.createElement(React.Fragment, null, children);
  },
  IconMin: () => null,
  IconMax: () => null,
  IconCloseWin: () => null,
  // OV-01: App 根部浮层挂载点（本测试不关心浮层，渲染 null）
  ConfirmDialogHost: () => null,
  ToastHost: () => null,
}));

// TB-02: useLayout 需同时支持 App 的 getState() 调用与 TitleBar 的 hook 调用（双形态函数）
vi.mock("../stores/layout", () => {
  const layoutState = () => ({
    activePageId: mocks.mockActivePageId,
    setActivePage: mocks.mockSetActivePage,
  });
  const useLayout = Object.assign(
    vi.fn((selector: (s: ReturnType<typeof layoutState>) => unknown) =>
      selector ? selector(layoutState()) : layoutState(),
    ),
    {
      getState: vi.fn(layoutState),
      setState: vi.fn(),
      subscribe: vi.fn(),
    },
  );
  return { useLayout };
});

// TB-02: useProjects 同上双形态（hook selector 调用返回 projects）
vi.mock("../stores/projects", () => {
  const projectsState = () => ({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        pages: [{ pageId: "test-page-1", name: "Test", layout: { panels: {} } }],
      },
    },
    updatePageLayout: mocks.mockUpdatePageLayout,
  });
  const useProjects = Object.assign(
    vi.fn((selector: (s: ReturnType<typeof projectsState>) => unknown) =>
      selector ? selector(projectsState()) : projectsState(),
    ),
    {
      getState: vi.fn(projectsState),
      setState: vi.fn(),
      subscribe: vi.fn(),
    },
  );
  return {
    useProjects,
    // S12 (88c4caa) 引入的共用 debounce 常量——fontSize/keybindings/sideBar 静态 import，
    // mock 缺导出会得 undefined → setTimeout(fn, undefined) 立即执行保存链 → App init 抛错渲染卡死
    PERSIST_DEBOUNCE_MS: 2000,
    loadAllProjects: mocks.mockLoadAllProjects,
    saveAllProjects: mocks.mockSaveAllProjects,
    cancelPendingSave: vi.fn(),
    markPersistenceReady: mocks.mockMarkPersistenceReady,
  };
});

vi.mock("dockview-react/dist/styles/dockview.css", () => ({}));

// 阻止真实 ShortcutRegistry 在 close-handler 测试中安装全局 keydown 监听器
vi.mock("../features/shortcuts", () => ({
  getShortcutRegistry: () => ({
    register: vi.fn(() => vi.fn()), // 返回空注销函数
    unregister: vi.fn(),
    pushContext: vi.fn(),
    popContext: vi.fn(),
    setOverrides: vi.fn(),
    _reset: vi.fn(),
  }),
  createGlobalShortcuts: vi.fn(() => []),
  usePanelFocus: vi.fn(),
  wireKeybindings: vi.fn(() => vi.fn()), // 返回空 unsubscribe
  // App 经面板 keyboard 工厂间接依赖 commandFromMeta（createTerminal/EditorShortcuts）
  commandFromMeta: (id: string, handler: (e: KeyboardEvent) => boolean) => ({
    id, handler, defaultKey: null, context: "global", priority: 0, title: id, category: "global",
  }),
}));

// ─── 导入被测模块（在 mocks 之后） ───
import App from "../App";

// ─── 测试辅助 ───

/** 内存 localStorage stub（Node 22 jsdom 中 localStorage 可能不可用） */
function setupLocalStorage() {
  const store = new Map<string, string>();
  const stub: Storage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => { store.set(k, v); },
    removeItem: (k: string) => { store.delete(k); },
    clear: () => { store.clear(); },
    get length() { return store.size; },
    key: (i: number) => [...store.keys()][i] ?? null,
  };
  // 双注册：window + globalThis（确保 bare `localStorage` 引用能解析）
  (globalThis as unknown as Record<string, unknown>).localStorage = stub;
  Object.defineProperty(window, "localStorage", { value: stub, writable: true, configurable: true });
  return stub;
}

/** 渲染 App 并等待 registerCloseHandler 回调注册完成 */
async function renderAndCapture(): Promise<() => void | Promise<void>> {
  render(React.createElement(App));
  // registerCloseHandler 在 useEffect 中同步注册，render 后立即可用
  await waitFor(() => {
    expect(mocks.capturedHandler).not.toBeNull();
  }, { timeout: 3000 });
  return mocks.capturedHandler!;
}

// ─── 测试套件 ───

describe("onCloseRequested 关闭钩子", () => {
  let localStorageStub: Storage;

  beforeEach(() => {
    mocks.resetAll();
    localStorageStub = setupLocalStorage();
    delete ((window as unknown) as Record<string, unknown>).__dockviewApi;
  });

  it("1. 正常关闭流程：flush layout → saveAllProjects → localStorage", async () => {
    // 模拟 Dockview API 已就绪
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    const handler = await renderAndCapture();
    await handler();

    // 验证四步保存序列（preventDefault + destroy 由 registerCloseHandler 封装处理）
    // flush: updatePageLayout 被调用
    expect(mocks.mockUpdatePageLayout).toHaveBeenCalledWith(
      "proj-1",
      "test-page-1",
      expect.any(Object),
    );
    // save
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();
    // localStorage
    expect(localStorageStub.getItem("slterm-last-active-page")).toBe("test-page-1");
    // 第 4 步：restoreStatusline 在保存序列内被调用（cliId 恒 claude 常量）
    expect(mocks.mockRestoreStatusline).toHaveBeenCalledWith("claude");
  });

  it("2. saveAllProjects 抛出异常 → 回调仍可完成（不崩溃，destroy 由 registerCloseHandler finally 保证）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };
    mocks.mockSaveAllProjects.mockRejectedValueOnce(new Error("磁盘满"));

    const handler = await renderAndCapture();
    await handler();

    // 不应崩溃
    expect(mocks.mockUpdatePageLayout).toHaveBeenCalled();
  });

  it("3. 无 activePageId → 跳过 flush layout 和 localStorage，仅 saveAllProjects", async () => {
    mocks.mockActivePageId = null;
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    const handler = await renderAndCapture();
    await handler();

    // flush 和 localStorage 应跳过
    expect(mocks.mockUpdatePageLayout).not.toHaveBeenCalled();
    expect(localStorageStub.getItem("slterm-last-active-page")).toBeNull();
    // save 仍需调用
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();
  });

  it("4. 无 __dockviewApi → 跳过 layout flush，仅 saveAllProjects + localStorage", async () => {
    // 不设置 window.__dockviewApi（模拟 Dockview 未初始化场景）

    const handler = await renderAndCapture();
    await handler();

    // flush 跳过（无 __dockviewApi）
    expect(mocks.mockUpdatePageLayout).not.toHaveBeenCalled();
    // save + localStorage 仍需调用
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();
    expect(localStorageStub.getItem("slterm-last-active-page")).toBe("test-page-1");
  });

  it("5. saveAllProjects 超过 3s → Promise.race 超时触发 → 回调完成", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // 先用真实定时器捕获 handler
    const handler = await renderAndCapture();

    // 切换到假定时器
    vi.useFakeTimers();

    // saveAllProjects 返回永不完结的 Promise
    let neverResolve: () => void;
    const neverPromise = new Promise<void>((resolve) => {
      neverResolve = resolve;
    });
    mocks.mockSaveAllProjects.mockReturnValue(neverPromise);

    // 触发关闭（不要 await，handler 会卡在 Promise.race）
    const closePromise = handler();

    // 推进 3000ms 触发 race 超时——须用异步版：handler 挂起在 ptyKillAll 等
    // await 微任务链上，同步 advance 不 flush 微任务，race 的 setTimeout 尚未注册
    await vi.advanceTimersByTimeAsync(3000);

    await closePromise;

    // 不应卡死
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();

    vi.useRealTimers();
    neverResolve!(); // 清理未完结的 Promise
  });

  it("6. localStorage.setItem 抛异常 → 静默捕获，不阻止完成", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // 让 localStorage.setItem 抛出异常
    localStorageStub.setItem = () => {
      throw new Error("quota exceeded");
    };

    const handler = await renderAndCapture();
    await handler();

    // 不应崩溃
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();
  });

  it("7. restoreStatusline 失败 → 静默 catch，不阻断关闭（错误仅日志）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };
    mocks.mockRestoreStatusline.mockRejectedValueOnce(new Error("settings.json 写入失败"));

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const handler = await renderAndCapture();
    // 不应抛异常
    await expect(handler()).resolves.toBeUndefined();
    // 前序保存不受影响
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();

    consoleSpy.mockRestore();
  });
});

// ─── P1-7: PTY kill 路径测试 ───

import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { pty } from "../ipc";

describe("onCloseRequested PTY kill 路径", () => {
  beforeEach(() => {
    mocks.resetAll();
    setupLocalStorage();
    delete ((window as unknown) as Record<string, unknown>).__dockviewApi;
    // 重置 TerminalRegistry mock 为空 Map
    vi.mocked(TerminalRegistry.getAll).mockReturnValue(new Map());
    vi.mocked(pty.kill).mockReset();
    vi.mocked(pty.kill).mockResolvedValue(undefined);
    // BE-08: 重置兜底 killAll（默认 0 个残留 session）
    vi.mocked(pty.ptyKillAll).mockReset();
    vi.mocked(pty.ptyKillAll).mockResolvedValue(0);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function renderAndCapture(): Promise<() => void | Promise<void>> {
    render(React.createElement(App));
    await waitFor(() => {
      expect(mocks.capturedHandler).not.toBeNull();
    }, { timeout: 3000 });
    return mocks.capturedHandler!;
  }

  it("7. TerminalRegistry 有活跃 session → pty.kill 被调用", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
        ["panel-2", { term: {} as never, sessionId: "session-002", webglAddon: null, fitAddon: {} as never }],
      ]),
    );

    const handler = await renderAndCapture();
    await handler();

    // 每个 session 都调用了 pty.kill（含 panelId——App.tsx 从 Map key 获取）
    expect(pty.kill).toHaveBeenCalledWith("session-001", "panel-1");
    expect(pty.kill).toHaveBeenCalledWith("session-002", "panel-2");
    expect(pty.kill).toHaveBeenCalledTimes(2);
  });

  it("8. 单条 kill 失败 → 不阻塞其他 kill，全部结束后统一一条汇总日志（FE-05）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // session-001 的 kill 失败
    vi.mocked(pty.kill)
      .mockRejectedValueOnce(new Error("process not found"))
      .mockResolvedValueOnce(undefined);

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
        ["panel-2", { term: {} as never, sessionId: "session-002", webglAddon: null, fitAddon: {} as never }],
      ]),
    );

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const handler = await renderAndCapture();
    await handler();

    // 两个 kill 都被调用
    expect(pty.kill).toHaveBeenCalledTimes(2);
    // FE-05：统一一条汇总日志（含失败数 1），替代逐条 console.error
    expect(consoleSpy).toHaveBeenCalledTimes(1);
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining("[slTerminal] 关闭时 1 个 PTY session kill 失败:"),
      expect.any(Array),
    );
    // 汇总数组内含失败归属（sessionId + panelId + 错误）
    const failures = consoleSpy.mock.calls[0][1] as Array<Record<string, unknown>>;
    expect(failures).toHaveLength(1);
    expect(failures[0]).toMatchObject({ sessionId: "session-001", panelId: "panel-1" });

    consoleSpy.mockRestore();
  });

  it("9. 并发 kill + 单条失败 → Promise.all 不 reject（每条有 .catch）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // 全部失败也不抛异常
    vi.mocked(pty.kill).mockRejectedValue(new Error("kill failed"));

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
      ]),
    );

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const handler = await renderAndCapture();
    // 不应抛异常
    await expect(handler()).resolves.toBeUndefined();

    consoleSpy.mockRestore();
  });

  it("10. kill 超过 3s → Promise.race 超时不阻塞关闭", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // kill 永远不 resolve
    vi.mocked(pty.kill).mockReturnValue(new Promise(() => {}));

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
      ]),
    );

    const handler = await renderAndCapture();

    vi.useFakeTimers();

    // 触发关闭
    const closePromise = handler();

    // 推进 3000ms 触发 race 超时（异步版：flush 微任务链后再推进，理由见测试 5）
    await vi.advanceTimersByTimeAsync(3000);

    await closePromise;

    // 不应卡死
    expect(pty.kill).toHaveBeenCalledWith("session-001", "panel-1");

    vi.useRealTimers();
  });

  it("11. TerminalRegistry 为空 Map → pty.kill 不调用", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(new Map());

    const handler = await renderAndCapture();
    await handler();

    // size=0 → 整个 kill 代码块跳过
    expect(pty.kill).not.toHaveBeenCalled();
  });

  it("12. 多条 kill 失败 → 汇总日志含失败数（FE-05）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // 全部失败
    vi.mocked(pty.kill).mockRejectedValue(new Error("kill failed"));

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
        ["panel-2", { term: {} as never, sessionId: "session-002", webglAddon: null, fitAddon: {} as never }],
        ["panel-3", { term: {} as never, sessionId: "session-003", webglAddon: null, fitAddon: {} as never }],
      ]),
    );

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const handler = await renderAndCapture();
    await handler();

    // FE-05：仍只汇总一条日志，失败数 = 3，条目含全部归属
    expect(consoleSpy).toHaveBeenCalledTimes(1);
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining("[slTerminal] 关闭时 3 个 PTY session kill 失败:"),
      expect.any(Array),
    );
    const failures = consoleSpy.mock.calls[0][1] as Array<Record<string, unknown>>;
    expect(failures).toHaveLength(3);
    expect(failures.map((f) => f.sessionId)).toEqual(["session-001", "session-002", "session-003"]);

    consoleSpy.mockRestore();
  });

  it("13. Registry 有活跃 session → 先逐 session kill，再 ptyKillAll 兜底（BE-08 顺序）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(
      new Map([
        ["panel-1", { term: {} as never, sessionId: "session-001", webglAddon: null, fitAddon: {} as never }],
      ]),
    );
    vi.mocked(pty.ptyKillAll).mockResolvedValue(2); // 后端兜底清理 2 个残留 session

    const handler = await renderAndCapture();
    await handler();

    // Registry kill 与 ptyKillAll 都被调用
    expect(pty.kill).toHaveBeenCalledWith("session-001", "panel-1");
    expect(pty.ptyKillAll).toHaveBeenCalledTimes(1);
    // 顺序契约：Registry 快速 kill 先于兜底 killAll
    expect(vi.mocked(pty.kill).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(pty.ptyKillAll).mock.invocationCallOrder[0],
    );
  });

  it("14. Registry 为空 → ptyKillAll 仍被调用（后端 session 兜底不依赖前端映射）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(TerminalRegistry.getAll).mockReturnValue(new Map());

    const handler = await renderAndCapture();
    await handler();

    // 前端无 session 记录，但后端可能残留——兜底照常执行
    expect(pty.kill).not.toHaveBeenCalled();
    expect(pty.ptyKillAll).toHaveBeenCalledTimes(1);
  });

  it("15. ptyKillAll 失败 → 不阻断关闭（仅错误日志）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(pty.ptyKillAll).mockRejectedValue(new Error("backend gone"));

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const handler = await renderAndCapture();
    await expect(handler()).resolves.toBeUndefined();
    // 后续保存序列不受影响
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();

    consoleSpy.mockRestore();
  });

  it("16. ptyKillAll 返回 kill 数 > 0 → console.info 记录兜底清理数", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    vi.mocked(pty.ptyKillAll).mockResolvedValue(3);

    const infoSpy = vi.spyOn(console, "info").mockImplementation(() => {});

    const handler = await renderAndCapture();
    await handler();

    expect(infoSpy).toHaveBeenCalledWith(
      expect.stringContaining("关闭兜底清理 3 个后端残留 PTY session"),
    );

    infoSpy.mockRestore();
  });

  it("17. ptyKillAll 永不 resolve → 3s 超时后关窗流程继续（FE-47）", async () => {
    (window as unknown as Record<string, unknown>).__dockviewApi = { _mock: true };

    // ptyKillAll 永远不 resolve（后端逐 session 串行 kill 卡死场景）
    vi.mocked(pty.ptyKillAll).mockReturnValue(new Promise(() => {}));

    const handler = await renderAndCapture();

    vi.useFakeTimers();

    // 触发关闭
    const closePromise = handler();

    // 推进 3000ms 触发 ptyKillAll race 超时（异步版：flush 微任务链后再推进，
    // race 的 setTimeout 须先注册；理由见测试 5）——超时后 killed=null，分支跳过
    await vi.advanceTimersByTimeAsync(3000);

    await closePromise;

    // 不应卡死——后续保存序列照常执行
    expect(mocks.mockSaveAllProjects).toHaveBeenCalled();

    vi.useRealTimers();
  });
});

// ─── FE-28: App 五个顶层组件分别包 inline ErrorBoundary ───

describe("FE-28 App 顶层组件错误边界", () => {
  beforeEach(() => {
    mocks.resetAll();
    setupLocalStorage();
    delete ((window as unknown) as Record<string, unknown>).__dockviewApi;
  });

  it("1. ready 后渲染五个 inline 边界（TitleBar/Workspace/NotificationListener/ConfirmDialogHost/ToastHost）", async () => {
    render(React.createElement(App));

    // 等待启动完成（ready=true → 完整骨架渲染）：inline 边界恰 5 处
    await waitFor(() => {
      expect(
        mocks.mockErrorBoundaryVariants.filter((v) => v === "inline"),
      ).toHaveLength(5);
    }, { timeout: 3000 });
    // 外层 fullscreen 兜底保留（启动中 1 处 + ready 后 1 处）——filter 返回数组，须取 .length 再数值断言
    expect(
      mocks.mockErrorBoundaryVariants.filter((v) => v === "fullscreen").length,
    ).toBeGreaterThanOrEqual(2);
  });
});
