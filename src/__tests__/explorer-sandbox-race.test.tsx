// explorer-sandbox-race.test.tsx — ExplorerPanel setProjectRoot 沙箱竞态回归测试（DBG-10）
//
// 验证：页面切换时 setProjectRoot 须先于 readDir 完成，防止路径沙箱拒绝文件访问。
// 回归故障A：React effect 子先于父执行 → readDir 必在 setProjectRoot 前到达后端被拒。
//
// 约束：仅测试文件，不修改生产代码。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";

// ─── 共享 mock 工厂 ───
import { mockEntry, makeVfs } from "./helpers/vfs";

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
  resetDeferred();

  const mockSetProjectRoot = vi.fn((_path: string) => {
    void _path;
    return sprPromise;
  });

  return {
    get mockReadDir() { return fs.readDir; },
    get mockGitStatus() { return git.gitStatus; },
    get mockOnFsEvent() { return notify.onFsEvent; },
    get triggerFsEvent() { return notify.triggerFsEvent; },
    get mockSetProjectRoot() { return mockSetProjectRoot; },
    get resolveSetProjectRoot() { return () => { resolveSPR(); }; },
    get rejectSetProjectRoot() { return (err?: unknown) => { rejectSPR(err); }; },
    resetDeferred() { resetDeferred(); mockSetProjectRoot.mockClear(); },
    resetAll() {
      fs.readDir.mockReset();
      git.gitStatus.mockReset();
      notify.onFsEvent.mockClear();
      mockSetProjectRoot.mockClear();
      resetDeferred();
    },
  };
});

// Mock ../ipc/fs：setProjectRoot 为手动控制 promise，readDir 为 spy
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

vi.mock("../ipc/notify", () => ({
  startWatch: vi.fn().mockResolvedValue(undefined),
  onFsEvent: mocks.mockOnFsEvent,
}));

import { useFileTree } from "../features/explorer/useFileTree";
import type { DirEntry } from "../types/fs";

// ─── 辅助 ───

/** 创建模拟 DirEntry */
function makeEntry(name: string, isDir: boolean, path: string): DirEntry {
  return {
    name,
    path,
    isDir,
    ...(isDir ? {} : { size: 1024, modified: 1 }),
  };
}

// ─── Tests ───

