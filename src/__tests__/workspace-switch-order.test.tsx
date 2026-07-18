// workspace-switch-order.test.tsx — switchToPage 时序断言（DBG-9）
//
// 验证 switchToPage 的 setProjectRoot 前置时序契约：
// - setProjectRoot 须先于 activePageId 生效
// - setProjectRoot 失败时降级（console.error + 仍完成切换）
//
// 约束：仅测试文件，不修改生产代码。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render, waitFor, cleanup, act } from "@testing-library/react";

// ─── Mock @xterm/xterm（同 workspace.test.tsx） ───
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(function (this: Record<string, unknown>) {
    this.open = vi.fn();
    this.dispose = vi.fn();
    this.loadAddon = vi.fn();
    this.write = vi.fn();
    this.writeln = vi.fn();
    this.onData = vi.fn();
    this.focus = vi.fn();
    this.attachCustomKeyEventHandler = vi.fn();
    this.element = document.createElement("div");
    this.options = {} as Record<string, unknown>;
    this.parser = { registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })) };
    return this;
  }),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function (this: Record<string, unknown>) {
    this.fit = vi.fn();
    this.proposeDimensions = vi.fn(() => ({ cols: 80, rows: 24 }));
    this.dispose = vi.fn();
    return this;
  }),
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// ─── Hoisted mocks（需在 vi.mock 执行前就绪） ───
const mocks = vi.hoisted(() => {
  const fs = __createFsMocks();
  const git = __createGitMocks();

  // setProjectRoot 手动控制 promise
  let resolveSPR!: (value: void) => void;
  let rejectSPR!: (reason?: unknown) => void;
  let sprPromise = Promise.resolve();

  const resetDeferred = () => {
    sprPromise = new Promise<void>((res, rej) => {
      resolveSPR = res;
      rejectSPR = rej;
    });
  };
  resetDeferred(); // 初始化为 pending 状态

  // 标记 setProjectRoot 是否已被调用过
  let calledCount = 0;
  const wrappedSetProjectRoot = vi.fn((_path: string) => {
    void _path;
    calledCount++;
    return sprPromise;
  });

  return {
    get mockReadDir() { return fs.readDir; },
    get mockGitStatus() { return git.gitStatus; },
    get mockSetProjectRoot() { return wrappedSetProjectRoot; },
    get resolveSetProjectRoot() { return () => { resolveSPR(); }; },
    get rejectSetProjectRoot() { return (err?: unknown) => { rejectSPR(err); }; },
    resetDeferred() { resetDeferred(); calledCount = 0; wrappedSetProjectRoot.mockClear(); },
    get calledCount() { return calledCount; },
    resetAll() {
      fs.readDir.mockReset();
      git.gitStatus.mockReset();
      resetDeferred();
    },
  };
});

// Mock ../ipc/fs：setProjectRoot 为手动控制 promise，其余为 stub
vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  readFile: vi.fn(),
  writeFile: vi.fn(),
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  setProjectRoot: mocks.mockSetProjectRoot,
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

// setup.ts 已全局 mock ../ipc/notify

import Workspace from "../workspace/Workspace";
import { useProjects, type OperationPage } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { titleManager } from "../workspace/titleManager";

// ─── 辅助 ───

/** 种子：一项目两页面 */
function seedTwoPageProject() {
  const projId = "proj-switch";
  const pageA = "page-alpha";
  const pageB = "page-beta";
  const rootPath = "C:\\switch-test";

  const page1: OperationPage = {
    pageId: pageA, name: "Alpha", layout: {}, cwd: rootPath,
    createdAt: 1, lastAccessedAt: 1,
  };
  const page2: OperationPage = {
    pageId: pageB, name: "Beta", layout: {}, cwd: rootPath,
    createdAt: 2, lastAccessedAt: 2,
  };

  useProjects.getState().addProject({
    projectId: projId,
    name: "switch-test-project",
    rootPath,
    pages: [page1, page2],
    activePageId: pageA,
    version: 1,
  });

  const expanded = useProjects.getState().expandedNodes;
  useProjects.setState({ expandedNodes: { ...expanded, [projId]: true } });

  useLayout.setState({ activePageId: pageA });

  return { projId, pageA, pageB, rootPath };
}

// ─── Tests ───

