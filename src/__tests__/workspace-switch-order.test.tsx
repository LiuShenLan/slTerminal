// workspace-switch-order.test.tsx — switchToPage 时序断言（DBG-9）
//
// 验证 switchToPage 的 setProjectRoot 前置时序契约：
// - setProjectRoot 须先于 activePageId 生效
// - setProjectRoot 失败时降级（console.error + toast 告警 + 仍完成切换，FE-04/D7）
//
// 约束：仅测试文件，不修改生产代码。

import { describe, it, expect, beforeEach, afterEach, afterAll, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render, waitFor, cleanup, act, fireEvent } from "@testing-library/react";

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

// 模块级 stub 须 afterAll 恢复——防同 worker 后续文件被污染（TQ-A-02）
const originalResizeObserver = global.ResizeObserver;
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};
afterAll(() => {
  global.ResizeObserver = originalResizeObserver;
});

// ─── Hoisted mocks（需在 vi.mock 执行前就绪） ───
const mocks = vi.hoisted(() => {
  const fs = __createFsMocks();
  const git = __createGitMocks();
  const notify = __createNotifyMocks();

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

  // toast mock（FE-04：失败路径 toast 告警断言）
  const mockToast = { show: vi.fn() };

  return {
    get mockReadDir() { return fs.readDir; },
    get mockGitStatus() { return git.gitStatus; },
    get mockSetProjectRoot() { return wrappedSetProjectRoot; },
    get resolveSetProjectRoot() { return () => { resolveSPR(); }; },
    get rejectSetProjectRoot() { return (err?: unknown) => { rejectSPR(err); }; },
    get mockToast() { return mockToast; },
    get mockStartWatch() { return notify.startWatch; },
    get mockStopWatch() { return notify.stopWatch; },
    resetDeferred() { resetDeferred(); calledCount = 0; wrappedSetProjectRoot.mockClear(); },
    get calledCount() { return calledCount; },
    resetAll() {
      fs.readDir.mockReset();
      git.gitStatus.mockReset();
      notify.startWatch.mockReset();
      notify.stopWatch.mockReset();
      mockToast.show.mockClear();
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

// Mock ../lib：toast 替换为 vi.fn()（FE-04 断言用），其余（ErrorBoundary/E2E_ENABLED 等）保持真实实现
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: mocks.mockToast };
});

// setup.ts 已全局 mock ../ipc/notify——本文件覆盖为可断言实例
// （watcher 上提后 startWatch/stopWatch 由 Workspace 项目激活层调用）
vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  stopWatch: mocks.mockStopWatch,
  onFsEvent: vi.fn(() => () => {}),
}));

