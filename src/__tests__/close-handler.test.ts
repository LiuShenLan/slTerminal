// close-handler.test.ts — App onCloseRequested 关闭钩子自动化测试
//
// 用 vi.mock 替代 ipc/window + stores + layoutSerde + Workspace，
// render <App /> 后捕获 registerCloseHandler 回调，模拟关闭事件验证保存序列。
// 覆盖：正常关闭 / 保存失败仍销毁 / 无 activePageId / 无 __dockviewApi

import { describe, it, expect, beforeEach, vi } from "vitest";
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
  /** 可变 activePageId（test 3 设为 null 模拟无活跃页面） */
  let mockActivePageId: string | null = "test-page-1";

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
    /** 完整重置（beforeEach 调用） */
    resetAll() {
      capturedHandler = null;
      mockActivePageId = "test-page-1";
      mockSaveAllProjects.mockClear();
      mockLoadAllProjects.mockClear();
      mockMarkPersistenceReady.mockClear();
      mockUpdatePageLayout.mockClear();
      mockSetActivePage.mockClear();
    },
  };
});

// ─── Module mocks ───

vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn((cb: () => Promise<void>) => {
    mocks.capturedHandler = cb;
    return () => {};
  }),
}));

vi.mock("../workspace", () => ({
  Workspace: () => React.createElement("div", { "data-testid": "workspace" }),
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({ panels: {}, grid: {} })),
}));

// P1-19: mock pty + TerminalRegistry（App 关闭时 kill 活跃 session）
vi.mock("../ipc", () => ({
  pty: { kill: vi.fn().mockResolvedValue(undefined) },
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

// P1-03: mock lib ErrorBoundary（避免渲染实际组件）
vi.mock("../lib", () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

vi.mock("../stores/layout", () => ({
  useLayout: {
    getState: vi.fn(() => ({
      activePageId: mocks.mockActivePageId,
      setActivePage: mocks.mockSetActivePage,
    })),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
}));

vi.mock("../stores/projects", () => ({
  useProjects: {
    getState: vi.fn(() => ({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          pages: [{ pageId: "test-page-1", name: "Test", layout: { panels: {} } }],
        },
      },
      updatePageLayout: mocks.mockUpdatePageLayout,
    })),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
  loadAllProjects: mocks.mockLoadAllProjects,
  saveAllProjects: mocks.mockSaveAllProjects,
  cancelPendingSave: vi.fn(),
  markPersistenceReady: mocks.mockMarkPersistenceReady,
}));

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
  });
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

    // 验证三步保存序列（preventDefault + destroy 由 registerCloseHandler 封装处理）
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

    // 推进 3000ms 触发 race 超时
    vi.advanceTimersByTime(3000);

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
});
