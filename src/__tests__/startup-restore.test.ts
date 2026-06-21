// startup-restore.test.ts — S4 启动恢复路径自动化测试
//
// Mock localStorage + stores + Workspace，渲染 <App /> 验证启动恢复四条路径：
// 1. localStorage 有记录 → 恢复 activePageId
// 2. localStorage 为空 → 静默降级
// 3. loadAllProjects 异常 → 不阻塞启动
// 4. ready 状态切换：加载中 → 就绪

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockLoadAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockSaveAllProjects = vi.fn().mockResolvedValue(undefined);
  const mockMarkPersistenceReady = vi.fn();
  const mockSetActivePage = vi.fn();
  let capturedHandler: ((e: { preventDefault: () => void }) => void | Promise<void>) | null = null;

  return {
    mockLoadAllProjects,
    mockSaveAllProjects,
    mockMarkPersistenceReady,
    mockSetActivePage,
    get capturedHandler() { return capturedHandler; },
    set capturedHandler(h: typeof capturedHandler) { capturedHandler = h; },
    resetAll() {
      mockLoadAllProjects.mockClear();
      mockSaveAllProjects.mockClear();
      mockMarkPersistenceReady.mockClear();
      mockSetActivePage.mockClear();
      capturedHandler = null;
    },
  };
});

// ─── Module mocks ───
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onCloseRequested: vi.fn((h: (e: { preventDefault: () => void }) => void | Promise<void>) => {
      mocks.capturedHandler = h;
      return Promise.resolve(() => {});
    }),
    destroy: vi.fn().mockResolvedValue(undefined),
  })),
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
      projects: {},
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
    });
  });

  it("2. localStorage 为空 → 静默降级（无异常，setActivePage 不被调用）", async () => {
    setLastActivePage(null);

    render(React.createElement(App));

    await waitFor(() => {
      expect(mocks.mockMarkPersistenceReady).toHaveBeenCalled();
    });
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
    });
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
    });
  });
});