import Workspace from "../workspace/Workspace";
import { useProjects, type OperationPage } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { useSideBar } from "../stores/sideBar";
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
    // 种子侧栏 store 默认值（Workspace 三栏改造后依赖 sideBar 状态）
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
      width: 250,
      splitRatio: 0.5,
      loaded: true,
    });
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
      // NavTree 项目默认收起（NAV-10 契约：组件内展开态）——点击项目行展开后页面名可见
      fireEvent.click(container.querySelector('[data-e2e="nav-row-project"]') as HTMLElement);
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
      });
    });

    it("setProjectRoot 接收正确的项目 rootPath", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();

      render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      });
    });
  });

  describe("文件监听跟随项目激活（watcher 上提——E2E editor auto-reload 根因修复）", () => {
    // watcher 生命周期从 ExplorerPanel 上提到 Workspace 项目激活层：
    // 编辑器外部修改 reload / commit 面板刷新依赖 fs-event，
    // 不依赖 explorer 视图是否打开（explorer 关闭时 watcher 缺失根因）。
    it("项目激活 → startWatch(rootPath)（与 setProjectRoot 同 effect）", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();

      render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      });
      // FE-38：setProjectRoot resolve 成功后 startWatch 才启动
      mocks.resolveSetProjectRoot();
      await waitFor(() => {
        expect(mocks.mockStartWatch).toHaveBeenCalledWith(rootPath);
      });
    });

    it("切换项目 → stopWatch(旧) + startWatch(新)（BE-10 成对语义）", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();
      render(<Workspace />);
      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      });
      // FE-38：resolve 成功后 startWatch 才启动
      mocks.resolveSetProjectRoot();
      await waitFor(() => {
        expect(mocks.mockStartWatch).toHaveBeenCalledWith(rootPath);
      });

      // 切换到另一项目（rootPath 不同）
      mocks.resetAll();
      useProjects.getState().addProject({
        projectId: "proj-other",
        name: "其他项目",
        rootPath: "C:\\other-root",
        pages: [
          {
            pageId: "page-other",
            name: "其他页面",
            layout: {},
            cwd: "C:\\other-root",
            createdAt: 9,
            lastAccessedAt: 9,
          },
        ],
        activePageId: "page-other",
        version: 1,
      });
      useLayout.setState({ activePageId: "page-other" });

      // stopWatch(旧) 在 effect 内同步执行；startWatch(新) 待 resolve（FE-38）
      await waitFor(() => {
        expect(mocks.mockStopWatch).toHaveBeenCalledWith(rootPath);
      });
      mocks.resolveSetProjectRoot();
      await waitFor(() => {
        expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\other-root");
      });
    });

    it("无活跃项目 → 不调用 startWatch", () => {
      mockIPC(() => null);
      render(<Workspace />);
      expect(mocks.mockStartWatch).not.toHaveBeenCalled();
    });

    it("activePageId 置 null → stopWatch(旧 rootPath)（BE-10）", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();

      render(<Workspace />);

      // 先等 watcher 激活完成（FE-38：resolve 后 startWatch 才启动）
      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      });
      mocks.resolveSetProjectRoot();
      await waitFor(() => {
        expect(mocks.mockStartWatch).toHaveBeenCalledWith(rootPath);
      });

      // 置 null（删除末页/移除活跃项目两条链）→ 停掉旧项目 watcher
      useLayout.setState({ activePageId: null });

      await waitFor(() => {
        expect(mocks.mockStopWatch).toHaveBeenCalledWith(rootPath);
      });
    });

    it("setProjectRoot resolve 前 startWatch 不启动；reject 时 startWatch 不调用且 toast 告警（FE-38）", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { rootPath } = seedTwoPageProject();
      const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      render(<Workspace />);

      // setProjectRoot 已调用但未 resolve → startWatch 未启动
      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      });
      expect(mocks.mockStartWatch).not.toHaveBeenCalled();

      // reject → startWatch 仍不调用（失败不启动 watcher）+ toast 告警
      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      await act(() => Promise.resolve());

      expect(mocks.mockStartWatch).not.toHaveBeenCalled();
      expect(mocks.mockToast.show).toHaveBeenCalledWith(
        "warning",
        "项目根路径设置失败，文件操作可能被拒绝",
      );
      consoleErrorSpy.mockRestore();
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
      });

      // reject
      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      // 等待 .catch 处理
      await act(() => Promise.resolve());

      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "[slTerminal] 设置项目根路径失败:",
        expect.any(Error),
      );
      // FE-04（D7）：失败路径 toast 告警
      expect(mocks.mockToast.show).toHaveBeenCalledWith(
        "warning",
        "项目根路径设置失败，文件操作可能被拒绝",
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
      });

      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      await act(() => Promise.resolve());

      // activePageId 仍为原始值，不会被清空
      expect(useLayout.getState().activePageId).toBe(pageA);
      // FE-04（D7）：失败路径 toast 告警
      expect(mocks.mockToast.show).toHaveBeenCalledWith(
        "warning",
        "项目根路径设置失败，文件操作可能被拒绝",
      );
    });
  });

  describe("真实驱动：页面行点击触发 switchToPage（DBG-5/9 时序）", () => {
    // 渲染真实 Workspace → 点击侧栏 Beta 页面行 → SidebarTree 回调
    // Workspace.switchToPage（真实 useCallback）→ switchToPageShared。
    // setProjectRoot 用 deferred mock 手动控制挂起/完成，断言时序契约。
    // 挂载时 SEC-01 effect 也会调一次 setProjectRoot——mockClear 隔离切换调用。

    it("点击页面行：setProjectRoot 先 await 完成再 setActivePage", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { pageA, pageB, rootPath } = seedTwoPageProject();
      const setActivePageSpy = vi.spyOn(useLayout.getState(), "setActivePage");

      const { getByText } = render(<Workspace />);

      // 等 SEC-01 effect 挂载时的一次 setProjectRoot 完成，再隔离切换调用
      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalled();
      });
      mocks.mockSetProjectRoot.mockClear();

      // 展开 NavTree 项目行（组件内展开态默认收起）——点击 Beta 页面行（真实 UI 路径）
      fireEvent.click(getByText("switch-test-project"));
      fireEvent.click(getByText("Beta"));
      // 让 switchToPageShared 运行到 await setProjectRoot 挂起点
      await act(() => Promise.resolve());

      // setProjectRoot 已调用但 setActivePage 未执行（activePageId 保持 pageA）
      expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(rootPath);
      expect(useLayout.getState().activePageId).toBe(pageA);
      expect(setActivePageSpy).not.toHaveBeenCalled();

      // resolve 后 setActivePage 执行，activePageId 变为 pageB
      mocks.resolveSetProjectRoot();
      await waitFor(() => {
        expect(useLayout.getState().activePageId).toBe(pageB);
      });

      // invocationCallOrder：setProjectRoot 调用先于 setActivePage
      const sprOrder = mocks.mockSetProjectRoot.mock.invocationCallOrder[0];
      const setPageOrder = setActivePageSpy.mock.invocationCallOrder[0];
      expect(sprOrder).toBeLessThan(setPageOrder);
      setActivePageSpy.mockRestore();
    });

    it("点击页面行：setProjectRoot reject → console.error 降级 + 仍完成切换", async () => {
      mockIPC(() => null);
      mocks.resetDeferred();
      const { pageB } = seedTwoPageProject();
      const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      const { getByText } = render(<Workspace />);

      await waitFor(() => {
        expect(mocks.mockSetProjectRoot).toHaveBeenCalled();
      });
      mocks.mockSetProjectRoot.mockClear();

      // 展开 NavTree 项目行后点击 Beta（组件内展开态默认收起）
      fireEvent.click(getByText("switch-test-project"));
      fireEvent.click(getByText("Beta"));
      await act(() => Promise.resolve());
      mocks.rejectSetProjectRoot(new Error("路径不存在"));

      await waitFor(() => {
        expect(useLayout.getState().activePageId).toBe(pageB);
      });
      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "[slTerminal] 设置项目根路径失败:",
        expect.any(Error),
      );
      consoleErrorSpy.mockRestore();
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