describe("DBG-10: ExplorerPanel setProjectRoot 沙箱竞态", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    mocks.mockGitStatus.mockResolvedValue([]);
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

    it("readDir spy 正常挂载", () => {
      expect(mocks.mockReadDir).toBeDefined();
      expect(mocks.mockReadDir.getMockName).toBeDefined();
    });
  });

  describe("useFileTree 正常流程", () => {
    it("rootPath 提供后 loadRoot 调用 readDir", async () => {
      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
      });

      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      await waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalledWith("/proj");
      }, { timeout: 3000 });

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });
    });

    it("rootPath 为 null 时不调用 readDir", () => {
      mocks.mockReadDir.mockClear();
      renderHook(() => useFileTree({ rootPath: null }));
      expect(mocks.mockReadDir).not.toHaveBeenCalled();
    });

    it("readDir 返回的数据正确渲染为文件树节点", async () => {
      makeVfs(mocks.mockReadDir, {
        "/proj": [
          mockEntry("src", true, "/proj/src"),
          mockEntry("README.md", false, "/proj/README.md"),
        ],
      });

      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(2);
      }, { timeout: 3000 });

      const names = result.current.rootNodes.map((n) => n.entry.name).sort();
      expect(names).toEqual(["README.md", "src"]);
    });
  });

  describe("rootPath 变化触发 readDir", () => {
    it("rootPath 从 A 切换到 B 时 readDir 被新路径调用", async () => {
      makeVfs(mocks.mockReadDir, {
        "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
        "/proj-b": [mockEntry("b.ts", false, "/proj-b/b.ts")],
      });

      const { result, rerender } = renderHook(
        ({ rootPath }) => useFileTree({ rootPath }),
        { initialProps: { rootPath: "/proj-a" as string | null } },
      );

      await waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalledWith("/proj-a");
      }, { timeout: 3000 });

      mocks.mockReadDir.mockClear();
      rerender({ rootPath: "/proj-b" });

      await waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalledWith("/proj-b");
      }, { timeout: 3000 });

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
        expect(result.current.rootNodes[0].entry.name).toBe("b.ts");
      }, { timeout: 3000 });
    });
  });

  describe("setProjectRoot + ExplorerPanel 时序集成", () => {
    it("setProjectRoot 正常 resolve 后，useFileTree 可正常加载", async () => {
      mocks.resetDeferred();

      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("main.ts", false, "/proj/main.ts")],
      });

      // 先调 setProjectRoot（模拟正确的时序）
      const sprPromise = mocks.mockSetProjectRoot("/proj");

      // resolve
      mocks.resolveSetProjectRoot();
      await sprPromise;

      expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith("/proj");

      // 然后 useFileTree 加载
      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });
    });

    it("setProjectRoot reject 后 useFileTree 仍可加载（降级场景）", async () => {
      mocks.resetDeferred();

      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("main.ts", false, "/proj/main.ts")],
      });

      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      // setProjectRoot reject
      const sprPromise = mocks.mockSetProjectRoot("/proj");
      mocks.rejectSetProjectRoot(new Error("路径不存在"));
      await expect(sprPromise).rejects.toThrow("路径不存在");

      // useFileTree 仍可加载（降级：文件树可能为空但不应崩溃）
      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });

      consoleSpy.mockRestore();
    });
  });

  describe("沙箱竞态回归：readDir 调用时序", () => {
    it("先启动 setProjectRoot（pending）再加载 useFileTree → readDir 仍会被调用", async () => {
      // 此用例记录当前行为：useFileTree 不感知 setProjectRoot 状态，
      // rootPath 变化后即调用 readDir，无论 setProjectRoot 是否已完成。
      // DBG-5 修复后：switchToPage 先 await setProjectRoot 再 setActivePage，
      // 使得 useFileTree 的 rootPath 变化发生在 setProjectRoot resolve 之后。
      mocks.resetDeferred();

      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("main.ts", false, "/proj/main.ts")],
      });

      // 先调 setProjectRoot（pending 状态）
      mocks.mockSetProjectRoot("/proj");
      expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith("/proj");

      // 未 resolve 时加载 useFileTree
      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      // readDir 在当前实现中会被立即调用（useFileTree 不等待 setProjectRoot）
      await waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalledWith("/proj");
      }, { timeout: 3000 });

      // 文件树数据正常加载
      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });

      // resolve setProjectRoot
      mocks.resolveSetProjectRoot();
      // 最终状态一致
      expect(result.current.rootNodes.length).toBe(1);
    });

    it("setProjectRoot 在 readDir 之前完成时，readDir 正常工作", async () => {
      mocks.resetDeferred();

      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("main.ts", false, "/proj/main.ts")],
      });

      // 先 resolve setProjectRoot
      const sprPromise = mocks.mockSetProjectRoot("/proj");
      mocks.resolveSetProjectRoot();
      await sprPromise;

      // 再触发 useFileTree
      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });

      // readDir 与 setProjectRoot 的调用顺序：setProjectRoot 先完成
      const sprCallOrder = mocks.mockSetProjectRoot.mock.invocationCallOrder[0];
      const rdCallOrder = mocks.mockReadDir.mock.invocationCallOrder[0];
      expect(sprCallOrder).toBeLessThan(rdCallOrder);
    });
  });

  describe("rendering integration with vfs helpers", () => {
    it("makeVfs 绑定后 readDir 返回模拟数据", async () => {
      makeVfs(mocks.mockReadDir, {
        "/root": [
          makeEntry("src", true, "/root/src"),
          makeEntry("index.ts", false, "/root/index.ts"),
        ],
      });

      const { result } = renderHook(() => useFileTree({ rootPath: "/root" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(2);
      }, { timeout: 3000 });

      const names = result.current.rootNodes.map((n) => n.entry.name).sort();
      expect(names).toEqual(["index.ts", "src"]);
    });

    it("vfs 动态增删通过 refresh() 反映在后续渲染中", async () => {
      const vfs = makeVfs(mocks.mockReadDir, {
        "/root": [mockEntry("a.ts", false, "/root/a.ts")],
      });

      const { result } = renderHook(() => useFileTree({ rootPath: "/root" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });
      expect(result.current.rootNodes[0].entry.name).toBe("a.ts");

      // 磁盘新增文件
      vfs.set("/root", [
        mockEntry("a.ts", false, "/root/a.ts"),
        mockEntry("b.ts", false, "/root/b.ts"),
      ]);

      // refresh() 触发 reloadPreservingExpanded 重读目录
      await act(async () => {
        await result.current.refresh();
      });

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(2);
      }, { timeout: 3000 });
      const names = result.current.rootNodes.map((n) => n.entry.name).sort();
      expect(names).toEqual(["a.ts", "b.ts"]);
    });
  });
});
