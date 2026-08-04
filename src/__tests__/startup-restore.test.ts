// startup-restore.test.ts — S4 启动恢复路径自动化测试
//
// Mock localStorage + stores + Workspace，渲染 <App /> 验证启动恢复路径：
// 1. localStorage 有记录 → 恢复 activePageId
// 2. localStorage 为空 → 静默降级
// 3. loadAllProjects 异常 → 不阻塞启动
// 4. ready 状态切换：加载中 → 就绪
// 5. DBG-6：setProjectRoot 先于 setActivePage（D7 时序断言，WRK-03）
// 6. requestUserAttention reject → 静默 catch（WRK-03）

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockLoadAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockSaveAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockMarkPersistenceReady = vi.fn();
  const mockSetActivePage = vi.fn();
  const mockSetProjectRoot = vi.fn().mockResolvedValue(undefined);
  const mockRequestUserAttention = vi.fn().mockResolvedValue(undefined);
  /** 窗口焦点变化回调（onFocusChanged 捕获，供测试触发） */
  let capturedFocusCb: ((focused: boolean) => void) | null = null;
  /** 可变项目数据（默认空——本地存储恢复路径无匹配项目） */
  let mockProjects: Record<string, unknown> = {};

  return {
    mockLoadAllProjects,
    mockSaveAllProjects,
    mockMarkPersistenceReady,
    mockSetActivePage,
    mockSetProjectRoot,
    mockRequestUserAttention,
    get capturedFocusCb() {
      return capturedFocusCb;
    },
    set capturedFocusCb(cb: ((focused: boolean) => void) | null) {
      capturedFocusCb = cb;
    },
    get mockProjects() {
      return mockProjects;
    },
    set mockProjects(p: Record<string, unknown>) {
      mockProjects = p;
    },
    resetAll() {
      mockLoadAllProjects.mockClear();
      mockSaveAllProjects.mockClear();
      mockMarkPersistenceReady.mockClear();
      mockSetActivePage.mockClear();
      mockSetProjectRoot.mockClear();
      mockRequestUserAttention.mockClear();
      capturedFocusCb = null;
      mockProjects = {};
    },
  };
});

// ─── Module mocks ───
vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn(() => () => {}),
  onFocusChanged: vi.fn((cb: (focused: boolean) => void) => {
    mocks.capturedFocusCb = cb;
    return () => {};
  }),
  requestUserAttention: mocks.mockRequestUserAttention,
}));

// App.tsx 启动恢复链路依赖 setProjectRoot（DBG-6 时序断言需 spy）
vi.mock("../ipc/fs", () => ({
  setProjectRoot: mocks.mockSetProjectRoot,
}));

vi.mock("../workspace", () => ({
  Workspace: () => React.createElement("div", { "data-testid": "workspace" }),
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({ panels: {} })),
}));

