// startup-store-fail-warn.test.tsx — App 启动链 store loadFromDisk 失败告警（FE-03）
//
// settings 类 store 的 loadFromDisk 内部自行吞错，App 外层 catch 为防御性兜底——
// 本测试 mock fontSize/keybindings/sideBar/cliAliases 四 store 使其 reject，断言 App
// 启动链各 catch 均 console.warn 带模块名 [App]（降级兜底逻辑不动）。CP-009 后启动
// 链新增第 5 个 store（conptyInputModes），mock 为加载成功——计数口径保持四 store
// 失败各一次（照 CP-010 为 getConptyStatus 补 mock 的先例）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockFontSizeLoad = vi.fn().mockRejectedValue(new Error("fontSize 读取失败"));
  const mockKeybindingsLoad = vi.fn().mockRejectedValue(new Error("keybindings 读取失败"));
  const mockSideBarLoad = vi.fn().mockRejectedValue(new Error("sideBar 读取失败"));
  const mockCliAliasesLoad = vi.fn().mockRejectedValue(new Error("cliAliases 读取失败"));
  // CP-009: 第 5 个 settings 类 store（conptyInputModes）mock 为加载成功——
  // 本文件聚焦 FE-03 四 store 失败告警计数，真实 loadFromDisk 在 jsdom 无 IPC
  // 后端下 reject → store 内 catch + App 外层 catch 双 warn，干扰 4 次断言
  // （照 CP-010 为 getConptyStatus 补 mock 的先例）
  const mockConptyModesLoad = vi.fn().mockResolvedValue(undefined);
  const mockLoadAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockMarkPersistenceReady = vi.fn();
  const mockSetActivePage = vi.fn();
  const mockSetProjectRoot = vi.fn().mockResolvedValue(undefined);

  return {
    mockFontSizeLoad,
    mockKeybindingsLoad,
    mockSideBarLoad,
    mockCliAliasesLoad,
    mockConptyModesLoad,
    mockLoadAllProjects,
    mockMarkPersistenceReady,
    mockSetActivePage,
    mockSetProjectRoot,
    resetAll() {
      mockFontSizeLoad.mockClear();
      mockKeybindingsLoad.mockClear();
      mockSideBarLoad.mockClear();
      mockCliAliasesLoad.mockClear();
      mockConptyModesLoad.mockClear();
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

// CP-010: App 启动链紧随 store 加载后调一次 getConptyStatus（回退 toast 数据源）。
// 本文件聚焦 FE-03 四 store 失败告警计数——mock 为 resolve(未尝试回退),否则
// jsdom 无 IPC 后端 → invoke reject → catch 产出第 5 条 warn 干扰 4 次断言
vi.mock("../ipc/pty", () => ({
  getConptyStatus: vi.fn().mockResolvedValue({
    attempted: false,
    bundled: false,
    fallbackReason: null,
  }),
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
    markLoadSucceeded: vi.fn(),
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

vi.mock("../stores/cliAliases", () => ({
  useCliAliases: {
    getState: () => ({
      loadFromDisk: mocks.mockCliAliasesLoad,
      aliases: {},
    }),
    // App 别名快照同步 effect 经 store.subscribe 持续同步——mock 返回空取消函数
    subscribe: vi.fn(() => () => {}),
  },
  cancelPendingSave: vi.fn(),
}));

// CP-009: App 启动链第 5 个 store 加载（conptyInputModes）——mock 加载成功（resolve
// 不产 warn）。测试目标仍是「四 store 失败各 warn 一次」，计数口径不含第 5 个
vi.mock("../stores/conptyInputModes", () => ({
  useConptyInputModes: {
    getState: () => ({ loadFromDisk: mocks.mockConptyModesLoad }),
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

  it("1. 四 store loadFromDisk 全部失败 → 各 catch console.warn 带模块名 [App]，启动不阻塞", async () => {
    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(React.createElement(App));

    // 降级兜底：启动流程仍完成（markPersistenceReady 被调用）
    await waitFor(() => {
      expect(mocks.mockMarkPersistenceReady).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 四个 catch 各输出一条带模块名 [App] 的告警
    expect(consoleWarnSpy).toHaveBeenCalledTimes(4);
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
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[App] 加载 CLI 别名设置失败，保持默认空别名:"),
      expect.any(Error),
    );

    consoleWarnSpy.mockRestore();
  });

  it("2. FE-20: 四 store loadFromDisk 并行发起（Promise.all）——任一挂起时其余仍被调用", async () => {
    // 四个 loadFromDisk 全部返回永不 resolve 的挂起 promise：
    // 串行实现（逐个 await）下只有第一个会被调用；并行实现（Promise.all）下
    // 四个会同时发起——以此区分串行/并行语义
    mocks.mockFontSizeLoad.mockReturnValueOnce(new Promise<void>(() => {}));
    mocks.mockKeybindingsLoad.mockReturnValueOnce(new Promise<void>(() => {}));
    mocks.mockSideBarLoad.mockReturnValueOnce(new Promise<void>(() => {}));
    mocks.mockCliAliasesLoad.mockReturnValueOnce(new Promise<void>(() => {}));

    render(React.createElement(App));

    // 四个 load 全部已被调用（并行发起）
    await waitFor(() => {
      expect(mocks.mockFontSizeLoad).toHaveBeenCalledTimes(1);
      expect(mocks.mockKeybindingsLoad).toHaveBeenCalledTimes(1);
      expect(mocks.mockSideBarLoad).toHaveBeenCalledTimes(1);
      expect(mocks.mockCliAliasesLoad).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // loadAllProjects 保持在其后（Promise.all 未完成 → 未调用；markPersistenceReady 时序不动）
    expect(mocks.mockLoadAllProjects).not.toHaveBeenCalled();
    expect(mocks.mockMarkPersistenceReady).not.toHaveBeenCalled();
  });
});