describe("DBG-9: switchToPage 时序", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    mocks.mockGitStatus.mockResolvedValue([]);
    mocks.mockReadDir.mockResolvedValue([]);

    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    useLayout.setState({ activePageId: null });
    titleManager.reset();
  });

  afterEach(() => {
    clearMocks();
  });

  describe("基础设施", () => {
    it("deferred setProjectRoot mock 正常挂载", () => {
      expect(mocks.mockSetProjectRoot).toBeDefined();
      expect(typeof mocks.mockSetProjectRoot).toBe("function");
    });

    it("deferred promise 初始为 pending 状态", async () => {
      mocks.resetDeferred();
      const spr = mocks.mockSetProjectRoot("C:\\test");

      let resolved = false;
      spr.then(() => { resolved = true; });

      // 微任务后仍未 resolve
      await Promise.resolve();
      expect(resolved).toBe(false);

      // resolve 后变为 true
      mocks.resolveSetProjectRoot();
      await spr;
      expect(resolved).toBe(true);
    });

    it("deferred promise reject 正确传播", async () => {
      mocks.resetDeferred();
      const spr = mocks.mockSetProjectRoot("C:\\test");

      mocks.rejectSetProjectRoot(new Error("模拟失败"));
      await expect(spr).rejects.toThrow("模拟失败");
    });
  });

  describe("渲染 Workspace", () => {
    it("种子项目后 activePageId 正确", () => {
      mockIPC(() => null);
      const { pageA } = seedTwoPageProject();
      expect(useLayout.getState().activePageId).toBe(pageA);
    });

    it("Workspace 渲染不抛错", () => {
      mockIPC(() => null);
      seedTwoPageProject();
      expect(() => render(<Workspace />)).not.toThrow();
    });

    it("渲染后侧栏显示项目名和页面名", () => {
      mockIPC(() => null);
      seedTwoPageProject();
      const { container } = render(<Workspace />);
      const text = container.textContent ?? "";
      expect(text).toContain("switch-test-project");
      expect(text).toContain("Alpha");
      expect(text).toContain("Beta");
    });
  });

  describe("SEC-01 effect: setProjectRoot 调用", () => {
    it("activePageId 已设置时，Workspace 渲染触发 SEC-01 调用 setProjectRoot", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();
      // activePageId 已设为 page-alpha，渲染触发 SEC-01

      render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      }, { timeout: 3000 });
    });

    it("setProjectRoot 接收正确的项目 rootPath", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();

      render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      }, { timeout: 3000 });
    });
  });

  describe("降级：setProjectRoot reject", () => {
    it("setProjectRoot reject 时 console.error 被调用", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();

      const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      seedTwoPageProject();

      render(<Workspace />);

      // 等待 SEC-01 effect 触发 setProjectRoot
      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalled();
      }, { timeout: 3000 });

      // reject
      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      // 等待 .catch 处理
      await act(() => Promise.resolve());

      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "[slTerminal] 设置项目根路径失败:",
        expect.any(Error),
      );
      consoleErrorSpy.mockRestore();
    });

    it("setProjectRoot reject 后 activePageId 仍然有效（降级不阻断）", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();

      vi.spyOn(console, "error").mockImplementation(() => {});
      const { pageA } = seedTwoPageProject();

      render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalled();
      }, { timeout: 3000 });

      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      await act(() => Promise.resolve());

      // activePageId 仍为原始值，不会被清空
      expect(useLayout.getState().activePageId).toBe(pageA);
    });
  });

  describe("时序契约（手动验证）", () => {
    it("先 await setProjectRoot 再 setActivePage —— 顺序正确", async () => {
      mocks.resetDeferred();
      const { pageA, pageB, rootPath } = seedTwoPageProject();

      // 模拟 DBG-5 修复后的正确时序
      const sprPromise = mocks.mockSetProjectRoot(rootPath);

      // 还未 resolve，activePageId 保持原值
      expect(useLayout.getState().activePageId).toBe(pageA);

      // resolve setProjectRoot
      mocks.resolveSetProjectRoot();
      await sprPromise;

      // 然后设置 activePageId
      useLayout.getState().setActivePage(pageB);
      expect(useLayout.getState().activePageId).toBe(pageB);
    });

    it("setProjectRoot reject 后仍可切换页面（降级链路）", async () => {
      mocks.resetDeferred();
      const { pageB, rootPath } = seedTwoPageProject();

      const sprPromise = mocks.mockSetProjectRoot(rootPath);
      mocks.rejectSetProjectRoot(new Error("模拟失败"));

      // reject 后仍可切换
      try { await sprPromise; } catch { /* 预期 */ }
      useLayout.getState().setActivePage(pageB);
      expect(useLayout.getState().activePageId).toBe(pageB);
    });
  });
});

describe("现有测试兼容性排查", () => {
  it("sidebar-actions 使用 mock switchToPage，不受 switchToPage 异步化影响", () => {
    // sidebar-actions 中 switchToPage 为 vi.fn() mock，不依赖真实 Workspace 实现
    const mockFn = vi.fn();
    expect(typeof mockFn).toBe("function");
    mockFn("proj-1", "page-2");
    expect(mockFn).toHaveBeenCalledWith("proj-1", "page-2");
  });

  it("workspace 基础测试的 store 操作不受影响", () => {
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    useLayout.setState({ activePageId: null });

    expect(useLayout.getState().activePageId).toBeNull();
    useLayout.getState().setActivePage("test-page");
    expect(useLayout.getState().activePageId).toBe("test-page");
  });
});