vi.mock("../stores/layout", () => ({
  useLayout: {
    getState: vi.fn(() => ({
      activePageId: "test-page-1",
      setActivePage: mocks.mockSetActivePage,
    })),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
}));

vi.mock("../stores/projects", () => ({
  useProjects: {
    getState: vi.fn(() => ({
      projects: mocks.mockProjects,
      updatePageLayout: vi.fn(),
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

import App from "../App";

// ─── localStorage stub ───
function setLastActivePage(value: string | null) {
  const store = new Map<string, string>();
  if (value) store.set("slterm-last-active-page", value);
  const stub = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => { store.set(k, v); },
    removeItem: (k: string) => { store.delete(k); },
    clear: () => { store.clear(); },
    get length() { return store.size; },
    key: (i: number) => [...store.keys()][i] ?? null,
  };
  Object.defineProperty(window, "localStorage", { value: stub, writable: true, configurable: true });
  (globalThis as unknown as Record<string, unknown>).localStorage = stub;
  return stub;
}

// ─── Tests ───
describe("S4 启动恢复", () => {
  beforeEach(() => {
    mocks.resetAll();
    delete (window as unknown as Record<string, unknown>).__dockviewApi;
  });

  it("1. localStorage 有 last-active-page → setActivePage 被调用", async () => {
    setLastActivePage("stored-page-id");

    render(React.createElement(App));

    await waitFor(() => {
      expect(mocks.mockSetActivePage).toHaveBeenCalledWith("stored-page-id");
    }, { timeout: 3000 });
  });

  it("2. localStorage 为空 → 静默降级（无异常，setActivePage 不被调用）", async () => {
    setLastActivePage(null);

    render(React.createElement(App));

    await waitFor(() => {
      expect(mocks.mockMarkPersistenceReady).toHaveBeenCalled();
    }, { timeout: 3000 });
    // setActivePage 不应被调用（无 localstorage 值）
    expect(mocks.mockSetActivePage).not.toHaveBeenCalled();
  });

  it("3. loadAllProjects 异常 → 静默降级，应用仍进入 ready 状态", async () => {
    mocks.mockLoadAllProjects.mockRejectedValueOnce(new Error("文件损坏"));
    setLastActivePage(null);

    render(React.createElement(App));

    // 不应崩溃；ready 仍为 true（Workspace 被渲染）
    await waitFor(() => {
      expect(mocks.mockMarkPersistenceReady).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it("4. 启动时显示 Loading → 数据就绪后消失（ready 状态切换）", async () => {
    setLastActivePage(null);
    // 延迟 loadAllProjects 以观察 Loading 状态
    let resolveLoad: () => void;
    mocks.mockLoadAllProjects.mockReturnValueOnce(
      new Promise<void>((r) => { resolveLoad = r; }),
    );

    const { queryByText } = render(React.createElement(App));

    // 初始：Loading 画面可见
    expect(queryByText("slTerminal 启动中…")).toBeTruthy();

    // 完成加载
    resolveLoad!();
    await waitFor(() => {
      expect(queryByText("slTerminal 启动中…")).toBeFalsy();
    }, { timeout: 3000 });
  });

  it("5. DBG-6: setProjectRoot 先于 setActivePage 完成（D7 时序断言）", async () => {
    setLastActivePage("stored-page-id");
    // 种子项目：含匹配 lastPage 的页面 + rootPath（触发恢复路径）
    mocks.mockProjects = {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath: "C:\\proj",
        pages: [
          { pageId: "stored-page-id", name: "p", layout: {}, cwd: "C:\\proj", createdAt: 1, lastAccessedAt: 1 },
        ],
      },
    };
    // setProjectRoot 挂起——期间 setActivePage 不得被调用
    let resolveRoot!: (v: unknown) => void;
    mocks.mockSetProjectRoot.mockReturnValueOnce(
      new Promise<unknown>((r) => { resolveRoot = r; }),
    );

    render(React.createElement(App));

    // setProjectRoot 已调用（参数为项目 rootPath），但 await 未完成 → setActivePage 未调用
    await waitFor(() => {
      expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith("C:\\proj");
    }, { timeout: 3000 });
    expect(mocks.mockSetActivePage).not.toHaveBeenCalled();

    // setProjectRoot resolve 后 setActivePage 才执行
    resolveRoot(undefined);
    await waitFor(() => {
      expect(mocks.mockSetActivePage).toHaveBeenCalledWith("stored-page-id");
    }, { timeout: 3000 });

    // 调用顺序：setProjectRoot 先于 setActivePage（D7）
    const rootOrder = mocks.mockSetProjectRoot.mock.invocationCallOrder[0];
    const pageOrder = mocks.mockSetActivePage.mock.invocationCallOrder[0];
    expect(rootOrder).toBeLessThan(pageOrder);
  });

  it("6. setProjectRoot 失败 → console.error 降级后仍 setActivePage", async () => {
    setLastActivePage("stored-page-id");
    mocks.mockProjects = {
      "proj-1": {
        projectId: "proj-1",
        rootPath: "C:\\proj",
        pages: [
          { pageId: "stored-page-id", name: "p", layout: {}, cwd: "C:\\proj", createdAt: 1, lastAccessedAt: 1 },
        ],
      },
    };
    mocks.mockSetProjectRoot.mockRejectedValueOnce(new Error("root 失败"));

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(React.createElement(App));

    // 降级后恢复仍完成
    await waitFor(() => {
      expect(mocks.mockSetActivePage).toHaveBeenCalledWith("stored-page-id");
    }, { timeout: 3000 });
    // 失败日志已输出
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining("[slTerminal] 启动恢复—设置项目根路径失败:"),
      expect.any(Error),
    );

    consoleSpy.mockRestore();
  });

  it("7. requestUserAttention reject → 静默 catch（无 console.error、不冒泡）", async () => {
    setLastActivePage(null);
    // 窗口获得焦点时 requestUserAttention(null) reject
    mocks.mockRequestUserAttention.mockRejectedValueOnce(new Error("attention 失败"));

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(React.createElement(App));

    // 捕获 onFocusChanged 回调后模拟窗口获得焦点
    await waitFor(() => {
      expect(mocks.capturedFocusCb).not.toBeNull();
    }, { timeout: 3000 });
    mocks.capturedFocusCb!(true);

    // requestUserAttention(null) 被调用（停止任务栏闪烁）
    await waitFor(() => {
      expect(mocks.mockRequestUserAttention).toHaveBeenCalledWith(null);
    }, { timeout: 3000 });
    // 静默 catch：不输出任何错误日志
    expect(consoleSpy).not.toHaveBeenCalled();

    consoleSpy.mockRestore();
  });
});
