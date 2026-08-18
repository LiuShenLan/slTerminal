// startup-store-fail-warn.test.tsx — App 启动链三 store loadFromDisk 失败告警（FE-03）
//
// 三个 store（fontSize/keybindings/sideBar）的 loadFromDisk 内部自行吞错，
// App 外层 catch 为防御性兜底——本测试 mock 三个 store 使其 reject，
// 断言 App 启动链各 catch 均 console.warn 带模块名 [App]（降级兜底逻辑不动）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockFontSizeLoad = vi.fn().mockRejectedValue(new Error("fontSize 读取失败"));
  const mockKeybindingsLoad = vi.fn().mockRejectedValue(new Error("keybindings 读取失败"));
  const mockSideBarLoad = vi.fn().mockRejectedValue(new Error("sideBar 读取失败"));
  const mockLoadAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockMarkPersistenceReady = vi.fn();
  const mockSetActivePage = vi.fn();
  const mockSetProjectRoot = vi.fn().mockResolvedValue(undefined);

  return {
    mockFontSizeLoad,
    mockKeybindingsLoad,
    mockSideBarLoad,
    mockLoadAllProjects,
    mockMarkPersistenceReady,
    mockSetActivePage,
    mockSetProjectRoot,
    resetAll() {
      mockFontSizeLoad.mockClear();
      mockKeybindingsLoad.mockClear();
      mockSideBarLoad.mockClear();
      mockLoadAllProjects.mockClear();
      mockMarkPersistenceReady.mockClear();
      mockSetActivePage.mockClear();
      mockSetProjectRoot.mockClear();
    },
  };
});

// ─── Module mocks ───
vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn(() => () => {}),
  onFocusChanged: vi.fn(() => () => {}),
  requestUserAttention: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc/fs", () => ({
  setProjectRoot: mocks.mockSetProjectRoot,
}));

vi.mock("../workspace", () => ({
  Workspace: () => React.createElement("div", { "data-testid": "workspace" }),
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({ panels: {} })),
}));

// TB-02: useLayout 需同时支持 App 的 getState() 调用与 TitleBar 的 hook 调用（双形态函数）
vi.mock("../stores/layout", () => {
  const layoutState = () => ({
    activePageId: null,
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

vi.mock("../stores/projects", () => {
  const projectsState = () => ({
    projects: {},
    updatePageLayout: vi.fn(),
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
    loadAllProjects: mocks.mockLoadAllProjects,
    saveAllProjects: vi.fn().mockResolvedValue(undefined),
    cancelPendingSave: vi.fn(),
    markPersistenceReady: mocks.mockMarkPersistenceReady,
  };
});

// FE-03：三 store loadFromDisk 全部 reject——触发 App 启动链各防御性 catch
vi.mock("../stores/fontSize", () => ({
  useFontSize: {
    getState: () => ({ loadFromDisk: mocks.mockFontSizeLoad }),
  },
  cancelPendingSave: vi.fn(),
}));

vi.mock("../stores/keybindings", () => ({
  useKeybindings: {
    getState: () => ({
      loadFromDisk: mocks.mockKeybindingsLoad,
      // FE-03 测试 mock 完整性：App wireKeybindings 需要 overrides + subscribe（缺 subscribe 会抛 TypeError）
      overrides: {},
    }),
    // wireKeybindings 经 store.subscribe 持续同步覆盖层——mock 返回空取消函数
    subscribe: vi.fn(() => () => {}),
  },
  cancelPendingSave: vi.fn(),
}));

vi.mock("../stores/sideBar", () => ({
  useSideBar: {
    getState: () => ({ loadFromDisk: mocks.mockSideBarLoad }),
  },
  cancelPendingSave: vi.fn(),
}));

vi.mock("dockview-react/dist/styles/dockview.css", () => ({}));

import App from "../App";

/** 内存 localStorage stub（Node 22 jsdom 中 localStorage 可能不可用——照 startup-restore 先例） */
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
  (globalThis as unknown as Record<string, unknown>).localStorage = stub;
  Object.defineProperty(window, "localStorage", { value: stub, writable: true, configurable: true });
  return stub;
}

describe("App 启动链 store 加载失败告警（FE-03）", () => {
  beforeEach(() => {
    mocks.resetAll();
    setupLocalStorage();
    delete (window as unknown as Record<string, unknown>).__dockviewApi;
  });

  it("1. 三 store loadFromDisk 全部失败 → 各 catch console.warn 带模块名 [App]，启动不阻塞", async () => {
    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(React.createElement(App));

    // 降级兜底：启动流程仍完成（markPersistenceReady 被调用）
    await waitFor(() => {
      expect(mocks.mockMarkPersistenceReady).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 三个 catch 各输出一条带模块名 [App] 的告警
    expect(consoleWarnSpy).toHaveBeenCalledTimes(3);
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[App] 加载字体大小设置失败，保持默认值:"),
      expect.any(Error),
    );
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[App] 加载快捷键设置失败，保持默认绑定:"),
      expect.any(Error),
    );
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[App] 加载侧栏设置失败，保持默认值:"),
      expect.any(Error),
    );

    consoleWarnSpy.mockRestore();
  });
});
