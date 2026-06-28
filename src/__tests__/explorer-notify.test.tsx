// explorer-notify.test.tsx — ExplorerPanel 文件监听集成测试
//
// 验证：项目根路径变化时 ExplorerPanel 调用 startWatch 启动文件监听。
// 使用 mock ipc/notify + 真实 stores，遵循 sidebar-actions 模式。

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup, renderHook, waitFor, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockStartWatch = vi.fn().mockResolvedValue(undefined);
  const mockReadDir = vi.fn().mockResolvedValue([]);
  const mockGitStatus = vi.fn().mockResolvedValue([]);

  return {
    mockStartWatch,
    mockReadDir,
    mockGitStatus,
    resetAll() {
      mockStartWatch.mockClear();
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
    },
  };
});

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  onFsEvent: () => () => {},
}));

vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

// 导入真实 stores + ExplorerPanel（在 IPC mock 之后）
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ExplorerPanel } from "../features/explorer";
import { useFileTree } from "../features/explorer/useFileTree";
import type { DirEntry } from "../types/fs";

// ─── 辅助函数 ───

function populateStore(rootPath = "C:\\test-project") {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: `${rootPath}\\sub`,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStore() {
  useProjects.setState({ projects: {} });
  useLayout.setState({ activePageId: null });
}

function renderExplorer() {
  return render(React.createElement(ExplorerPanel));
}

// ─── Tests ───

describe("ExplorerPanel 文件监听集成", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    resetStore();
  });

  describe("startWatch 调用时机", () => {
    it("1. 活跃项目存在时 → 调用 startWatch(projectRootPath)", () => {
      populateStore("C:\\my-project");
      renderExplorer();

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\my-project");
      expect(mocks.mockStartWatch).toHaveBeenCalledTimes(1);
    });

    it("2. 无活跃项目时 → 不调用 startWatch", () => {
      renderExplorer();

      expect(mocks.mockStartWatch).not.toHaveBeenCalled();
    });

    it("3. 切换项目 → 为新项目根路径重新调用 startWatch", () => {
      populateStore("C:\\project-a");
      const { rerender } = renderExplorer();
      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\project-a");

      // 切换到 project-b
      mocks.resetAll();
      useProjects.setState({
        projects: {
          "proj-2": {
            projectId: "proj-2",
            name: "项目 B",
            rootPath: "C:\\project-b",
            pages: [
              {
                pageId: "page-2",
                name: "页面 2",
                layout: {},
                cwd: "C:\\project-b",
                createdAt: 2,
                lastAccessedAt: 2,
              },
            ],
            activePageId: "page-2",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-2" });
      rerender(React.createElement(ExplorerPanel));

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\project-b");
    });

    it("4. startWatch 失败时 → 不抛错，静默 console.error", () => {
      const consoleSpy = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      mocks.resetAll();
      mocks.mockStartWatch.mockRejectedValueOnce(
        new Error("路径不存在"),
      );

      populateStore("C:\\invalid");
      expect(() => renderExplorer()).not.toThrow();

      // 异步错误在 microtask 中捕获
      expect(mocks.mockStartWatch).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe("projectRootPath 计算", () => {
    it("5. cwd 和 rootPath 不同时 → 对 rootPath 启动监听（项目根）", () => {
      useProjects.setState({
        projects: {
          "proj-3": {
            projectId: "proj-3",
            name: "深层项目",
            rootPath: "D:\\repo-root",
            pages: [
              {
                pageId: "page-3",
                name: "子目录页面",
                layout: {},
                cwd: "D:\\repo-root\\src\\components",
                createdAt: 3,
                lastAccessedAt: 3,
              },
            ],
            activePageId: "page-3",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-3" });
      renderExplorer();

      // 应传 rootPath 而非 cwd
      expect(mocks.mockStartWatch).toHaveBeenCalledWith("D:\\repo-root");
      expect(mocks.mockStartWatch).not.toHaveBeenCalledWith(
        "D:\\repo-root\\src\\components",
      );
    });

    it("6. 仅 rootPath、无 cwd → 使用 rootPath", () => {
      useProjects.setState({
        projects: {
          "proj-4": {
            projectId: "proj-4",
            name: "无 cwd 项目",
            rootPath: "E:\\bare-repo",
            pages: [
              {
                pageId: "page-4",
                name: "默认页面",
                layout: {},
                // cwd 为 undefined
                createdAt: 4,
                lastAccessedAt: 4,
              },
            ],
            activePageId: "page-4",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-4" });
      renderExplorer();

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("E:\\bare-repo");
    });
  });
});

// ═══════════════════════════════════════════════════════════
// P2-50: useFileTree hook 测试 — loadRoot / toggleExpand / loadChildren 状态转换
// ═══════════════════════════════════════════════════════════

/** 辅助：创建模拟 DirEntry */
function mockEntry(name: string, isDir: boolean, path?: string): DirEntry {
  const p = path ?? `/test/${name}`;
  return {
    name,
    path: p,
    isDir,
    ...(isDir ? {} : { size: 1024, modified: Date.now() }),
  };
}

describe("useFileTree 状态转换", () => {
  beforeEach(() => {
    // mockReset: 清空 calls + implementation（包括 mockResolvedValueOnce 队列，防跨测试泄漏）
    mocks.mockReadDir.mockReset();
    mocks.mockGitStatus.mockReset();
    // 默认 mock：返回 3 个根条目（2 目录 + 1 文件）
    mocks.mockReadDir.mockResolvedValue([
      mockEntry("src", true, "/test/src"),
      mockEntry("docs", true, "/test/docs"),
      mockEntry("README.md", false, "/test/README.md"),
    ]);
    mocks.mockGitStatus.mockResolvedValue([]);
  });

  describe("loadRoot 状态转换", () => {
    it("7. rootPath 非空时 → 调用 readDir 并设置 rootNodes", async () => {
      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      // 初始状态：空数组（异步加载中）
      expect(result.current.rootNodes).toEqual([]);

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      });

      expect(mocks.mockReadDir).toHaveBeenCalledWith("/test");
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);

      // 验证节点结构
      const node = result.current.rootNodes[0];
      expect(node.entry.name).toBe("src");
      expect(node.entry.isDir).toBe(true);
      expect(node.expanded).toBe(false);
      expect(node.children).toEqual([]);
      expect(node.loading).toBe(false);
    });

    it("8. rootPath 为 null → rootNodes 为空数组", async () => {
      const { result } = renderHook(() => useFileTree({ rootPath: null }));

      // 同步设置为空
      expect(result.current.rootNodes).toEqual([]);

      // 等待异步完成，确保 readDir 未被调用
      await new Promise((r) => setTimeout(r, 50));
      expect(mocks.mockReadDir).not.toHaveBeenCalled();
    });

    it("9. rootPath 变更时 → 重新调用 loadRoot", async () => {
      const { result, rerender } = renderHook(
        ({ rootPath }) => useFileTree({ rootPath }),
        { initialProps: { rootPath: "/test" as string | null } },
      );

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      });
      expect(mocks.mockReadDir).toHaveBeenCalledWith("/test");

      // 变更 rootPath
      mocks.resetAll();
      mocks.mockReadDir.mockResolvedValue([
        mockEntry("lib", true, "/other/lib"),
        mockEntry("main.ts", false, "/other/main.ts"),
      ]);

      rerender({ rootPath: "/other" });

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(2);
      });
      expect(mocks.mockReadDir).toHaveBeenCalledWith("/other");
    });
  });

  describe("toggleExpand 状态转换", () => {
    it("10. 点击展开目录 → expanded=true → 加载子节点 → loading=false", async () => {
      // 子目录内容
      mocks.mockReadDir
        .mockResolvedValueOnce([
          mockEntry("src", true, "/test/src"),
          mockEntry("docs", true, "/test/docs"),
          mockEntry("README.md", false, "/test/README.md"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
          mockEntry("lib.ts", false, "/test/src/lib.ts"),
        ]);

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      });

      const gitBefore = mocks.mockGitStatus.mock.calls.length;

      // 展开 src 目录
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      // 验证节点状态：expanded=true, loading=false（异步加载完成），children 已填充
      const srcNode = result.current.rootNodes.find(
        (n) => n.entry.path === "/test/src",
      );
      expect(srcNode).toBeDefined();
      expect(srcNode!.expanded).toBe(true);
      expect(srcNode!.loading).toBe(false);
      expect(srcNode!.children.length).toBe(2);
      expect(srcNode!.children[0].entry.name).toBe("index.ts");
      expect(srcNode!.children[1].entry.name).toBe("lib.ts");

      // 验证 readDir 被调用（第二次 call → 加载子目录）
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);

      // toggleExpand 不直接触发 gitStatus
      const gitAfter = mocks.mockGitStatus.mock.calls.length;
      expect(gitAfter).toBe(gitBefore);
    });

    it("11. 点击折叠已展开目录 → expanded=false（toggleExpand 折叠时也会重新 loadChildren）", async () => {
      // toggleExpand 总是调用 loadChildren(nodePath)，不限展开/折叠
      // 因此需要 3 个 once：根加载 + 展开加载 + 折叠时的 re-load
      mocks.mockReadDir
        .mockResolvedValueOnce([
          mockEntry("src", true, "/test/src"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
        ]);

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      await waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      });

      // 展开
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      let srcNode = result.current.rootNodes[0];
      expect(srcNode.expanded).toBe(true);
      expect(srcNode.children.length).toBe(1);

      // 折叠
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      srcNode = result.current.rootNodes[0];
      expect(srcNode.expanded).toBe(false);
      // children 保留（不丢失），但 expanded=false 时不应显示
      expect(srcNode.children.length).toBe(1);
    });
  });

  describe("错误处理", () => {
    it("12. readDir 失败 → rootNodes 保持空数组，不抛错", async () => {
      mocks.mockReadDir.mockRejectedValue(new Error("权限拒绝"));

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      // loadDirectory 的 catch 分支静默返回 []
      await waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalled();
      });

      expect(result.current.rootNodes).toEqual([]);
    });
  });
});
